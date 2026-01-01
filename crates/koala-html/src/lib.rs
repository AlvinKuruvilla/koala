//! HTML tokenizer and parser for the Koala browser.
//!
//! # Scope
//!
//! This crate implements:
//! - **HTML Tokenizer** ([WHATWG § 13.2.5](https://html.spec.whatwg.org/multipage/parsing.html#tokenization))
//!   - Data, RCDATA, RAWTEXT, and tag states
//!   - DOCTYPE, comment, and character reference handling
//!   - Attribute parsing
//!
//! - **HTML Parser / Tree Builder** ([WHATWG § 13.2.6](https://html.spec.whatwg.org/multipage/parsing.html#tree-construction))
//!   - Insertion modes: Initial, BeforeHtml, BeforeHead, InHead, AfterHead, InBody, Text, AfterBody, AfterAfterBody
//!   - Implicit tag handling and stack of open elements
//!
//! # Not Yet Implemented
//!
//! - Script data states
//! - Full character reference resolution
//! - Table parsing modes
//! - Form element handling
//! - Foster parenting
//! - Adoption agency algorithm

/// HTML parser and tree construction.
pub mod parser;
/// HTML tokenizer for converting input into tokens.
pub mod tokenizer;

pub use parser::{HTMLParser, InsertionMode, ParseIssue, print_tree};
pub use tokenizer::{Attribute, HTMLTokenizer, Token};
