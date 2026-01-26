//! CSS Value types and parsing
//!
//! This module contains CSS value types for lengths, colors, and borders,
//! along with their parsing functions.
//!
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//! - [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)
//! - [CSS Backgrounds and Borders Level 3](https://www.w3.org/TR/css-backgrounds-3/)

use serde::Serialize;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;
use koala_common::warning::warn_once;

/// User agent default font size.
/// [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
/// "Initial: medium" - we define medium as 16px per common browser convention.
/// NOTE: This is user-agent defined. Browsers typically use 16px as the default.
pub const DEFAULT_FONT_SIZE_PX: f64 = 16.0;

// ─────────────────────────────────────────────────────────────────────────────
// Length Values
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Color Values
// ─────────────────────────────────────────────────────────────────────────────

/// [§ 4 Color syntax](https://www.w3.org/TR/css-color-4/#color-syntax)
/// sRGB color represented as RGBA components.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColorValue {
    /// "the red color channel" (0-255)
    pub r: u8,
    /// "the green color channel" (0-255)
    pub g: u8,
    /// "the blue color channel" (0-255)
    pub b: u8,
    /// "the alpha channel" (0-255, 255 = fully opaque)
    pub a: u8,
}

impl ColorValue {
    /// [§ 4.2 The RGB hexadecimal notations](https://www.w3.org/TR/css-color-4/#hex-notation)
    /// "The syntax of a <hex-color> is a <hash-token> token whose value consists of
    /// 3, 4, 6, or 8 hexadecimal digits."
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            // [§ 4.2.1]
            // "The three-digit RGB notation (#RGB) is converted into six-digit form (#RRGGBB)
            // by replicating digits, not by adding zeros."
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(ColorValue { r, g, b, a: 255 })
            }
            // Four-digit RGBA notation (#RGBA)
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                Some(ColorValue { r, g, b, a })
            }
            // Six-digit RGB notation (#RRGGBB)
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(ColorValue { r, g, b, a: 255 })
            }
            // Eight-digit RGBA notation (#RRGGBBAA)
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(ColorValue { r, g, b, a })
            }
            _ => None,
        }
    }

    /// [§ 6.1 Named Colors](https://www.w3.org/TR/css-color-4/#named-colors)
    /// "CSS defines a large set of named colors..."
    pub fn from_named(name: &str) -> Option<Self> {
        // MVP: Common colors from the named color table
        match name.to_ascii_lowercase().as_str() {
            "white" => Some(ColorValue {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }),
            "black" => Some(ColorValue {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }),
            "red" => Some(ColorValue {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            }),
            "green" => Some(ColorValue {
                r: 0,
                g: 128,
                b: 0,
                a: 255,
            }),
            "blue" => Some(ColorValue {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            }),
            "yellow" => Some(ColorValue {
                r: 255,
                g: 255,
                b: 0,
                a: 255,
            }),
            "gray" | "grey" => Some(ColorValue {
                r: 128,
                g: 128,
                b: 128,
                a: 255,
            }),
            "transparent" => Some(ColorValue {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            }),
            // Basic 16 HTML colors (completing the set)
            // [§ 6.1 Named Colors](https://www.w3.org/TR/css-color-4/#named-colors)
            "aqua" | "cyan" => Some(ColorValue {
                r: 0,
                g: 255,
                b: 255,
                a: 255,
            }),
            "fuchsia" | "magenta" => Some(ColorValue {
                r: 255,
                g: 0,
                b: 255,
                a: 255,
            }),
            "lime" => Some(ColorValue {
                r: 0,
                g: 255,
                b: 0,
                a: 255,
            }),
            "maroon" => Some(ColorValue {
                r: 128,
                g: 0,
                b: 0,
                a: 255,
            }),
            "navy" => Some(ColorValue {
                r: 0,
                g: 0,
                b: 128,
                a: 255,
            }),
            "olive" => Some(ColorValue {
                r: 128,
                g: 128,
                b: 0,
                a: 255,
            }),
            "purple" => Some(ColorValue {
                r: 128,
                g: 0,
                b: 128,
                a: 255,
            }),
            "silver" => Some(ColorValue {
                r: 192,
                g: 192,
                b: 192,
                a: 255,
            }),
            "teal" => Some(ColorValue {
                r: 0,
                g: 128,
                b: 128,
                a: 255,
            }),
            // TODO: Add extended color keywords (X11 colors, ~140 total)
            //   [§ 6.1](https://www.w3.org/TR/css-color-4/#named-colors)
            //   aliceblue, antiquewhite, aquamarine, azure, beige, bisque, ...
            //
            // STEP 3: Add system colors
            //   [§ 6.2 System Colors](https://www.w3.org/TR/css-color-4/#css-system-colors)
            //   Canvas, CanvasText, LinkText, VisitedText, ActiveText, ...
            //
            // Consider: Generate from a build script or use a lookup table
            _ => None,
        }
    }

    /// Convert to hex string notation (#RRGGBB or #RRGGBBAA if alpha != 255)
    ///
    /// [§ 4.2 The RGB hexadecimal notations](https://www.w3.org/TR/css-color-4/#hex-notation)
    pub fn to_hex_string(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Border Values
// ─────────────────────────────────────────────────────────────────────────────

/// [§ 4 Borders](https://www.w3.org/TR/css-backgrounds-3/#borders)
///
/// Border value representing width, style, and color.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BorderValue {
    /// [§ 4.3 'border-width'](https://www.w3.org/TR/css-backgrounds-3/#border-width)
    pub width: LengthValue,
    /// [§ 4.2 'border-style'](https://www.w3.org/TR/css-backgrounds-3/#border-style)
    pub style: String,
    /// [§ 4.1 'border-color'](https://www.w3.org/TR/css-backgrounds-3/#border-color)
    pub color: ColorValue,
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsing Functions
// ─────────────────────────────────────────────────────────────────────────────

/// [§ 4.2 RGB hex notation](https://www.w3.org/TR/css-color-4/#hex-notation)
/// Parse a color value from component values.
pub fn parse_color_value(values: &[ComponentValue]) -> Option<ColorValue> {
    for v in values {
        if let Some(color) = parse_single_color(v) {
            return Some(color);
        }
    }
    None
}

/// Parse a single component value as a color.
pub fn parse_single_color(v: &ComponentValue) -> Option<ColorValue> {
    match v {
        ComponentValue::Token(CSSToken::Hash { value, .. }) => ColorValue::from_hex(value),
        ComponentValue::Token(CSSToken::Ident(name)) => ColorValue::from_named(name),
        // TODO: Implement color functions
        // [§ 4 Representing Colors](https://www.w3.org/TR/css-color-4/#color-functions)
        //
        // STEP 1: Parse rgb() and rgba() functions
        // [§ 4.1 The RGB functions](https://www.w3.org/TR/css-color-4/#funcdef-rgb)
        // "rgb() = rgb( <percentage>{3} [ / <alpha-value> ]? ) |
        //          rgb( <number>{3} [ / <alpha-value> ]? )"
        // Examples: rgb(255, 0, 0), rgb(100% 0% 0%), rgb(255 0 0 / 50%)
        //
        // ComponentValue::Function { name, value } if name == "rgb" || name == "rgba" => {
        //     parse_rgb_function(value)
        // }
        //
        // STEP 2: Parse hsl() and hsla() functions
        // [§ 4.2 The HSL functions](https://www.w3.org/TR/css-color-4/#funcdef-hsl)
        // "hsl() = hsl( <hue> <percentage> <percentage> [ / <alpha-value> ]? )"
        // Examples: hsl(120, 100%, 50%), hsl(120deg 100% 50%)
        //
        // ComponentValue::Function { name, value } if name == "hsl" || name == "hsla" => {
        //     parse_hsl_function(value)
        // }
        //
        // STEP 3: Parse hwb() function
        // [§ 4.3 The HWB functions](https://www.w3.org/TR/css-color-4/#funcdef-hwb)
        //
        // STEP 4: Parse lab(), lch(), oklch() functions (future)
        // [§ 4.4-4.7](https://www.w3.org/TR/css-color-4/#lab-colors)
        _ => None,
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
///
/// "Lengths refer to distance measurements and are denoted by <length> in
/// the property definitions."
///
/// Parse a single component value as a <length>.
pub fn parse_single_length(v: &ComponentValue) -> Option<LengthValue> {
    match v {
        // [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
        //
        // "A dimension is a <number> immediately followed by a unit identifier."
        ComponentValue::Token(CSSToken::Dimension { value, unit, .. }) => {
            // [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
            // "1px = 1/96th of 1in"
            if unit.eq_ignore_ascii_case("px") {
                Some(LengthValue::Px(*value))
            }
            // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
            // "Equal to the computed value of the font-size property of the element"
            else if unit.eq_ignore_ascii_case("em") {
                Some(LengthValue::Em(*value))
            }
            // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
            // "1vw = 1% of viewport width"
            else if unit.eq_ignore_ascii_case("vw") {
                Some(LengthValue::Vw(*value))
            }
            // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
            // "1vh = 1% of viewport height"
            else if unit.eq_ignore_ascii_case("vh") {
                Some(LengthValue::Vh(*value))
            } else {
                warn_once(
                    "CSS",
                    &format!("unsupported unit '{unit}' in value {value}{unit}"),
                );
                None
            }
        }
        // [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
        // "0 can be written without a unit..."
        ComponentValue::Token(CSSToken::Number { value, .. }) if *value == 0.0 => {
            Some(LengthValue::Px(0.0))
        }
        _ => None,
    }
}

/// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
///
/// "Value: <margin-width>{1,4} | inherit"
/// "<margin-width> = <length> | <percentage> | auto"
///
/// Parse a value that can be either 'auto' or a length.
/// Used for margin properties where 'auto' has special meaning (centering).
pub fn parse_auto_length_value(values: &[ComponentValue]) -> Option<AutoLength> {
    // Iterate through component values to find the first valid <margin-width>
    for v in values {
        if let Some(al) = parse_single_auto_length(v) {
            return Some(al);
        }
    }
    None
}

/// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
///
/// "<margin-width> = <length> | <percentage> | auto"
///
/// Parse a single component value as a <margin-width>.
pub fn parse_single_auto_length(v: &ComponentValue) -> Option<AutoLength> {
    // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    //
    // "<margin-width> = <length> | <percentage> | auto"
    //
    // The grammar allows three types of values. We check them in order:

    // STEP 1: Check for 'auto' keyword.
    // [§ 4.4 Automatic values](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
    //
    // "Some properties can take the keyword 'auto' as a value."
    //
    // For margins, 'auto' has special meaning during layout:
    // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    // "If both 'margin-left' and 'margin-right' are 'auto', their used values
    // are equal. This horizontally centers the element..."
    if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
        if ident.eq_ignore_ascii_case("auto") {
            return Some(AutoLength::Auto);
        }
    }

    // STEP 2: Try to parse as a <length>.
    // [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
    //
    // "Lengths refer to distance measurements..."
    //
    // NOTE: <percentage> is not yet implemented (TODO).
    parse_single_length(v).map(AutoLength::Length)
}

/// Parse font-family value.
pub fn parse_font_family(values: &[ComponentValue]) -> Option<String> {
    // For MVP, just return the first ident or string
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
            // Unitless number (preferred)
            ComponentValue::Token(CSSToken::Number { value, .. }) => {
                return Some(*value);
            }
            // Length value - convert to ratio assuming 16px base
            ComponentValue::Token(CSSToken::Dimension { value, unit, .. })
                if unit.eq_ignore_ascii_case("px") =>
            {
                // TODO: This is a simplification - should use actual font-size
                return Some(*value / 16.0);
            }
            _ => {}
        }
    }
    None
}
