//! Small primitives that hide the syntactic warts of `for v in values
//! { match v { … } }` inside CSS property parsers.
//!
//! Every property parser previously rewrote the same dance:
//!
//! ```ignore
//! for v in values {
//!     match v {
//!         ComponentValue::Token(CSSToken::Ident(ident))
//!             if ident.eq_ignore_ascii_case("normal") => { … }
//!         ComponentValue::Token(CSSToken::Dimension { value, unit, .. })
//!             if unit.eq_ignore_ascii_case("px") => { … }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! The dance has four warts: struct-variant destructuring, the
//! case-insensitive comparison, the deref-and-cast from `*value as
//! f32`, and the outer `for/find` loop. Each helper here hides one
//! of them in exactly one place. Per-property parsers then read like
//! the CSS spec rather than like Rust pattern matching:
//!
//! ```ignore
//! if contains_keyword(values, "normal") { return Some(0.0); }
//! first_px_length(values)
//! ```
//!
//! Helpers all take `&[ComponentValue]` because that's the shape
//! every property parser already has on hand. The scan order is
//! "first matching token wins" — properties whose grammar genuinely
//! depends on multi-token sequences (font shorthand, grid lines)
//! still hand-roll their own loop and don't reach for these.

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;

/// Returns `true` if `values` contains an ident token equal to `name`
/// (case-insensitive). Used for keyword-only properties or for the
/// keyword arm of a `keyword | <length>` grammar.
#[must_use]
pub fn contains_keyword(values: &[ComponentValue], name: &str) -> bool {
    values.iter().any(|v| {
        matches!(
            v,
            ComponentValue::Token(CSSToken::Ident(ident))
            if ident.eq_ignore_ascii_case(name)
        )
    })
}

/// Returns the first ident in `values` that matches one of `names`
/// (case-insensitive). The returned `&str` is from `names`, not from
/// the parsed value — so callers get back a canonical form they
/// control even when the CSS source uses unusual casing.
///
/// Useful for properties whose grammar is a small set of named
/// keywords (`text-align`, `text-transform`, `white-space`, …).
#[must_use]
pub fn first_keyword<'a>(values: &[ComponentValue], names: &[&'a str]) -> Option<&'a str> {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
            if let Some(&name) = names.iter().find(|n| ident.eq_ignore_ascii_case(n)) {
                return Some(name);
            }
        }
    }
    None
}

/// Returns the first `<dimension-token>` in `values` whose unit is
/// `px` (case-insensitive), coerced to `f32`. `em`, `%`, and other
/// units return `None` — the caller decides how to handle them
/// (today, most callers treat anything but `px` as unsupported).
///
/// This hides the four-line struct-variant guard + dereference cast
/// every length parser was writing inline.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn first_px_length(values: &[ComponentValue]) -> Option<f32> {
    values.iter().find_map(|v| match v {
        ComponentValue::Token(CSSToken::Dimension { value, unit, .. })
            if unit.eq_ignore_ascii_case("px") =>
        {
            Some(*value as f32)
        }
        _ => None,
    })
}

/// Returns the first unitless `<number-token>` in `values`, coerced
/// to `f32`. Used by `flex-grow`, `flex-shrink`, `opacity`, and the
/// occasional property that accepts a bare number.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn first_number(values: &[ComponentValue]) -> Option<f32> {
    values.iter().find_map(|v| match v {
        ComponentValue::Token(CSSToken::Number { value, .. }) => Some(*value as f32),
        _ => None,
    })
}

