//! CSS parser module.

/// CSS parser implementation per [§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing).
pub mod parser;

pub use parser::{
    AtRule, CSSParser, ComponentValue, Declaration, Rule, Selector, StyleRule, Stylesheet,
};
