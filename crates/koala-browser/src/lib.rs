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
//!
//! # Not Yet Implemented
//!
//! - JavaScript integration
//! - Resource loading (images, fonts, etc.)

pub mod renderer;

pub use koala_dom as dom;
pub use koala_html as html;
pub use koala_css as css;

use koala_dom::{DomTree, NodeId};
use koala_css::{
    compute_styles, extract_style_content, ComputedStyle, CSSParser, CSSTokenizer, LayoutBox,
    Stylesheet,
};
use koala_html::{HTMLParser, HTMLTokenizer, Token};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

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
/// - URL fetching via curl for http:// and https:// URLs
/// - HTML parsing and tokenization
/// - CSS extraction and parsing
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
    let html_source = if path.starts_with("http://") || path.starts_with("https://") {
        fetch_url(path)?
    } else {
        fs::read_to_string(path)
            .map_err(|e| LoadError::FileError(format!("Failed to read '{}': {}", path, e)))?
    };

    // Parse the document
    let mut doc = parse_html_string(&html_source);
    doc.source_path = path.to_string();

    Ok(doc)
}

/// Parse an HTML string into a LoadedDocument.
///
/// Use this when you already have the HTML content as a string.
pub fn parse_html_string(html: &str) -> LoadedDocument {
    // Tokenize HTML
    let mut tokenizer = HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let tokens = tokenizer.into_tokens();

    // Parse HTML
    let parser = HTMLParser::new(tokens.clone());
    let (dom, issues) = parser.run_with_issues();
    let parse_issues: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();

    // Extract and parse CSS
    let css_text = extract_style_content(&dom);
    let stylesheet = if !css_text.is_empty() {
        let mut css_tokenizer = CSSTokenizer::new(css_text.clone());
        css_tokenizer.run();
        let mut css_parser = CSSParser::new(css_tokenizer.into_tokens());
        css_parser.parse_stylesheet()
    } else {
        Stylesheet { rules: vec![] }
    };

    // Compute styles
    let styles = compute_styles(&dom, &stylesheet);

    // Build layout tree
    let layout_tree = LayoutBox::build_layout_tree(&dom, &styles, dom.root());

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
    }
}

/// Fetch HTML content from a URL using curl.
fn fetch_url(url: &str) -> Result<String, LoadError> {
    let output = Command::new("curl")
        .args(["-sL", "--max-time", "10", url])
        .output()
        .map_err(|e| LoadError::NetworkError(format!("Failed to run curl: {}", e)))?;

    if !output.status.success() {
        return Err(LoadError::NetworkError(format!(
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| LoadError::NetworkError(format!("Invalid UTF-8: {}", e)))
}

