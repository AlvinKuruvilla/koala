//! CSS Font value parsing
//!
//! [CSS Fonts Module Level 4](https://www.w3.org/TR/css-fonts-4/)

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;

/// Parse font-family value.
pub fn parse_font_family(values: &[ComponentValue]) -> Option<String> {
    for v in values {
        match v {
            ComponentValue::Token(CSSToken::Ident(name)) => {
                return Some(name.clone());
            }
            ComponentValue::Token(CSSToken::String(name)) => {
                return Some(name.clone());
            }
            _ => {}
        }
    }
    None
}

/// [§ 4.2 line-height](https://www.w3.org/TR/css-inline-3/#line-height-property)
/// Parse line-height as a unitless number or length.
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

// TODO: Add parse_font_weight()
// [§ 3.2 font-weight](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
