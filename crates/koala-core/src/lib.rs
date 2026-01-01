//! Core browser API for the Koala browser.
//!
//! # Scope
//!
//! This crate provides:
//! - **Document Loading** - fetch and parse HTML documents
//! - **Style Computation** - integrate CSS with DOM
//! - **Render Tree** - styled DOM ready for layout
//!
//! # Not Yet Implemented
//!
//! - Network requests
//! - JavaScript integration
//! - Resource loading (images, fonts, etc.)

pub use koala_dom as dom;
pub use koala_html as html;
pub use koala_css as css;

use koala_dom::DomTree;
use koala_css::{compute_styles, extract_style_content, ComputedStyle, CSSParser, CSSTokenizer, Stylesheet};
use koala_html::{HTMLParser, HTMLTokenizer};
use std::collections::HashMap;

/// Parse an HTML document and compute styles.
///
/// This is the main entry point for loading a document.
pub fn parse_document(html: &str) -> (DomTree, Stylesheet, HashMap<koala_dom::NodeId, ComputedStyle>) {
    // Parse HTML
    let mut tokenizer = HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let parser = HTMLParser::new(tokenizer.into_tokens());
    let tree = parser.run();

    // Extract and parse CSS
    let css_text = extract_style_content(&tree);
    let mut css_tokenizer = CSSTokenizer::new(css_text);
    css_tokenizer.run();
    let mut css_parser = CSSParser::new(css_tokenizer.into_tokens());
    let stylesheet = css_parser.parse_stylesheet();

    // Compute styles
    let styles = compute_styles(&tree, &stylesheet);

    (tree, stylesheet, styles)
}
