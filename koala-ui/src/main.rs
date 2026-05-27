//! koala-ui — Slint-based browser UI for koala.
//!
//! Phase 3 scope: multi-tab browser. Each tab owns its own
//! [`BrowserPage`] (with its own render + loader worker threads),
//! tracked by a [`TabState`] entry in the `tabs` list. The
//! currently-displayed tab is identified by the `active` cell;
//! switching tabs swaps the viewport image, URL bar, back/forward
//! enabled state, title, and loading indicator to the new tab's
//! cached values without touching the previous tab's engine state.
//!
//! Two synchronised collections track the tabs:
//!
//!   * `tabs: Rc<RefCell<Vec<Rc<TabState>>>>` — the Rust-side state
//!     for each tab. Rust callbacks and the timer iterate this.
//!   * `tab_model: Rc<VecModel<TabEntry>>` — the Slint-side view of
//!     each tab (title + loading flag). The `.slint` tab strip
//!     binds to this; updating an entry triggers a tab-strip
//!     refresh without rebuilding the whole row layout.
//!
//! The two collections stay in lockstep — every push/remove on
//! `tabs` is paired with the same operation on `tab_model`, and
//! every per-tab state change that the user can see (title,
//! loading) calls `refresh_tab_entry` to push the new row into
//! `tab_model`.
//!
//! The 16 ms `slint::Timer` polls every tab on every tick (not
//! just the active one). Background tabs' loader and render
//! channels still need draining so their labels can update when a
//! load completes off-screen, and so their cached frame is fresh
//! by the time the user switches to them. Render *requests*,
//! however, only ever go out for the active tab — inactive tabs
//! stay at their last-rendered size until they're activated, at
//! which point the next tick detects the size mismatch and queues
//! a render at the current viewport.

mod browser_page;
mod error_page;
mod landing;
mod tab_state;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::{
    ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel,
};

