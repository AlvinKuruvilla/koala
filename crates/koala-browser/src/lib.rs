//! High-level browser API for the Koala browser.
//!
//! # Scope
//!
//! This crate provides:
//! - **Document Loading** - fetch and parse HTML documents
//! - **Style Computation** - integrate CSS with DOM
//! - **Render Tree** - styled DOM ready for layout
//! - **Layout Tree** - box tree with computed dimensions
//! - **Software Rendering** - headless screenshot generation
//! - **JavaScript Execution** - inline script execution via Boa
//!
//! # Not Yet Implemented
//!
//! - Resource loading (images, fonts, etc.)
//! - External script loading (`<script src="...">`)
//! - DOM manipulation from JavaScript

pub mod font_metrics;
pub mod renderer;

pub use koala_css as css;
pub use koala_dom as dom;
pub use koala_html as html;
pub use koala_js as js;

// Re-export LoadedImage from koala-common for backwards compatibility.
pub use koala_common::image::LoadedImage;

use koala_css::{
    ComputedStyle, LayoutBox, Stylesheet, compute_styles, extract_all_stylesheets,
    extract_style_content,
};
use koala_dom::{DomTree, NodeId};
use koala_html::{HTMLParser, HTMLTokenizer, Token};
use koala_js::JsRuntime;
use std::collections::HashMap;
use std::fs;

/// A fully loaded and parsed document.
///
/// Contains all the data needed to render a page: DOM, styles, layout tree,
/// and debugging information.
pub struct LoadedDocument {
    /// Original HTML source
    pub html_source: String,

    /// Source path or URL
    pub source_path: String,

    /// HTML tokens (for debugging)
    pub tokens: Vec<Token>,

    /// Parsed DOM tree
    pub dom: DomTree,

    /// Extracted CSS text
    pub css_text: String,

    /// Parsed stylesheet
    pub stylesheet: Stylesheet,

    /// Computed styles per node
    pub styles: HashMap<NodeId, ComputedStyle>,

    /// Layout tree (box tree, dimensions not yet computed)
    pub layout_tree: Option<LayoutBox>,

    /// Parse issues/warnings
    pub parse_issues: Vec<String>,

    /// JavaScript runtime for this document
    pub js_runtime: JsRuntime,

    /// Loaded images keyed by their `src` attribute value.
    ///
    /// [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
    ///
    /// Used by the renderer to draw `DrawImage` commands.
    pub images: HashMap<String, LoadedImage>,
}

/// Error type for document loading.
#[derive(Debug)]
pub enum LoadError {
    /// Failed to read file
    FileError(String),
    /// Failed to fetch URL
    NetworkError(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::FileError(msg) => write!(f, "File error: {}", msg),
            LoadError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for LoadError {}

/// Load a document from a file path or URL.
///
/// This is the main entry point for loading a document. It handles:
/// - File reading for local paths
/// - URL fetching for http:// and https:// URLs
/// - HTML parsing and tokenization
/// - CSS extraction and parsing (including external stylesheets)
/// - Style computation
/// - Layout tree construction
///
/// # Arguments
///
/// * `path` - A file path or URL to load
///
/// # Returns
///
/// A `LoadedDocument` containing all parsed data, or a `LoadError`.
pub fn load_document(path: &str) -> Result<LoadedDocument, LoadError> {
    // Fetch or read the HTML source
    let (html_source, base_url) = if path.starts_with("http://") || path.starts_with("https://") {
        let text = koala_common::net::fetch_text(path)
            .map_err(|e| LoadError::NetworkError(e))?;
        (text, Some(path))
    } else {
        let content = fs::read_to_string(path)
            .map_err(|e| LoadError::FileError(format!("Failed to read '{}': {}", path, e)))?;
        (content, None)
    };

    // Parse the document with base URL for resolving external stylesheets
    let mut doc = parse_html_with_base_url(&html_source, base_url);
    doc.source_path = path.to_string();

    Ok(doc)
}

/// Parse an HTML string into a LoadedDocument.
///
/// Use this when you already have the HTML content as a string.
/// Note: External stylesheets cannot be loaded without a base URL.
pub fn parse_html_string(html: &str) -> LoadedDocument {
    parse_html_with_base_url(html, None)
}

/// Parse an HTML string with an optional base URL for resolving external resources.
fn parse_html_with_base_url(html: &str, base_url: Option<&str>) -> LoadedDocument {
    // Tokenize HTML
    let mut tokenizer = HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let tokens = tokenizer.into_tokens();

    // Parse HTML
    let parser = HTMLParser::new(tokens.clone());
    let (dom, issues) = parser.run_with_issues();
    let mut parse_issues: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();

    // Extract and parse CSS (including external stylesheets)
    // TODO: Implement proper Fetch Standard and CSSOM spec compliance
    let doc_stylesheets = extract_all_stylesheets(&dom, base_url);
    let stylesheet = doc_stylesheets.into_merged_stylesheet();

    // Keep inline CSS text for debugging
    let css_text = extract_style_content(&dom);

    // Compute styles
    // [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
    //
    // "Each style rule has a cascade origin... User-Agent origin rules have
    // the lowest priority."
    let ua = koala_css::ua_stylesheet::ua_stylesheet();
    let styles = compute_styles(&dom, ua, &stylesheet);

    // Load images referenced by <img> elements
    let (images, image_dims) = load_images(&dom, base_url);

    // Build layout tree
    let layout_tree = LayoutBox::build_layout_tree(&dom, &styles, dom.root(), &image_dims);

    // Execute JavaScript
    // [§ 4.12.1.1 Processing model](https://html.spec.whatwg.org/multipage/scripting.html)
    let mut js_runtime = JsRuntime::new();
    let scripts = extract_inline_scripts(&dom);
    for script in scripts {
        if let Err(e) = js_runtime.execute(&script) {
            parse_issues.push(format!("JavaScript error: {e}"));
        }
    }

    LoadedDocument {
        html_source: html.to_string(),
        source_path: String::new(),
        tokens,
        dom,
        css_text,
        stylesheet,
        styles,
        layout_tree,
        parse_issues,
        js_runtime,
        images,
    }
}

/// Load images referenced by `<img>` elements in the DOM.
///
/// [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
///
/// Walks the DOM for `<img>` elements with a `src` attribute, fetches the
/// image data (network or filesystem), and decodes it to RGBA pixels.
///
/// Returns:
/// - A map of src → LoadedImage for the renderer
/// - A map of NodeId → (width, height) for layout intrinsic dimensions
fn load_images(
    dom: &DomTree,
    base_url: Option<&str>,
) -> (HashMap<String, LoadedImage>, HashMap<NodeId, (f32, f32)>) {
    let mut images: HashMap<String, LoadedImage> = HashMap::new();
    let mut image_dims: HashMap<NodeId, (f32, f32)> = HashMap::new();

    for node_id in dom.iter_all() {
        if let Some(element) = dom.as_element(node_id) {
            if !element.tag_name.eq_ignore_ascii_case("img") {
                continue;
            }

            let Some(src) = element.attrs.get("src") else {
                continue;
            };
            let src = src.trim();
            if src.is_empty() {
                continue;
            }

            // If we already loaded this src, just record its dims for this node.
            if let Some(existing) = images.get(src) {
                let _ = image_dims.insert(node_id, existing.dimensions_f32());
                continue;
            }

            // Resolve URL and fetch bytes.
            let resolved = koala_common::url::resolve_url(src, base_url);

            let bytes = if resolved.starts_with("http://") || resolved.starts_with("https://") {
                match koala_common::net::fetch_bytes(&resolved) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[Koala] Warning: failed to fetch image '{}': {}", src, e);
                        continue;
                    }
                }
            } else {
                match fs::read(&resolved) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[Koala] Warning: failed to read image '{}': {}", resolved, e);
                        continue;
                    }
                }
            };

