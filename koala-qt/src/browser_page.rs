// BrowserPage — Rust side of the viewport rendering bridge.
//
// Wraps koala-browser's `LoadedDocument` + rendering pipeline behind
// a small set of methods that cxx exports to the C++ `BrowserView`
// widget. The type is held on the C++ side as `rust::Box<BrowserPage>`
// inside each `BrowserView` instance.
//
// Everything is synchronous for now — `render_to_rgba` blocks the
// calling thread until the full pipeline (layout → paint → rasterise)
// finishes. That's acceptable while we only render the static landing
// page. When we wire up real URL navigation the render call should
// move to a worker thread to keep the Qt event loop responsive.

use koala_browser::css::{Painter, Rect, canvas_background};
use koala_browser::renderer::Renderer;
use koala_browser::{FontProvider, LoadedDocument, parse_html_string};

pub struct BrowserPage {
    // The currently-loaded document. Replaced wholesale on every
    // navigation — koala-browser has no incremental re-parsing yet,
    // and the landing page is small enough that a full re-parse is
    // fast in practice.
    document: LoadedDocument,

    // System font provider, loaded once and reused across renders.
    // Font loading hits disk and is noticeably slower than the rest
    // of the pipeline, so we don't want to repeat it per-render.
    font_provider: FontProvider,
}

impl BrowserPage {
    fn new() -> Self {
        Self {
            document: parse_html_string(""),
            font_provider: FontProvider::load(),
        }
    }

    /// Replaces the current document with one parsed from `html`.
    pub fn load_html(&mut self, html: &str) {
        self.document = parse_html_string(html);
    }

    /// Loads the built-in landing page (see `landing.rs`).
    pub fn load_landing_page(&mut self) {
        self.load_html(crate::landing::LANDING_HTML);
    }

    /// Renders the current document at the given pixel dimensions and
    /// returns the raw RGBA buffer (row-major, 4 bytes per pixel, no
    /// padding). The caller owns the returned `Vec` and is free to
    /// copy it into a `QImage` or upload it to a texture.
    ///
    /// Returns an empty `Vec` when asked for a zero-size render and a
    /// white buffer when the current document has no layout tree
    /// (that only happens for parse-error edge cases where the input
    /// produced no DOM body).
    pub fn render_to_rgba(&mut self, width: u32, height: u32) -> Vec<u8> {
        if width == 0 || height == 0 {
            return Vec::new();
        }

        let Some(layout_tree) = self.document.layout_tree.as_ref() else {
            return vec![255; (width as usize) * (height as usize) * 4];
        };

        // Layout pass: koala-css's `layout` mutates the tree in place,
        // so we clone per render. The clone cost is proportional to
        // the document size, which is fine for the landing page but
        // will want caching once we render real sites.
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        };
        let mut layout = layout_tree.clone();
        let font_metrics = self.font_provider.metrics();
        layout.layout(viewport, viewport, &*font_metrics, viewport);

        // Paint: turn the laid-out box tree into a display list of
        // fill/stroke/draw-text/draw-image commands.
        let painter = Painter::new(&self.document.styles);
        let display_list = painter.paint(&layout);

        // Rasterise: execute the display list into an RGBA buffer.
        // Renderer loads its own fonts internally per-instance; that's
        // a known inefficiency to revisit when we cache renders.
        let mut renderer = Renderer::new(width, height, self.document.images.clone());

        // Propagate the canvas background (CSS 2.1 § 14.2) so regions
        // of the viewport not covered by painted content still show
        // the html/body background colour instead of Renderer's
        // default white fill.
        if let Some(bg) = canvas_background(&self.document.dom, &self.document.styles) {
            renderer.set_canvas_background(bg);
        }

        renderer.render(&display_list);

        renderer.rgba_bytes().to_vec()
    }
}

/// Factory used by the cxx bridge. Must be a free function because
/// cxx `extern "Rust"` types can't expose their own constructors.
pub fn new_browser_page() -> Box<BrowserPage> {
    Box::new(BrowserPage::new())
}