use tab_state::TabState;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;

    // Tab storage. `Rc<RefCell<Vec<Rc<TabState>>>>` lets the list
    // itself be mutated (new tab / close tab) while each tab can
    // be handed out as an `Rc` to a closure for the duration of a
    // single dispatched event.
    let tabs: Rc<RefCell<Vec<Rc<TabState>>>> = Rc::new(RefCell::new(Vec::new()));
    let active: Rc<Cell<usize>> = Rc::new(Cell::new(0));

    // Mirror of `tabs` for the Slint side. Keeping it as `Rc<VecModel<_>>`
    // (not just `ModelRc`) so we can push/remove/set-row on it from
    // Rust callbacks; the same handle is also published to Slint as
    // a `ModelRc<TabEntry>` via the `tabs` property.
    let tab_model: Rc<VecModel<TabEntry>> = Rc::new(VecModel::default());
    window.set_tabs(ModelRc::from(tab_model.clone()));

    // Open the initial tab on the landing page. `TabState::new_landing`
    // starts the engine workers and seeds the page state; the first
    // timer tick will issue the first render once the viewport area
    // is known.
    push_tab(&tabs, &tab_model, TabState::new_landing());
    active.set(0);
    sync_window_to_active_tab(&window, 0, &tabs.borrow()[0]);
    {
        let tabs = tabs.clone();
        let active = active.clone();
        let tab_model = tab_model.clone();
        let weak = window.as_weak();
        window.on_navigate(move |text| {
            let Some(window) = weak.upgrade() else { return };
            let Some(url) = normalize_url_input(text.as_str()) else { return };
            let i = active.get();
            let tabs_ref = tabs.borrow();
            let Some(tab) = tabs_ref.get(i) else { return };
            window.set_url_text(SharedString::from(url.as_str()));
            *tab.url_text.borrow_mut() = url.clone();
            tab.page.borrow().request_load(&url);
            tab.expecting_paint.set(true);
            window.set_loading(true);
            refresh_tab_entry(&tab_model, i, tab);
        });
    }
    {
        let tabs = tabs.clone();
        let active = active.clone();
        let tab_model = tab_model.clone();
        let weak = window.as_weak();
        window.on_back(move || {
            let Some(window) = weak.upgrade() else { return };
            let i = active.get();
            let tabs_ref = tabs.borrow();
            let Some(tab) = tabs_ref.get(i) else { return };
            if tab.page.borrow_mut().go_back() {
                tab.expecting_paint.set(true);
                window.set_loading(true);
                refresh_tab_entry(&tab_model, i, tab);
            }
        });
    }
    {
        let tabs = tabs.clone();
        let active = active.clone();
        let tab_model = tab_model.clone();
        let weak = window.as_weak();
        window.on_forward(move || {
            let Some(window) = weak.upgrade() else { return };
            let i = active.get();
            let tabs_ref = tabs.borrow();
            let Some(tab) = tabs_ref.get(i) else { return };
            if tab.page.borrow_mut().go_forward() {
                tab.expecting_paint.set(true);
                window.set_loading(true);
                refresh_tab_entry(&tab_model, i, tab);
            }
        });
    }
    {
        let tabs = tabs.clone();
        let active = active.clone();
        let tab_model = tab_model.clone();
        let weak = window.as_weak();
        window.on_reload(move || {
            let Some(window) = weak.upgrade() else { return };
            let i = active.get();
            let tabs_ref = tabs.borrow();
            let Some(tab) = tabs_ref.get(i) else { return };
            // For in-memory pages (landing) there's no URL to
            // re-fetch, but the user still asked to refresh — force
            // a re-render at the current size by resetting the
            // last-requested dims.
            if !tab.page.borrow().reload_current_url() {
                tab.last_requested.set((0, 0));
            }
            tab.expecting_paint.set(true);
            window.set_loading(true);
            refresh_tab_entry(&tab_model, i, tab);
        });
    }

    {
        let tabs = tabs.clone();
        let active = active.clone();
        let weak = window.as_weak();
        window.on_tab_activated(move |i| {
            let Some(window) = weak.upgrade() else { return };
            let Ok(idx) = usize::try_from(i) else { return };
            let tabs_ref = tabs.borrow();
            if idx >= tabs_ref.len() {
                return;
            }
            active.set(idx);
            sync_window_to_active_tab(&window, idx, &tabs_ref[idx]);
        });
    }
    {
        let tabs = tabs.clone();
        let active = active.clone();
        let tab_model = tab_model.clone();
        let weak = window.as_weak();
        window.on_tab_closed(move |i| {
            let Some(window) = weak.upgrade() else { return };
            let Ok(idx) = usize::try_from(i) else { return };
            let mut tabs_mut = tabs.borrow_mut();
            if idx >= tabs_mut.len() {
                return;
            }
            let _ = tabs_mut.remove(idx);
            tab_model.remove(idx);

            if tabs_mut.is_empty() {
                // Closing the last tab quits the browser, matching
                // Chrome / Safari behaviour. Dropping the
                // `BrowserPage`s closes the worker channels, which
                // lets the worker threads exit cleanly.
                drop(tabs_mut);
                let _ = slint::quit_event_loop();
                return;
            }

            // Pick a new active index. Standard browser behaviour:
            // if the closed tab was the active one, fall back to
            // the tab on its left (or row 0 if it was already 0).
            // Otherwise, just shift the index if the close was to
            // the left of active.
            let cur = active.get();
            let new_active = if cur == idx {
                idx.saturating_sub(1).min(tabs_mut.len() - 1)
            } else if cur > idx {
                cur - 1
            } else {
                cur
            };
            active.set(new_active);
            sync_window_to_active_tab(&window, new_active, &tabs_mut[new_active]);
        });
    }
    {
        let tabs = tabs.clone();
        let active = active.clone();
        let tab_model = tab_model.clone();
        let weak = window.as_weak();
        window.on_new_tab(move || {
            let Some(window) = weak.upgrade() else { return };
            let new_tab = TabState::new_landing();
            push_tab(&tabs, &tab_model, new_tab);
            let new_idx = tabs.borrow().len() - 1;
            active.set(new_idx);
            sync_window_to_active_tab(&window, new_idx, &tabs.borrow()[new_idx]);
        });
    }
    // Menu-bar Quit. The Slint event loop returns from `run()`
    // when this is invoked; `Drop` on the `BrowserPage`s closes
    // their worker channels, which lets the worker threads exit.
    window.on_quit_requested(|| {
        let _ = slint::quit_event_loop();
    });

    let timer = Timer::default();
    let weak = window.as_weak();
    let tabs_for_tick = tabs.clone();
    let active_for_tick = active.clone();
    let tab_model_for_tick = tab_model.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(window) = weak.upgrade() else { return };
        let tabs_ref = tabs_for_tick.borrow();
        let active_idx = active_for_tick.get();

        // 1) Drain loader + render channels for every tab. Even
        // background tabs need this so their labels and spinners
        // update when an off-screen load completes.
        for (i, tab) in tabs_ref.iter().enumerate() {
            let load_update = tab.page.borrow_mut().try_take_load_result();
            if load_update.state_swapped {
                let page = tab.page.borrow();
                *tab.title.borrow_mut() = page.current_title();
                *tab.url_text.borrow_mut() = page.current_url().unwrap_or_default();
                tab.can_go_back.set(page.can_go_back());
                tab.can_go_forward.set(page.can_go_forward());
                // `expecting_paint` was set when the navigation
                // was initiated; leave it true — clearing happens
                // when the post-swap frame arrives.
                tab.last_requested.set((0, 0));
                drop(page);
                refresh_tab_entry(&tab_model_for_tick, i, tab);
                if i == active_idx {
                    sync_window_to_active_tab(&window, active_idx, tab);
                }
            }

            if let Some(image) = tab.page.borrow().try_take_render_image() {
                *tab.last_image.borrow_mut() = Some(image.clone());
                if i == active_idx {
                    window.set_viewport_source(image);
                }
                if tab.expecting_paint.get() {
                    tab.expecting_paint.set(false);
                    refresh_tab_entry(&tab_model_for_tick, i, tab);
                    if i == active_idx {
                        window.set_loading(false);
                    }
                }
            }
        }

        // 2) Resize / state-change check, active tab only. Inactive
        // tabs don't queue renders until they're activated — see
        // the module-level docs.
        let Some(active_tab) = tabs_ref.get(active_idx) else { return };
        let scale = window.window().scale_factor();
        let physical_w = (window.get_viewport_width() * scale).round() as u32;
        let physical_h = (window.get_viewport_height() * scale).round() as u32;
        let dims = (physical_w, physical_h);
        if dims != active_tab.last_requested.get() && physical_w > 0 && physical_h > 0 {
            active_tab.last_requested.set(dims);
            active_tab.page.borrow().request_render(physical_w, physical_h);
        }
    });

    window.run()?;
    drop(timer);
    Ok(())
}