/// Returns the first `<percentage-token>` in `values` as its raw
/// percentage value (e.g. `50%` → `50.0`, not `0.5`). Callers that
/// want a 0..1 fraction divide on their own.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn first_percentage(values: &[ComponentValue]) -> Option<f32> {
    values.iter().find_map(|v| match v {
        ComponentValue::Token(CSSToken::Percentage { value, .. }) => Some(*value as f32),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the value-extraction helpers.
    //!
    //! Each helper has at minimum: one positive case (the token it
    //! recognises), one negative case (a token it shouldn't match),
    //! and one case-insensitivity check where the contract calls for
    //! it. Two-helpers' worth of mistakes propagate to every property
    //! parser that calls them, so pinning the contract here is worth
    //! the keystrokes.
    use super::*;
    use crate::tokenizer::NumericType;

    fn ident(name: &str) -> ComponentValue {
        ComponentValue::Token(CSSToken::Ident(name.to_owned()))
    }

    fn px(value: f64) -> ComponentValue {
        ComponentValue::Token(CSSToken::Dimension {
            value,
            int_value: None,
            unit: "px".to_owned(),
            numeric_type: NumericType::Number,
        })
    }

    fn em(value: f64) -> ComponentValue {
        ComponentValue::Token(CSSToken::Dimension {
            value,
            int_value: None,
            unit: "em".to_owned(),
            numeric_type: NumericType::Number,
        })
    }

    fn number(value: f64) -> ComponentValue {
        ComponentValue::Token(CSSToken::Number {
            value,
            int_value: None,
            numeric_type: NumericType::Number,
        })
    }

    fn pct(value: f64) -> ComponentValue {
        ComponentValue::Token(CSSToken::Percentage {
            value,
            int_value: None,
            numeric_type: NumericType::Number,
        })
    }

    #[test]
    fn contains_keyword_matches_case_insensitively() {
        let values = [ident("NoRmAl")];
        assert!(contains_keyword(&values, "normal"));
    }

    #[test]
    fn contains_keyword_rejects_dimensions_and_numbers() {
        let values = [px(2.0), number(0.0)];
        assert!(!contains_keyword(&values, "normal"));
    }

    #[test]
    fn contains_keyword_misses_when_name_doesnt_appear() {
        let values = [ident("wide")];
        assert!(!contains_keyword(&values, "normal"));
    }

    #[test]
    fn first_keyword_returns_canonical_form_from_names_list() {
        let values = [ident("UPPERCASE")];
        // The returned &str is the one we passed in, not the one we
        // parsed — callers get back their canonical casing regardless
        // of how the CSS author wrote it.
        assert_eq!(first_keyword(&values, &["uppercase", "lowercase"]), Some("uppercase"));
    }

    #[test]
    fn first_keyword_returns_first_match_in_values_order() {
        // Two idents in values; both are in the allow-list. Order
        // matches the values' order so the parser's "first wins"
        // semantic is preserved.
        let values = [ident("center"), ident("left")];
        assert_eq!(first_keyword(&values, &["left", "center"]), Some("center"));
    }

    #[test]
    fn first_keyword_misses_when_no_value_matches_allow_list() {
        let values = [ident("justify")];
        assert_eq!(first_keyword(&values, &["left", "right"]), None);
    }

    #[test]
    fn first_px_length_recognises_px_dimension() {
        let values = [px(12.0)];
        assert_eq!(first_px_length(&values), Some(12.0));
    }

    #[test]
    fn first_px_length_rejects_other_units() {
        let values = [em(1.5)];
        assert_eq!(first_px_length(&values), None);
    }

    #[test]
    fn first_px_length_unit_match_is_case_insensitive() {
        let values = [ComponentValue::Token(CSSToken::Dimension {
            value: 4.0,
            int_value: None,
            unit: "PX".to_owned(),
            numeric_type: NumericType::Number,
        })];
        assert_eq!(first_px_length(&values), Some(4.0));
    }

    #[test]
    fn first_number_recognises_bare_number() {
        let values = [number(1.5)];
        assert_eq!(first_number(&values), Some(1.5));
    }

    #[test]
    fn first_number_does_not_match_dimensions() {
        // `2px` is a Dimension, not a Number — the helper must say
        // None or callers that want "bare numbers" (flex-grow,
        // opacity) would silently accept lengths.
        let values = [px(2.0)];
        assert_eq!(first_number(&values), None);
    }

    #[test]
    fn first_percentage_returns_raw_percent() {
        // `50%` is `50.0`, not `0.5`. Callers that want a fraction
        // divide on their own.
        let values = [pct(50.0)];
        assert_eq!(first_percentage(&values), Some(50.0));
    }

    #[test]
    fn first_percentage_does_not_match_numbers_or_dimensions() {
        let values = [number(50.0), px(50.0)];
        assert_eq!(first_percentage(&values), None);
    }
}
