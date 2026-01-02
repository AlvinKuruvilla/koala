//! HTML parser module for tree construction.

/// Foreign content (SVG, MathML) support.
pub mod foreign_content;

/// HTML parser implementation.
pub mod parser;

pub use parser::{print_tree, HTMLParser, InsertionMode, ParseIssue};