            // Decode with the `image` crate.
            match image::load_from_memory(&bytes) {
                Ok(dynamic_img) => {
                    let rgba = dynamic_img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let loaded = LoadedImage::new(w, h, rgba.into_raw());
                    let _ = image_dims.insert(node_id, (w as f32, h as f32));
                    let _ = images.insert(src.to_string(), loaded);
                }
                Err(e) => {
                    eprintln!("[Koala] Warning: failed to decode image '{}': {}", src, e);
                }
            }
        }
    }

    (images, image_dims)
}

/// Extract inline script content from the DOM.
///
/// [§ 4.12.1 The script element](https://html.spec.whatwg.org/multipage/scripting.html#the-script-element)
///
/// Finds all `<script>` elements without a `src` attribute and extracts their
/// text content. Scripts are returned in document order.
fn extract_inline_scripts(dom: &DomTree) -> Vec<String> {
    let mut scripts = Vec::new();

    // Walk the DOM in document order
    for node_id in dom.iter_all() {
        // Check if this is a <script> element
        if let Some(element) = dom.as_element(node_id) {
            if element.tag_name.eq_ignore_ascii_case("script") {
                // Skip external scripts (those with src attribute)
                // [§ 4.12.1.3](https://html.spec.whatwg.org/multipage/scripting.html)
                // "If the element has a src content attribute..."
                if element.attrs.contains_key("src") {
                    continue;
                }

                // Collect text content from child text nodes
                // [§ 4.12.1.3](https://html.spec.whatwg.org/multipage/scripting.html)
                // "...the script block's source is the value of the text content..."
                let mut script_text = String::new();
                for child_id in dom.children(node_id) {
                    if let Some(text) = dom.as_text(*child_id) {
                        script_text.push_str(text);
                    }
                }

                if !script_text.is_empty() {
                    scripts.push(script_text);
                }
            }
        }
    }

    scripts
}

/// Try to load a system font for text measurement and rendering.
///
/// Searches common system font paths (macOS, Linux, Windows) and returns
/// the first font that loads successfully, or None if no font is found.
pub fn load_system_font() -> Option<fontdue::Font> {
    renderer::Renderer::load_system_font()
}

/// Create a [`FontMetrics`](koala_css::FontMetrics) provider, using real
/// font metrics if a font is available, falling back to approximation.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
pub fn create_font_metrics(
    font: Option<&fontdue::Font>,
) -> Box<dyn koala_css::FontMetrics + '_> {
    match font {
        Some(f) => Box::new(font_metrics::FontdueFontMetrics::new(f)),
        None => Box::new(koala_css::ApproximateFontMetrics),
    }
}

/// Opaque font handle for text measurement during layout.
///
/// Wraps the underlying font library so that downstream crates (koala-gui,
/// koala-cli) can use real font metrics without depending on fontdue directly.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
pub struct FontProvider {
    /// The loaded system font, if one was found.
    font: Option<fontdue::Font>,
}

impl FontProvider {
    /// Load a system font for text measurement.
    ///
    /// Searches common system font paths and loads the first one found.
    /// If no font is available, [`metrics()`](Self::metrics) will return
    /// an approximate metrics provider.
    pub fn load() -> Self {
        Self {
            font: renderer::Renderer::load_system_font(),
        }
    }

    /// Create a [`FontMetrics`](koala_css::FontMetrics) provider from this font.
    ///
    /// Returns real per-glyph metrics if a font was loaded, or an
    /// approximation (0.6 × font size per character) otherwise.
    pub fn metrics(&self) -> Box<dyn koala_css::FontMetrics + '_> {
        create_font_metrics(self.font.as_ref())
    }
}
