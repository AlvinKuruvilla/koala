//! CSS parser module.

/// CSS parser implementation per [§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing).
pub mod css_parser;

pub use css_parser::{
    AtRule, CSSParser, ComponentValue, Declaration, Rule, Selector, StyleRule, Stylesheet,
};
