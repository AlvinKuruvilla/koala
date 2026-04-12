// cxx bridge between the Rust entry point and the Qt widget layer in C++.
//
// The C++ side owns the Qt object graph — QApplication, QMainWindow,
// tab widget, URL bar, and so on. Rust owns the browser engine: each
// C++ `BrowserView` holds a `rust::Box<BrowserPage>` created through
// the `new_browser_page` factory below.
//
// `BrowserPage` runs layout + paint + rasterization on a dedicated
// Rust worker thread; the C++ widget posts jobs with `request_render`
// and picks up finished frames with `try_take_render_result` from a
// Qt timer slot at ~60 Hz.

// Re-exported so the cxx macro can resolve `BrowserPage` and
// `new_browser_page` from the bridge module's scope.
pub use crate::browser_page::{BrowserPage, new_browser_page};

#[cxx::bridge(namespace = "koala")]
pub mod ffi {
    /// One finished frame produced by the Rust render worker.
    ///
    /// `pixels` is RGBA (4 bytes/pixel, row-major, no padding) at
    /// the stated `width`×`height`. An empty `pixels` vector means
    /// "no new frame is ready" — `BrowserView` polls this from a
    /// QTimer and skips empty results.
    ///
    /// `error` is populated when the render worker caught a panic
    /// while trying to rasterise the page. The Rust side of
    /// `try_take_render_result` handles this case internally by
    /// swapping the current page for the built-in error template
    /// and scheduling a fresh render; the C++ caller never sees
    /// the error payload and only knows that no paintable frame
    /// arrived on this tick.
    pub struct RenderResult {
        pub width: u32,
        pub height: u32,
        pub pixels: Vec<u8>,
        pub error: String,
    }

    /// Summary returned by `try_take_load_result`. The GUI side
    /// uses `state_swapped` to decide whether to trigger a fresh
    /// render, and `load_finished` to toggle any loading indicator
    /// (success and failure both count as "finished" — the engine
    /// is no longer working on that request).
    pub struct LoadPollResult {
        /// `true` when the loader worker delivered a new page
        /// state on this poll (successful load only).
        pub state_swapped: bool,
        /// `true` when a load request completed (either with a new
        /// state or with an error). `false` means no load event
        /// was pending.
        pub load_finished: bool,
    }

    extern "Rust" {
        /// Opaque Rust type representing a single browser page's
        /// engine state plus its dedicated render and loader
        /// worker threads.
        type BrowserPage;

        /// Create a fresh `BrowserPage`. Spawns both worker threads.
        /// Called from `BrowserView`'s constructor.
        fn new_browser_page() -> Box<BrowserPage>;

        /// Replace the page's content with HTML parsed from `html`.
        /// Runs on the calling thread (typically the Qt GUI thread).
        /// Used for the built-in landing page and any caller that
        /// already has raw HTML in hand; for URL navigation use
        /// `request_load` instead.
        fn load_html(self: &mut BrowserPage, html: &str);

        /// Replace the page's content with the built-in landing page.
        fn load_landing_page(self: &mut BrowserPage);

        /// Queue a URL load on the loader worker thread. Returns
        /// immediately; the result lands in the result channel and
        /// is picked up by `try_take_load_result`. Accepts
        /// `http://`, `https://`, or filesystem paths.
        fn request_load(self: &BrowserPage, url: &str);

        /// Re-queue the most recently committed URL, if any. Called
        /// by the Reload action. Returns `true` when a request was
        /// actually queued; `false` when the current page came
        /// from `load_html` or the built-in landing page and there
        /// is nothing to re-fetch.
        fn reload_current_url(self: &BrowserPage) -> bool;

        /// Navigate one step back in the per-tab history stack,
        /// queueing a load for the previous URL. Returns `true`
        /// when a back-navigation was issued (there was an earlier
        /// entry), `false` otherwise.
        fn go_back(self: &mut BrowserPage) -> bool;

        /// Navigate one step forward in the per-tab history stack.
        /// Returns `true` when a forward navigation was issued.
        fn go_forward(self: &mut BrowserPage) -> bool;

        /// Whether `go_back` would do anything. Drives the Back
        /// toolbar action's enabled state.
        fn can_go_back(self: &BrowserPage) -> bool;

        /// Whether `go_forward` would do anything. Drives the
        /// Forward toolbar action's enabled state.
        fn can_go_forward(self: &BrowserPage) -> bool;

        /// The current document's `<title>` text, or an empty
        /// string when there is no current document or its title
        /// element is empty. Read by `BrowserView` after every
        /// load to update the tab label.
        fn current_title(self: &BrowserPage) -> String;

        /// Non-blocking check for a completed URL load. See the
        /// `LoadPollResult` docs for the meaning of the two flags.
        fn try_take_load_result(self: &mut BrowserPage) -> LoadPollResult;

        /// Queue a render job on the render worker thread. Returns
        /// immediately. `width` / `height` are in device (physical)
        /// pixels.
        fn request_render(self: &BrowserPage, width: u32, height: u32);

        /// Non-blocking check for a finished frame. Returns an empty
        /// `RenderResult` when nothing is ready.
        fn try_take_render_result(self: &BrowserPage) -> RenderResult;
    }

    unsafe extern "C++" {
        include!("koala_window.h");

        /// Runs the Qt event loop. Creates a `QApplication`, shows the
        /// initial `BrowserWindow`, and blocks until the last window
        /// closes. Returns the exit code `QApplication::exec()` produced.
        fn run_event_loop(argv: Vec<String>) -> i32;
    }
}
