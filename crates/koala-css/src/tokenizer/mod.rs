//! CSS tokenizer module.

/// CSS tokenizer implementation.
pub mod css_tokenizer;
/// CSS token types per [§ 4 Tokenization](https://www.w3.org/TR/css-syntax-3/#tokenization).
pub mod token;

pub use css_tokenizer::CSSTokenizer;
pub use token::{CSSToken, HashType, NumericType};
