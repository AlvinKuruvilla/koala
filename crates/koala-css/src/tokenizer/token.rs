//! CSS Token types per [§ 4 Tokenization](https://www.w3.org/TR/css-syntax-3/#tokenization).
//!
//! "The output of the tokenization step is a stream of zero or more of the
//! following tokens: `<ident-token>`, `<function-token>`, `<at-keyword-token>`,
//! `<hash-token>`, `<string-token>`, `<bad-string-token>`, `<url-token>`,
//! `<bad-url-token>`, `<delim-token>`, `<number-token>`, `<percentage-token>`,
//! `<dimension-token>`, `<unicode-range-token>`, `<whitespace-token>`,
//! `<CDO-token>`, `<CDC-token>`, `<colon-token>`, `<semicolon-token>`,
//! `<comma-token>`, `<[-token>`, `<]-token>`, `<(-token>`, `<)-token>`, `<{-token>`,
//! and `<}-token>`."

use core::fmt;

/// [§ 4.2 Definitions](https://www.w3.org/TR/css-syntax-3/#token-diagrams)
///
/// "A `<hash-token>` with the type flag set to 'id'... or 'unrestricted'."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashType {
    /// "id" - the hash token's value is a valid identifier
    Id,
    /// "unrestricted" - the hash token's value is not a valid identifier
    Unrestricted,
}

/// [§ 4.2 Definitions](https://www.w3.org/TR/css-syntax-3/#token-diagrams)
///
/// "A `<number-token>` has a type flag set to either 'integer' or 'number'."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericType {
    /// "integer" - the number is an integer
    Integer,
    /// "number" - the number has a decimal point or exponent
    Number,
}

/// [§ 4.2 Definitions](https://www.w3.org/TR/css-syntax-3/#token-diagrams)
///
/// CSS tokens as defined by the CSS Syntax Module Level 3 specification.
/// Each variant corresponds to a token type in the spec's railroad diagrams.
#[derive(Debug, Clone, PartialEq)]
pub enum CSSToken {
    /// "`<ident-token>`"
    /// "has a value composed of one or more code points"
    Ident(String),

    /// "`<function-token>`"
    /// "has a value composed of one or more code points, followed by U+0028 LEFT PARENTHESIS"
    Function(String),

    /// "`<at-keyword-token>`"
    /// "has a value composed of one or more code points, preceded by U+0040 COMMERCIAL AT (@)"
    AtKeyword(String),

    /// "`<hash-token>`"
    /// "has a value composed of one or more code points, preceded by U+0023 NUMBER SIGN (#)"
    /// "has a type flag set to either 'id' or 'unrestricted'"
    Hash {
        /// "a value composed of one or more code points"
        value: String,
        /// "a type flag set to either 'id' or 'unrestricted'"
        hash_type: HashType,
    },

    /// "`<string-token>`"
    /// "has a value composed of zero or more code points"
    String(String),

    /// "`<bad-string-token>`"
    /// "represents a parsing error"
    BadString,

    /// "`<url-token>`"
    /// "has a value composed of zero or more code points"
    Url(String),

    /// "`<bad-url-token>`"
    /// "represents a parsing error"
    BadUrl,

    /// "`<delim-token>`"
    /// "has a value composed of a single code point"
    Delim(char),

    /// "`<number-token>`"
    /// "has a numeric value, and a type flag set to either 'integer' or 'number'"
    Number {
        /// "a numeric value"
        value: f64,
        /// The integer value if this is an integer type.
        int_value: Option<i64>,
        /// "a type flag set to either 'integer' or 'number'"
        numeric_type: NumericType,
    },

    /// "`<percentage-token>`"
    /// "has a numeric value, and a type flag set to either 'integer' or 'number'"
    Percentage {
        /// "a numeric value"
        value: f64,
        /// The integer value if this is an integer type.
        int_value: Option<i64>,
        /// "a type flag set to either 'integer' or 'number'"
        numeric_type: NumericType,
    },

    /// "`<dimension-token>`"
    /// "has a numeric value, a type flag, and a unit"
    Dimension {
        /// "a numeric value"
        value: f64,
        /// The integer value if this is an integer type.
        int_value: Option<i64>,
        /// "a type flag set to either 'integer' or 'number'"
        numeric_type: NumericType,
        /// "a unit"
        unit: String,
    },

    /// "`<whitespace-token>`"
    /// "represents one or more whitespace code points"
    Whitespace,

    /// "`<CDO-token>`"
    /// "represents the character sequence U+003C U+0021 U+002D U+002D (<!--)"
    CDO,

    /// "`<CDC-token>`"
    /// "represents the character sequence U+002D U+002D U+003E (-->)"
    CDC,

    /// "`<colon-token>`"
    /// "represents U+003A COLON (:)"
    Colon,

    /// "`<semicolon-token>`"
    /// "represents U+003B SEMICOLON (;)"
    Semicolon,

    /// "`<comma-token>`"
    /// "represents U+002C COMMA (,)"
    Comma,

    /// `<[-token>`
    /// "represents U+005B LEFT SQUARE BRACKET ([)"
    LeftBracket,

