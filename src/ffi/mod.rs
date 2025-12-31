//! FFI module for exposing Koala's HTML/CSS parsing to other languages.
//!
//! This module provides C-compatible functions that can be called from Swift, Python, etc.

use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::ptr;

use crate::lib_css::css_cascade::compute_styles;
use crate::lib_css::css_parser::parser::CSSParser;
use crate::lib_css::css_style::ComputedStyle;
use crate::lib_css::css_tokenizer::tokenizer::CSSTokenizer;
use crate::lib_css::extract_style_content;
use crate::lib_dom::{Node, NodeType};
use crate::lib_html::html_parser::parser::HTMLParser;
use crate::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

/// Opaque handle to a parsed DOM document with computed styles
pub struct KoalaDocument {
    root: Node,
    styles: HashMap<*const Node, ComputedStyle>,
}

/// Parse HTML string and return a handle to the document.
/// Returns null on error.
///
/// # Safety
/// The `html` parameter must be a valid null-terminated C string.
/// The returned pointer must be freed with `koala_document_free`.
#[no_mangle]
pub unsafe extern "C" fn koala_parse_html(html: *const c_char) -> *mut KoalaDocument {
    if html.is_null() {
        return ptr::null_mut();
    }

    let html_str = match CStr::from_ptr(html).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };

    // Parse HTML
    let mut tokenizer = HTMLTokenizer::new(html_str);
    tokenizer.run();
    let tokens = tokenizer.into_tokens();
    let parser = HTMLParser::new(tokens);
    let root = parser.run();

    // Extract CSS from <style> elements and compute styles
    let css_text = extract_style_content(&root);
    let styles = if !css_text.is_empty() {
        let mut css_tokenizer = CSSTokenizer::new(css_text);
        css_tokenizer.run();
        let mut css_parser = CSSParser::new(css_tokenizer.into_tokens());
        let stylesheet = css_parser.parse_stylesheet();
        compute_styles(&root, &stylesheet)
    } else {
        HashMap::new()
    };

    let doc = Box::new(KoalaDocument { root, styles });
    Box::into_raw(doc)
}

/// Free a document handle.
///
/// # Safety
/// The `doc` parameter must be a valid pointer returned by `koala_parse_html`,
/// or null (in which case this is a no-op).
#[no_mangle]
pub unsafe extern "C" fn koala_document_free(doc: *mut KoalaDocument) {
    if !doc.is_null() {
        drop(Box::from_raw(doc));
    }
}

/// Get the document as a JSON string representation.
/// Returns null on error.
///
/// # Safety
/// The `doc` parameter must be a valid pointer returned by `koala_parse_html`.
/// The returned string must be freed with `koala_string_free`.
#[no_mangle]
pub unsafe extern "C" fn koala_document_to_json(doc: *const KoalaDocument) -> *mut c_char {
    if doc.is_null() {
        return ptr::null_mut();
    }

    let doc = &*doc;
    let json = node_to_json_with_styles(&doc.root, &doc.styles);

    match CString::new(json) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string returned by Koala functions.
///
/// # Safety
/// The `s` parameter must be a valid pointer returned by a Koala function,
/// or null (in which case this is a no-op).
#[no_mangle]
pub unsafe extern "C" fn koala_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Get the number of child nodes for a document.
///
/// # Safety
/// The `doc` parameter must be a valid pointer returned by `koala_parse_html`.
#[no_mangle]
pub unsafe extern "C" fn koala_document_child_count(doc: *const KoalaDocument) -> usize {
    if doc.is_null() {
        return 0;
    }
    (*doc).root.children.len()
}

// Internal helper to convert a node to JSON with computed styles
fn node_to_json_with_styles(
    node: &Node,
    styles: &HashMap<*const Node, ComputedStyle>,
) -> String {
    let mut json = String::from("{");

    match &node.node_type {
        NodeType::Document => {
            json.push_str("\"type\":\"document\"");
        }
        NodeType::Element(data) => {
            json.push_str("\"type\":\"element\",");
            json.push_str(&format!(
                "\"tagName\":{},",
                escape_json_string(&data.tag_name)
            ));

            // Attributes
            json.push_str("\"attributes\":{");
            let attrs: Vec<String> = data
                .attrs
                .iter()
                .map(|(k, v)| format!("{}:{}", escape_json_string(k), escape_json_string(v)))
                .collect();
            json.push_str(&attrs.join(","));
            json.push('}');

            // Computed style (if available)
            if let Some(style) = styles.get(&(node as *const Node)) {
                if let Ok(style_json) = serde_json::to_string(style) {
                    json.push_str(",\"computedStyle\":");
                    json.push_str(&style_json);
                }
            }
        }
        NodeType::Text(text) => {
            json.push_str("\"type\":\"text\",");
            json.push_str(&format!("\"content\":{}", escape_json_string(text)));
        }
        NodeType::Comment(text) => {
            json.push_str("\"type\":\"comment\",");
            json.push_str(&format!("\"content\":{}", escape_json_string(text)));
        }
    }

    // Children
    if !node.children.is_empty() {
        json.push_str(",\"children\":[");
        let children: Vec<String> = node
            .children
            .iter()
            .map(|child| node_to_json_with_styles(child, styles))
            .collect();
        json.push_str(&children.join(","));
        json.push(']');
    }

    json.push('}');
    json
}

fn escape_json_string(s: &str) -> String {
    let mut result = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_parse_simple_html() {
        let html = CString::new("<html><body><h1>Hello</h1></body></html>").unwrap();

        unsafe {
            let doc = koala_parse_html(html.as_ptr());
            assert!(!doc.is_null());

            let json_ptr = koala_document_to_json(doc);
            assert!(!json_ptr.is_null());

            let json = CStr::from_ptr(json_ptr).to_str().unwrap();
            println!("JSON output: {}", json);
            assert!(json.contains("\"type\":\"document\""));
            assert!(json.contains("\"tagName\":\"html\""));
            assert!(json.contains("\"tagName\":\"h1\""));
            assert!(json.contains("Hello"));

            koala_string_free(json_ptr);
            koala_document_free(doc);
        }
    }

    #[test]
    fn test_null_input() {
        unsafe {
            let doc = koala_parse_html(ptr::null());
            assert!(doc.is_null());
        }
    }

    #[test]
    fn test_parse_html_with_css_styles() {
        let html = CString::new(
            r#"<html>
            <head>
                <style>
                    body { color: #333333; }
                    .highlight { background-color: #ffff00; }
                </style>
            </head>
            <body>
                <p class="highlight">Hello</p>
            </body>
            </html>"#,
        )
        .unwrap();

        unsafe {
            let doc = koala_parse_html(html.as_ptr());
            assert!(!doc.is_null());

            let json_ptr = koala_document_to_json(doc);
            assert!(!json_ptr.is_null());

            let json = CStr::from_ptr(json_ptr).to_str().unwrap();
            println!("JSON with styles: {}", json);

            // Should contain computed styles
            assert!(json.contains("\"computedStyle\""));
            // Body should have color applied
            assert!(json.contains("\"color\""));
            // Paragraph should have background-color from .highlight
            assert!(json.contains("\"background_color\""));

            koala_string_free(json_ptr);
            koala_document_free(doc);
        }
    }
}
