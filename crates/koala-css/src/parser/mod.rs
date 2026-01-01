//! CSS parser module.

/// CSS parser implementation per [CSS Syntax Level 3 § 5](https://www.w3.org/TR/css-syntax-3/#parsing).
pub mod parser;

pub use parser::{
    AtRule, CSSParser, ComponentValue, Declaration, Rule, Selector, StyleRule, Stylesheet,
};
