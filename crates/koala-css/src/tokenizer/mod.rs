//! CSS tokenizer module.

/// CSS token types per [CSS Syntax Level 3 § 4](https://www.w3.org/TR/css-syntax-3/#tokenization).
pub mod token;
/// CSS tokenizer implementation.
pub mod tokenizer;

pub use token::{CSSToken, HashType, NumericType};
pub use tokenizer::CSSTokenizer;
