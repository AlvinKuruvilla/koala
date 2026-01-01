//! CSS tokenizer, parser, selector matching, cascade, and style computation for the Koala browser.
//!
//! # Scope
//!
//! This crate implements:
//! - **CSS Tokenizer** ([§ 4 Tokenization](https://www.w3.org/TR/css-syntax-3/#tokenization))
//!   - All token types: ident, function, at-keyword, hash, string, url, number, dimension, etc.
//!   - Comment handling
//!   - Escape sequences
//!
//! - **CSS Parser** ([§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing))
//!   - Stylesheet parsing
//!   - Rule parsing (style rules and at-rules)
//!   - Declaration parsing
//!
//! - **CSS Selectors** ([Selectors Level 4](https://www.w3.org/TR/selectors-4/))
//!   - Type, class, ID, and universal selectors
//!   - Compound selectors
//!   - Complex selectors with combinators (descendant, child, sibling)
//!   - Specificity calculation
//!
//! - **CSS Cascade** ([CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/))
//!   - Selector matching
//!   - Specificity-based ordering
//!   - Property inheritance
//!
//! - **Computed Styles** ([CSS Values Level 4](https://www.w3.org/TR/css-values-4/))
//!   - Color values (hex, named colors)
//!   - Length values (px)
//!   - Shorthand property expansion (margin, padding, border)
//!
//! - **Layout Engine** (stub) ([CSS Display Level 3](https://www.w3.org/TR/css-display-3/))
//!   - Display value types
//!   - Box model structures
//!
//! # Not Yet Implemented
//!
//! - Percentage and relative length units (em, rem, %)
//! - rgb(), hsl() color functions
//! - Pseudo-classes and pseudo-elements
//! - Attribute selectors
//! - Media queries
//! - Full layout algorithm

/// CSS backgrounds per [CSS Backgrounds and Borders Level 3](https://www.w3.org/TR/css-backgrounds-3/).
pub mod backgrounds;
/// CSS cascade and style computation per [CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/).
pub mod cascade;
/// Box model and layout structures per [CSS Display Level 3](https://www.w3.org/TR/css-display-3/).
pub mod layout;
/// CSS parser per [§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing).
pub mod parser;
/// CSS selector parsing and matching per [Selectors Level 4](https://www.w3.org/TR/selectors-4/).
pub mod selector;
/// Computed style representation per [CSS Values Level 4](https://www.w3.org/TR/css-values-4/).
pub mod style;
/// CSS tokenizer per [§ 4 Tokenization](https://www.w3.org/TR/css-syntax-3/#tokenization).
pub mod tokenizer;

// Re-exports for convenience
pub use backgrounds::canvas_background;
pub use cascade::compute_styles;
pub use parser::{CSSParser, ComponentValue, Declaration, Rule, Stylesheet};
pub use selector::{parse_selector, ParsedSelector, Specificity};
pub use style::{BorderValue, ColorValue, ComputedStyle, LengthValue};
pub use tokenizer::{CSSToken, CSSTokenizer};

use koala_dom::{DomTree, NodeId, NodeType};

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