/// Append a tab to both the Rust-side `tabs` vec and the
/// Slint-side `tab_model`, keeping them in lockstep. The new
/// entry's `loading` flag starts true to match
/// `TabState::expecting_paint` — the first render of the landing
/// page is the awaited paint and should show a spinner.
fn push_tab(
    tabs: &Rc<RefCell<Vec<Rc<TabState>>>>,
    tab_model: &Rc<VecModel<TabEntry>>,
    state: TabState,
) {
    let entry = TabEntry {
        title: SharedString::from(state.title.borrow().as_str()),
        loading: state.expecting_paint.get(),
    };
    tabs.borrow_mut().push(Rc::new(state));
    tab_model.push(entry);
}

/// Push the per-tab state into the model row at `index`. The
/// VecModel implementation fires a `row_changed` notification that
/// re-renders the corresponding `TabItem` in the strip without
/// rebuilding the whole `for` loop.
fn refresh_tab_entry(model: &VecModel<TabEntry>, index: usize, state: &TabState) {
    let entry = TabEntry {
        title: SharedString::from(state.title.borrow().as_str()),
        loading: state.expecting_paint.get(),
    };
    model.set_row_data(index, entry);
}

/// Copy the active tab's per-tab state into the window-level
/// properties that drive the chrome (title, URL bar, back/forward
/// enabled, loading indicator, viewport image, active-tab index).
/// Called on tab activation, tab close, new tab, and after every
/// state-swap on the active tab. `active_idx` is passed in so the
/// tab strip can highlight the right row.
fn sync_window_to_active_tab(window: &MainWindow, active_idx: usize, tab: &TabState) {
    window.set_active_tab(i32::try_from(active_idx).unwrap_or(0));
    window.set_page_title(SharedString::from(tab.title.borrow().as_str()));
    window.set_url_text(SharedString::from(tab.url_text.borrow().as_str()));
    window.set_back_enabled(tab.can_go_back.get());
    window.set_forward_enabled(tab.can_go_forward.get());
    window.set_loading(tab.expecting_paint.get());
    let image = tab.last_image.borrow().clone().unwrap_or_default();
    window.set_viewport_source(image);
}

