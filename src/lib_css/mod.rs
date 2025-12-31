pub mod css_cascade;
pub mod css_parser;
pub mod css_selector;
pub mod css_style;
pub mod css_tokenizer;

use crate::lib_dom::{Node, NodeType};

/// [HTML Standard § 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
///
/// Extract CSS text from all `<style>` elements in the DOM tree.
pub fn extract_style_content(dom: &Node) -> String {
    let mut css = String::new();
    collect_style_content(dom, &mut css);
    css
}

/// Recursively collect CSS text from style elements.
fn collect_style_content(node: &Node, css: &mut String) {
    match &node.node_type {
        NodeType::Element(data) if data.tag_name.eq_ignore_ascii_case("style") => {
            // Collect text content of style element
            for child in &node.children {
                if let NodeType::Text(text) = &child.node_type {
                    css.push_str(text);
                    css.push('\n');
                }
            }
        }
        _ => {
            for child in &node.children {
                collect_style_content(child, css);
            }
        }
    }
}
