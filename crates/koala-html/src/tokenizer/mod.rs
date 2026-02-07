//! HTML tokenizer module.
//!
//! Implements [§ 13.2.5 Tokenization](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
//! of the WHATWG HTML Living Standard.

/// Character reference parsing per § 13.2.5.72.
pub mod character_reference;
/// HTML tokenizer state machine implementation.
pub mod core;
/// Helper methods for tokenizer state transitions.
pub mod helpers;
/// Named character reference lookup table per § 13.5.
pub mod named_character_references;
/// Token types produced by the tokenizer.
pub mod token;

pub use core::HTMLTokenizer;
pub use token::{Attribute, Token};