/// Heuristic that turns whatever the user typed in the URL bar into
/// something `koala_browser::load_document` will accept. Mirrors the
/// cases `QUrl::fromUserInput` handled in `LocationEdit::normalize_input`:
///
///   * already-valid URL (`https://example.com`) → pass through
///   * bare hostname or hostname-with-port (`example.com`,
///     `localhost:8080`) → prepend `https://`
///   * absolute filesystem path (`/etc/hosts`) → wrap in `file://`
///   * empty / whitespace-only input → `None` (caller skips)
///
/// The host-vs-scheme disambiguation is the only subtle part: a
/// raw `localhost:8080` looks like a scheme-prefixed URL at first
/// glance because `localhost` matches the scheme character class.
/// We resolve the ambiguity by checking what's after the colon —
/// an all-digits suffix means port, not scheme.
fn normalize_url_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if has_url_scheme(trimmed) {
        return Some(trimmed.to_owned());
    }
    if trimmed.starts_with('/') {
        return Some(format!("file://{trimmed}"));
    }
    Some(format!("https://{trimmed}"))
}

/// RFC 3986 §3.1 scheme detection, with one carve-out: `host:port`
/// patterns (alpha host followed by `:` + all-digits) read as a
/// valid scheme to a naive parser but are really a bare hostname
/// the user expects HTTPS to be prepended to. Returning `false` for
/// those routes them through the bare-hostname branch in the caller.
fn has_url_scheme(s: &str) -> bool {
    let Some(colon) = s.find(':') else { return false; };
    let scheme = &s[..colon];
    let after = &s[colon + 1..];
    let starts_alpha = scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic());
    let valid_chars = scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
    if scheme.is_empty() || !starts_alpha || !valid_chars {
        return false;
    }
    // `host:8080` — bare port, not a scheme.
    !after.is_empty() && !after.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_full_urls() {
        assert_eq!(
            normalize_url_input("https://example.com").as_deref(),
            Some("https://example.com"),
        );
        assert_eq!(
            normalize_url_input("http://localhost:8080/foo").as_deref(),
            Some("http://localhost:8080/foo"),
        );
        assert_eq!(
            normalize_url_input("file:///etc/hosts").as_deref(),
            Some("file:///etc/hosts"),
        );
    }

    #[test]
    fn prepends_https_for_bare_hostnames() {
        assert_eq!(
            normalize_url_input("example.com").as_deref(),
            Some("https://example.com"),
        );
        assert_eq!(
            normalize_url_input("  example.com  ").as_deref(),
            Some("https://example.com"),
        );
    }

    #[test]
    fn treats_host_port_as_bare_hostname() {
        // The whole point of the digit-only-suffix check.
        assert_eq!(
            normalize_url_input("localhost:8080").as_deref(),
            Some("https://localhost:8080"),
        );
    }

    #[test]
    fn wraps_absolute_paths_in_file_scheme() {
        assert_eq!(
            normalize_url_input("/etc/hosts").as_deref(),
            Some("file:///etc/hosts"),
        );
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(normalize_url_input("").as_deref(), None);
        assert_eq!(normalize_url_input("   ").as_deref(), None);
    }
}
