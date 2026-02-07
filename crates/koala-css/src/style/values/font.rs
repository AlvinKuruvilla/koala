//! CSS Font value parsing
//!
//! [CSS Fonts Module Level 4](https://www.w3.org/TR/css-fonts-4/)

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;

/// Parse font-family value.
#[must_use]
pub fn parse_font_family(values: &[ComponentValue]) -> Option<String> {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(name) | CSSToken::String(name)) = v {
            return Some(name.clone());
        }
    }
    None
}

/// [§ 4.2 `line-height`](https://www.w3.org/TR/css-inline-3/#line-height-property)
/// Parse `line-height` as a unitless number or length.
#[must_use]
pub fn parse_line_height(values: &[ComponentValue]) -> Option<f64> {
    for v in values {
        match v {
            ComponentValue::Token(CSSToken::Number { value, .. }) => {
                return Some(*value);
            }
            ComponentValue::Token(CSSToken::Dimension { value, unit, .. })
                if unit.eq_ignore_ascii_case("px") =>
            {
                return Some(*value / 16.0);
            }
            _ => {}
        }
    }
    None
}

// [§ 3.2 `font-weight`](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn parse_font_weight(values: &[ComponentValue]) -> Option<u16> {
    for v in values {
        match v {
            ComponentValue::Token(CSSToken::Ident(ident))
                if ident.eq_ignore_ascii_case("normal") =>
            {
                return Some(400);
            }
            ComponentValue::Token(CSSToken::Ident(ident)) if ident.eq_ignore_ascii_case("bold") => {
                return Some(700);
            }
            ComponentValue::Token(CSSToken::Number { value, .. }) => {
                let weight = *value as u16;
                if (100..=900).contains(&weight) && weight.is_multiple_of(100) {
                    return Some(weight);
                }
            }
            _ => {}
        }
    }
    None
}
