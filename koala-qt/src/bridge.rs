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
    pub struct RenderResult {
        pub width: u32,
        pub height: u32,
        pub pixels: Vec<u8>,
    }

    extern "Rust" {
        /// Opaque Rust type representing a single browser page's
        /// engine state plus its dedicated render worker thread.
        type BrowserPage;

        /// Create a fresh `BrowserPage`. Spawns the worker thread.
        /// Called from `BrowserView`'s constructor.
        fn new_browser_page() -> Box<BrowserPage>;

        /// Replace the page's content with HTML parsed from `html`.
        /// Runs on the calling thread (typically the Qt GUI thread).
        fn load_html(self: &mut BrowserPage, html: &str);

        /// Replace the page's content with the built-in landing page.
        fn load_landing_page(self: &mut BrowserPage);

        /// Queue a render job on the worker thread. Returns
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
