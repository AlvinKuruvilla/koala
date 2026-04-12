// cxx bridge between the Rust entry point and the Qt widget layer in C++.
//
// The C++ side owns the Qt object graph — QApplication, QMainWindow, tab
// widget, URL bar, and so on. Rust owns the browser engine: each C++
// `BrowserView` holds a `rust::Box<BrowserPage>` created through the
// `new_browser_page` factory below.

// Re-exported so the cxx macro can resolve `BrowserPage` and
// `new_browser_page` from the bridge module's scope. Without this the
// generated glue code can't find the type across module boundaries.
pub use crate::browser_page::{BrowserPage, new_browser_page};

#[cxx::bridge(namespace = "koala")]
pub mod ffi {
    extern "Rust" {
        /// Opaque Rust type representing a single browser page's
        /// engine state (parsed document, fonts, layout caches).
        type BrowserPage;

        /// Create a fresh `BrowserPage`. Called from
        /// `BrowserView`'s constructor.
        fn new_browser_page() -> Box<BrowserPage>;

        /// Replace the page's content with HTML parsed from `html`.
        fn load_html(self: &mut BrowserPage, html: &str);

        /// Replace the page's content with the built-in landing page.
        fn load_landing_page(self: &mut BrowserPage);

        /// Run layout + paint + rasterise and return the resulting
        /// RGBA pixel buffer. `width` / `height` are in device
        /// (physical) pixels. The buffer has no padding — stride is
        /// exactly `width * 4`.
        fn render_to_rgba(self: &mut BrowserPage, width: u32, height: u32) -> Vec<u8>;
    }

    unsafe extern "C++" {
        include!("koala_window.h");

        /// Runs the Qt event loop. Creates a `QApplication`, shows the
        /// initial `BrowserWindow`, and blocks until the last window
        /// closes. Returns the exit code `QApplication::exec()` produced.
        fn run_event_loop(argv: Vec<String>) -> i32;
    }
}
