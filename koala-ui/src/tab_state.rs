//! Per-tab state for the multi-tab browser shell.
//!
//! Each tab owns its own [`BrowserPage`], which means its own
//! render and loader worker threads. Tabs are mutated wholesale on
//! creation and on close; their internal state (loading indicator,
//! cached last-rendered frame, history-button enabled flags, etc.)
//! is mutated in place from the event-loop thread, so individual
//! fields use `Cell` / `RefCell` instead of locking the whole
//! `TabState` behind a single `RefCell`. The Slint event loop is
//! single-threaded, so the borrows never overlap.
//!
//! Background tabs are still polled every tick — their loader and
//! render channels need draining so the tab strip can update the
//! label and spinner when a load completes off-screen. They do
//! not, however, queue render jobs while inactive; rendering for
//! the new viewport size is deferred until the tab is activated.
//! See `main.rs` for the dispatch logic.

use std::cell::{Cell, RefCell};

use slint::Image;

use crate::browser_page::BrowserPage;

pub struct TabState {
    pub page: RefCell<BrowserPage>,

    /// Last (width, height) handed to `request_render`, in physical
    /// pixels. The timer compares the active tab's viewport size
    /// against this and re-issues a render when they diverge.
    /// Reset to `(0, 0)` when the page state changes so the next
    /// tick repaints at the current size without needing a resize.
    pub last_requested: Cell<(u32, u32)>,

    /// Where the tab is in showing its page. Drives the tab's spinner
    /// and, when this tab is active, the window's progress strip.
    pub loading: Cell<Loading>,

    /// Most-recent rendered frame for this tab. Cached so that
    /// switching tabs can immediately restore the previous viewport
    /// without waiting for a fresh render. `None` until the first
    /// frame arrives.
    pub last_image: RefCell<Option<Image>>,

    /// What to show in the URL bar when this tab is active. Mirrors
    /// `BrowserPage::current_url`, but cached here so the Slint
    /// callback for tab activation doesn't have to dip back into
    /// the engine for every chrome refresh.
    pub url_text: RefCell<String>,

    /// What to show as the tab label (and window title when this
    /// tab is active). Falls back to `"New Tab"` in the UI when
    /// empty.
    pub title: RefCell<String>,

    /// Cached navigation-action enabled state. Updated on every
    /// load-result so toolbar refreshes don't borrow the engine.
    pub can_go_back: Cell<bool>,
    pub can_go_forward: Cell<bool>,
}

impl TabState {
    /// Spawns a fresh `BrowserPage` (which itself spawns the render
    /// + loader workers), seeds it with the landing page, and
    /// returns a tab ready to be appended to the tab list. It starts
    /// loading, awaiting the landing page's first frame.
    pub fn new_landing() -> Self {
        let mut page = BrowserPage::new();
        page.load_landing_page();
        let generation = page.state_generation();
        Self {
            page: RefCell::new(page),
            last_requested: Cell::new((0, 0)),
            loading: Cell::new(Loading::AwaitingFrame(generation)),
            last_image: RefCell::new(None),
            url_text: RefCell::new(String::new()),
            title: RefCell::new(String::new()),
            can_go_back: Cell::new(false),
            can_go_forward: Cell::new(false),
        }
    }
}

/// Where a tab is in showing a page.
///
/// A tab is loading from the moment a navigation starts until a frame of
/// the page that navigation produced is on screen. "A frame arrived" is
/// not enough: the viewport can be re-rendered while a load is in flight
/// (a resize, or anything else that changes its size), and that frame
/// shows the previous page. Frames carry the generation of the page state
/// they show (`BrowserPage::state_generation`), and only one of the
/// awaited generation or later ends loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loading {
    /// The current page is on screen.
    Idle,
    /// A navigation has started and its page has not loaded yet. No frame
    /// ends this, since every frame that can arrive shows an earlier page.
    AwaitingState,
    /// The page state with this generation is loaded; the first frame of
    /// it, or of a later state, ends loading.
    AwaitingFrame(u64),
}

impl Loading {
    /// Whether the tab shows a spinner.
    pub fn is_loading(self) -> bool {
        self != Self::Idle
    }

    /// The state after a frame of page state `generation` is shown.
    #[must_use]
    pub fn after_frame(self, generation: u64) -> Self {
        match self {
            Self::AwaitingFrame(awaited) if generation >= awaited => Self::Idle,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Loading;

    /// A frame of an earlier page leaves the tab loading; a frame of the
    /// awaited page or a later one ends it; and no frame ends a load whose
    /// page has not arrived yet.
    #[test]
    fn only_a_frame_of_the_awaited_page_ends_loading() {
        assert_eq!(Loading::AwaitingFrame(5).after_frame(4), Loading::AwaitingFrame(5));
        assert_eq!(Loading::AwaitingFrame(5).after_frame(5), Loading::Idle);
        assert_eq!(Loading::AwaitingFrame(5).after_frame(6), Loading::Idle);
        assert_eq!(Loading::AwaitingState.after_frame(99), Loading::AwaitingState);
        assert_eq!(Loading::Idle.after_frame(1), Loading::Idle);
    }
}
