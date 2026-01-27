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

pub mod renderer;

pub use koala_css as css;
pub use koala_dom as dom;
pub use koala_html as html;
pub use koala_js as js;

use koala_css::{
    ComputedStyle, LayoutBox, Stylesheet, compute_styles, extract_all_stylesheets,
    extract_style_content,
};
use koala_dom::{DomTree, NodeId};
use koala_html::{HTMLParser, HTMLTokenizer, Token};
use koala_js::JsRuntime;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

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
        (fetch_url(path)?, Some(path))
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
    let styles = compute_styles(&dom, &stylesheet);

    // Build layout tree
    let layout_tree = LayoutBox::build_layout_tree(&dom, &styles, dom.root());

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
    }
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

/// Fetch HTML content from a URL using reqwest.
fn fetch_url(url: &str) -> Result<String, LoadError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| LoadError::NetworkError(format!("Failed to create HTTP client: {e}")))?;

    // TODO: Implement proper Fetch Standard (https://fetch.spec.whatwg.org/)
    // For now, just set a User-Agent to avoid basic bot detection.
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .map_err(|e| LoadError::NetworkError(format!("Request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(LoadError::NetworkError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    response
        .text()
        .map_err(|e| LoadError::NetworkError(format!("Failed to read response body: {e}")))
}