    /// `<]-token>`
    /// "represents U+005D RIGHT SQUARE BRACKET (])"
    RightBracket,

    /// `<(-token>`
    /// "represents U+0028 LEFT PARENTHESIS (()"
    LeftParen,

    /// `<)-token>`
    /// "represents U+0029 RIGHT PARENTHESIS ())"
    RightParen,

    /// `<{-token>`
    /// "represents U+007B LEFT CURLY BRACKET ({)"
    LeftBrace,

    /// `<}-token>`
    /// "represents U+007D RIGHT CURLY BRACKET (})"
    RightBrace,

    /// End of file - signals end of input
    EOF,
}

impl CSSToken {
    /// Create a new ident token.
    #[must_use]
    pub fn ident(value: impl Into<String>) -> Self {
        Self::Ident(value.into())
    }

    /// Create a new function token.
    #[must_use]
    pub fn function(name: impl Into<String>) -> Self {
        Self::Function(name.into())
    }

    /// Create a new at-keyword token.
    #[must_use]
    pub fn at_keyword(value: impl Into<String>) -> Self {
        Self::AtKeyword(value.into())
    }

    /// Create a new hash token with id type.
    #[must_use]
    pub fn hash_id(value: impl Into<String>) -> Self {
        Self::Hash {
            value: value.into(),
            hash_type: HashType::Id,
        }
    }

    /// Create a new hash token with unrestricted type.
    #[must_use]
    pub fn hash_unrestricted(value: impl Into<String>) -> Self {
        Self::Hash {
            value: value.into(),
            hash_type: HashType::Unrestricted,
        }
    }

    /// Create a new string token.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Create a new number token (integer).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub const fn integer(value: i64) -> Self {
        Self::Number {
            value: value as f64,
            int_value: Some(value),
            numeric_type: NumericType::Integer,
        }
    }

    /// Create a new number token (float).
    #[must_use]
    pub const fn number(value: f64) -> Self {
        Self::Number {
            value,
            int_value: None,
            numeric_type: NumericType::Number,
        }
    }

    /// Create a new percentage token.
    #[must_use]
    pub const fn percentage(value: f64, int_value: Option<i64>) -> Self {
        Self::Percentage {
            value,
            int_value,
            numeric_type: if int_value.is_some() {
                NumericType::Integer
            } else {
                NumericType::Number
            },
        }
    }

    /// Create a new dimension token.
    #[must_use]
    pub fn dimension(value: f64, int_value: Option<i64>, unit: impl Into<String>) -> Self {
        Self::Dimension {
            value,
            int_value,
            numeric_type: if int_value.is_some() {
                NumericType::Integer
            } else {
                NumericType::Number
            },
            unit: unit.into(),
        }
    }

    /// Create a new delim token.
    #[must_use]
    pub const fn delim(c: char) -> Self {
        Self::Delim(c)
    }

    /// Create a new URL token.
    #[must_use]
    pub fn url(value: impl Into<String>) -> Self {
        Self::Url(value.into())
    }

    /// Returns true if this is an EOF token.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::EOF)
    }

    /// Returns true if this is a whitespace token.
    #[must_use]
    pub const fn is_whitespace(&self) -> bool {
        matches!(self, Self::Whitespace)
    }
}

impl fmt::Display for CSSToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ident(v) => write!(f, "<ident:{v}>"),
            Self::Function(v) => write!(f, "<function:{v}(>"),
            Self::AtKeyword(v) => write!(f, "<at-keyword:@{v}>"),
            Self::Hash { value, hash_type } => {
                let t = match hash_type {
                    HashType::Id => "id",
                    HashType::Unrestricted => "unrestricted",
                };
                write!(f, "<hash:#{value} ({t})>")
            }
            Self::String(v) => write!(f, "<string:\"{v}\">"),
            Self::BadString => write!(f, "<bad-string>"),
            Self::Url(v) => write!(f, "<url:{v}>"),
            Self::BadUrl => write!(f, "<bad-url>"),
            Self::Delim(c) => write!(f, "<delim:{c}>"),
            Self::Number { value, .. } => write!(f, "<number:{value}>"),
            Self::Percentage { value, .. } => write!(f, "<percentage:{value}%>"),
            Self::Dimension { value, unit, .. } => write!(f, "<dimension:{value}{unit}>"),
            Self::Whitespace => write!(f, "<whitespace>"),
            Self::CDO => write!(f, "<CDO>"),
            Self::CDC => write!(f, "<CDC>"),
            Self::Colon => write!(f, "<colon>"),
            Self::Semicolon => write!(f, "<semicolon>"),
            Self::Comma => write!(f, "<comma>"),
            Self::LeftBracket => write!(f, "<[>"),
            Self::RightBracket => write!(f, "<]>"),
            Self::LeftParen => write!(f, "<(>"),
            Self::RightParen => write!(f, "<)>"),
            Self::LeftBrace => write!(f, "<{{>"),
            Self::RightBrace => write!(f, "<}}>"),
            Self::EOF => write!(f, "<EOF>"),
        }
    }
}
