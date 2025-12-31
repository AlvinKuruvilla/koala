pub mod css_cascade;
pub mod css_parser;
pub mod css_selector;
pub mod css_style;
pub mod css_tokenizer;
pub mod layout;

use crate::lib_dom::{DomTree, NodeId, NodeType};

/// [HTML Standard § 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
///
/// Extract CSS text from all `<style>` elements in the DOM tree.
pub fn extract_style_content(tree: &DomTree) -> String {
    let mut css = String::new();
    collect_style_content(tree, tree.root(), &mut css);
    css
}

/// Recursively collect CSS text from style elements.
fn collect_style_content(tree: &DomTree, id: NodeId, css: &mut String) {
    let Some(node) = tree.get(id) else { return };

    match &node.node_type {
        NodeType::Element(data) if data.tag_name.eq_ignore_ascii_case("style") => {
            // Collect text content of style element
            for &child_id in tree.children(id) {
                if let Some(text) = tree.as_text(child_id) {
                    css.push_str(text);
                    css.push('\n');
                }
            }
        }
        _ => {
            for &child_id in tree.children(id) {
                collect_style_content(tree, child_id, css);
            }
        }
    }
}
