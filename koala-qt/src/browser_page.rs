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
use koala_browser::{
    FontProvider, LoadedDocument, Renderer, RendererFonts, parse_html_string,
};
use std::sync::OnceLock;

// Global, lazily-initialised font cache shared by every `BrowserPage`
// in the process. Loading the four font variants from disk takes
// ~250 ms on macOS; doing that once per tab (and per render!) was the
// dominant cost of new-tab lag, so we load them exactly once and
// clone the Arc handles into every renderer.
fn cached_fonts() -> &'static RendererFonts {
    static FONTS: OnceLock<RendererFonts> = OnceLock::new();
    FONTS.get_or_init(RendererFonts::from_system)
}

// Font provider used for layout-time metrics (line height, glyph
// advance). Different type from `RendererFonts` because koala-css's
// `FontMetrics` trait is keyed on a single `fontdue::Font` handle and
// `FontProvider` owns one internally. Still worth caching globally
// since it hits the same system font files.
fn cached_font_provider() -> &'static FontProvider {
    static PROVIDER: OnceLock<FontProvider> = OnceLock::new();
    PROVIDER.get_or_init(FontProvider::load)
}

pub struct BrowserPage {
    // None until the first `load_html` / `load_landing_page` call.
    // Starting empty avoids the ~5 ms cold cost of parsing "" through
    // koala-browser (which spins up a full Boa JS runtime every time).
    document: Option<LoadedDocument>,
}

impl BrowserPage {
    fn new() -> Self {
        Self { document: None }
    }

    /// Replaces the current document with one parsed from `html`.
    pub fn load_html(&mut self, html: &str) {
        self.document = Some(parse_html_string(html));
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

        // No content loaded yet → return a blank white buffer. Happens
        // in the brief window between `BrowserView` construction and
        // the first `load_landing_page` call.
        let Some(document) = self.document.as_ref() else {
            return vec![255; (width as usize) * (height as usize) * 4];
        };

        let Some(layout_tree) = document.layout_tree.as_ref() else {
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
        let font_metrics = cached_font_provider().metrics();
        layout.layout(viewport, viewport, &*font_metrics, viewport);

        // Paint: turn the laid-out box tree into a display list of
        // fill/stroke/draw-text/draw-image commands.
        let painter = Painter::new(&document.styles);
        let display_list = painter.paint(&layout);

        // Rasterise: execute the display list into an RGBA buffer.
        // Fonts come from the process-wide cache so `new_with_fonts`
        // just clones a few `Arc` handles — no disk I/O in the hot
        // path.
        let mut renderer = Renderer::new_with_fonts(
            width,
            height,
            document.images.clone(),
            cached_fonts().clone(),
        );

        // Propagate the canvas background (CSS 2.1 § 14.2) so regions
        // of the viewport not covered by painted content still show
        // the html/body background colour instead of Renderer's
        // default white fill.
        if let Some(bg) = canvas_background(&document.dom, &document.styles) {
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
