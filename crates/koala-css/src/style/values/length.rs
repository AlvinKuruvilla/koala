//! CSS Length values and parsing
//!
//! [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)

use serde::Serialize;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;
use koala_common::warning::warn_once;

/// User agent default font size.
/// [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
pub const DEFAULT_FONT_SIZE_PX: f64 = 16.0;

/// [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
/// "Lengths refer to distance measurements and are denoted by <length> in the
/// property definitions."
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum LengthValue {
    /// [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
    /// "1px = 1/96th of 1in"
    Px(f64),
    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    /// "Equal to the computed value of the font-size property of the element"
    Em(f64),
    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    /// "1vw = 1% of viewport width"
    Vw(f64),
    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    /// "1vh = 1% of viewport height"
    Vh(f64),
    // TODO: Implement additional length units:
    //
    // STEP 1: Add rem unit
    // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    // "Equal to the computed value of the font-size property of the root element."
    // Rem(f64),
    //
    // STEP 2: Add percentage values
    // [§ 4.3 Percentages](https://www.w3.org/TR/css-values-4/#percentages)
    // "A <percentage> value is denoted by <percentage>, and consists of a <number>
    // immediately followed by a percent sign '%'."
    // Percent(f64),
    //
    // STEP 3: Add calc() function support
    // [§ 8.1 calc()](https://www.w3.org/TR/css-values-4/#calc-notation)
    // "The calc() function allows mathematical expressions with addition (+),
    // subtraction (-), multiplication (*), division (/), and parentheses."
    // Calc(Box<CalcExpr>),
}

impl LengthValue {
    /// [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
    ///
    /// Get the value in pixels for non-viewport units.
    ///
    /// NOTE: For viewport units (vw, vh), this returns 0.0 as a fallback.
    /// Use `to_px_with_viewport()` instead when viewport dimensions are available.
    pub fn to_px(&self) -> f64 {
        match self {
            // [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
            LengthValue::Px(px) => *px,
            // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
            // "Equal to the computed value of the font-size property of the element"
            LengthValue::Em(em) => *em * DEFAULT_FONT_SIZE_PX,
            // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
            // Viewport units require viewport dimensions - return 0 as fallback.
            // The layout engine should use to_px_with_viewport() instead.
            LengthValue::Vw(_) | LengthValue::Vh(_) => 0.0,
        }
    }

    /// Get the value in pixels, resolving viewport units.
    ///
    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    /// "The viewport-percentage lengths are relative to the size of the
    /// initial containing block."
    pub fn to_px_with_viewport(&self, viewport_width: f64, viewport_height: f64) -> f64 {
        match self {
            LengthValue::Px(px) => *px,
            LengthValue::Em(em) => *em * DEFAULT_FONT_SIZE_PX,
            // "1vw = 1% of viewport width"
            LengthValue::Vw(vw) => *vw * viewport_width / 100.0,
            // "1vh = 1% of viewport height"
            LengthValue::Vh(vh) => *vh * viewport_height / 100.0,
        }
    }
}

/// [§ 4.4 Automatic values](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
///
/// "Some properties can take the keyword 'auto' as a value. This keyword
/// allows the user agent to compute the value based on other properties."
///
/// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
///
/// "Value: <margin-width>{1,4} | inherit"
/// "<margin-width> = <length> | <percentage> | auto"
///
/// This type represents a CSS value that can be either 'auto' or a specific length.
/// Used for properties like margin where 'auto' has special meaning.
///
/// [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
///
/// "If both 'margin-left' and 'margin-right' are 'auto', their used values
/// are equal. This horizontally centers the element with respect to the
/// edges of the containing block."
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AutoLength {
    /// [§ 4.4](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
    ///
    /// "The keyword 'auto'... allows the user agent to compute the value
    /// based on other properties."
    ///
    /// The value is 'auto' and will be resolved during layout.
    Auto,

    /// A specific length value (px, em, etc.).
    Length(LengthValue),
}

impl AutoLength {
    /// Check if the value is 'auto'.
    pub fn is_auto(&self) -> bool {
        matches!(self, AutoLength::Auto)
    }

    /// Get the length value in pixels, or 0.0 if 'auto'.
    ///
    /// NOTE: When 'auto', this returns 0.0 as a fallback. The actual
    /// resolved value depends on the layout algorithm (e.g., centering
    /// for `margin: auto`). See [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth).
    pub fn to_px(&self) -> f64 {
        match self {
            AutoLength::Auto => 0.0,
            AutoLength::Length(len) => len.to_px(),
        }
    }
}

/// [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
/// Parse a length value from component values.
pub fn parse_length_value(values: &[ComponentValue]) -> Option<LengthValue> {
    for v in values {
        if let Some(len) = parse_single_length(v) {
            return Some(len);
        }
    }
    None
}

/// [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
/// Parse a single component value as a <length>.
pub fn parse_single_length(v: &ComponentValue) -> Option<LengthValue> {
    match v {
        ComponentValue::Token(CSSToken::Dimension { value, unit, .. }) => {
            if unit.eq_ignore_ascii_case("px") {
                Some(LengthValue::Px(*value))
            } else if unit.eq_ignore_ascii_case("em") {
                Some(LengthValue::Em(*value))
            } else if unit.eq_ignore_ascii_case("vw") {
                Some(LengthValue::Vw(*value))
            } else if unit.eq_ignore_ascii_case("vh") {
                Some(LengthValue::Vh(*value))
            } else {
                warn_once("CSS", &format!("unsupported unit '{unit}'"));
                None
            }
        }
        ComponentValue::Token(CSSToken::Number { value, .. }) if *value == 0.0 => {
            Some(LengthValue::Px(0.0))
        }
        _ => None,
    }
}

/// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
/// Parse a value that can be either 'auto' or a length.
pub fn parse_auto_length_value(values: &[ComponentValue]) -> Option<AutoLength> {
    for v in values {
        if let Some(al) = parse_single_auto_length(v) {
            return Some(al);
        }
    }
    None
}

/// Parse a single component value as 'auto' or a length.
pub fn parse_single_auto_length(v: &ComponentValue) -> Option<AutoLength> {
    if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
        if ident.eq_ignore_ascii_case("auto") {
            return Some(AutoLength::Auto);
        }
    }
    parse_single_length(v).map(AutoLength::Length)
}
