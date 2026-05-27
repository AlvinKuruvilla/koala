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

    /// True between "navigation initiated for this tab" and "first
    /// frame painted after the page state swapped". Drives the
    /// tab's spinner (and, when this tab is active, the window's
    /// progress strip). Deliberately orthogonal to resize-driven
    /// renders so dragging the window doesn't flicker the
    /// indicator on idle tabs.
    pub expecting_paint: Cell<bool>,

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
    /// returns a tab ready to be appended to the tab list. The
    /// `expecting_paint` flag starts `true` because the landing
    /// page's first render is the awaited paint — without it the
    /// spinner would be off until a user-driven navigation lit it.
    pub fn new_landing() -> Self {
        let mut page = BrowserPage::new();
        page.load_landing_page();
        Self {
            page: RefCell::new(page),
            last_requested: Cell::new((0, 0)),
            expecting_paint: Cell::new(true),
            last_image: RefCell::new(None),
            url_text: RefCell::new(String::new()),
            title: RefCell::new(String::new()),
            can_go_back: Cell::new(false),
            can_go_forward: Cell::new(false),
        }
    }
}
