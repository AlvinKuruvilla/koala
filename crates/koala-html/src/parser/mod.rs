//! HTML parser module for tree construction.

/// HTML parser implementation.
pub mod parser;

pub use parser::{print_tree, HTMLParser, InsertionMode, ParseIssue};
