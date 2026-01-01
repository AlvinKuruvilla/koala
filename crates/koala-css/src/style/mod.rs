//! CSS Computed Style representation and value parsing
//!
//! This module implements CSS value types and computed style representation per:
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//! - [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)

use serde::Serialize;

use crate::parser::{ComponentValue, Declaration};
use crate::tokenizer::CSSToken;

/// [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
/// "Lengths refer to distance measurements and are denoted by <length> in the
/// property definitions."
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum LengthValue {
    /// [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
    /// "1px = 1/96th of 1in"
    Px(f64),
    // TODO: Em, Rem, Percent for future work
}

impl LengthValue {
    /// Get the value in pixels.
    pub fn to_px(&self) -> f64 {
        match self {
            LengthValue::Px(px) => *px,
        }
    }
}

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
            // TODO: Add more named colors as needed
            _ => None,
        }
    }
}

/// [CSS Backgrounds and Borders § 4 Borders](https://www.w3.org/TR/css-backgrounds-3/#borders)
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

/// Computed styles for an element.
///
/// [CSS Cascading Level 4 § 4.4 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
/// "The computed value is the result of resolving the specified value..."
///
/// All values are Option - None means "not set" (use inherited or initial value).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ComputedStyle {
    /// [CSS Color § 3.1 'color'](https://www.w3.org/TR/css-color-4/#the-color-property)
    pub color: Option<ColorValue>,
    /// [CSS Fonts § 3.1 'font-family'](https://www.w3.org/TR/css-fonts-4/#font-family-prop)
    pub font_family: Option<String>,
    /// [CSS Fonts § 3.5 'font-size'](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
    pub font_size: Option<LengthValue>,
    /// [CSS Inline § 4.2 'line-height'](https://www.w3.org/TR/css-inline-3/#line-height-property)
    pub line_height: Option<f64>,

    /// [CSS Backgrounds § 3.2 'background-color'](https://www.w3.org/TR/css-backgrounds-3/#background-color)
    pub background_color: Option<ColorValue>,

    /// [CSS Box § 6.1 'margin-top'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_top: Option<LengthValue>,
    /// [CSS Box § 6.1 'margin-right'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_right: Option<LengthValue>,
    /// [CSS Box § 6.1 'margin-bottom'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_bottom: Option<LengthValue>,
    /// [CSS Box § 6.1 'margin-left'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_left: Option<LengthValue>,

    /// [CSS Box § 6.2 'padding-top'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_top: Option<LengthValue>,
    /// [CSS Box § 6.2 'padding-right'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_right: Option<LengthValue>,
    /// [CSS Box § 6.2 'padding-bottom'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_bottom: Option<LengthValue>,
    /// [CSS Box § 6.2 'padding-left'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_left: Option<LengthValue>,

    /// [CSS Backgrounds § 4 'border-top'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_top: Option<BorderValue>,
    /// [CSS Backgrounds § 4 'border-right'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_right: Option<BorderValue>,
    /// [CSS Backgrounds § 4 'border-bottom'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_bottom: Option<BorderValue>,
    /// [CSS Backgrounds § 4 'border-left'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_left: Option<BorderValue>,
}

impl ComputedStyle {
    /// Apply a CSS declaration to update this computed style.
    pub fn apply_declaration(&mut self, decl: &Declaration) {
        match decl.name.to_ascii_lowercase().as_str() {
            "color" => {
                if let Some(color) = parse_color_value(&decl.value) {
                    self.color = Some(color);
                }
            }
            "background-color" => {
                if let Some(color) = parse_color_value(&decl.value) {
                    self.background_color = Some(color);
                }
            }
            "font-size" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.font_size = Some(len);
                }
            }
            "font-family" => {
                if let Some(family) = parse_font_family(&decl.value) {
                    self.font_family = Some(family);
                }
            }
            "line-height" => {
                if let Some(lh) = parse_line_height(&decl.value) {
                    self.line_height = Some(lh);
                }
            }
            // [§ 9.2 Shorthand properties](https://www.w3.org/TR/css-cascade-4/#shorthand)
            "margin" => {
                self.apply_margin_shorthand(&decl.value);
            }
            "margin-top" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_top = Some(len);
                }
            }
            "margin-right" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_right = Some(len);
                }
            }
            "margin-bottom" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_bottom = Some(len);
                }
            }
            "margin-left" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_left = Some(len);
                }
            }
            "padding" => {
                self.apply_padding_shorthand(&decl.value);
            }
            "padding-top" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_top = Some(len);
                }
            }
            "padding-right" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_right = Some(len);
                }
            }
            "padding-bottom" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_bottom = Some(len);
                }
            }
            "padding-left" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_left = Some(len);
                }
            }
            "border" => {
                self.apply_border_shorthand(&decl.value);
            }
            _ => {
                // Unknown property - ignore for MVP
            }
        }
    }

    /// [CSS Box Model § 6.3 Margins](https://www.w3.org/TR/css-box-4/#margins)
    /// The margin shorthand sets margin-top, margin-right, margin-bottom, margin-left.
    fn apply_margin_shorthand(&mut self, values: &[ComponentValue]) {
        let lengths: Vec<LengthValue> = values
            .iter()
            .filter_map(parse_single_length)
            .collect();

        match lengths.len() {
            1 => {
                // All sides same
                self.margin_top = Some(lengths[0].clone());
                self.margin_right = Some(lengths[0].clone());
                self.margin_bottom = Some(lengths[0].clone());
                self.margin_left = Some(lengths[0].clone());
            }
            2 => {
                // vertical | horizontal
                self.margin_top = Some(lengths[0].clone());
                self.margin_bottom = Some(lengths[0].clone());
                self.margin_right = Some(lengths[1].clone());
                self.margin_left = Some(lengths[1].clone());
            }
            3 => {
                // top | horizontal | bottom
                self.margin_top = Some(lengths[0].clone());
                self.margin_right = Some(lengths[1].clone());
                self.margin_left = Some(lengths[1].clone());
                self.margin_bottom = Some(lengths[2].clone());
            }
            4 => {
                // top | right | bottom | left
                self.margin_top = Some(lengths[0].clone());
                self.margin_right = Some(lengths[1].clone());
                self.margin_bottom = Some(lengths[2].clone());
                self.margin_left = Some(lengths[3].clone());
            }
            _ => {}
        }
    }

    /// [CSS Box Model § 6.2 Padding](https://www.w3.org/TR/css-box-4/#paddings)
    fn apply_padding_shorthand(&mut self, values: &[ComponentValue]) {
        let lengths: Vec<LengthValue> = values
            .iter()
            .filter_map(parse_single_length)
            .collect();

        match lengths.len() {
            1 => {
                self.padding_top = Some(lengths[0].clone());
                self.padding_right = Some(lengths[0].clone());
                self.padding_bottom = Some(lengths[0].clone());
                self.padding_left = Some(lengths[0].clone());
            }
            2 => {
                self.padding_top = Some(lengths[0].clone());
                self.padding_bottom = Some(lengths[0].clone());
                self.padding_right = Some(lengths[1].clone());
                self.padding_left = Some(lengths[1].clone());
            }
            3 => {
                self.padding_top = Some(lengths[0].clone());
                self.padding_right = Some(lengths[1].clone());
                self.padding_left = Some(lengths[1].clone());
                self.padding_bottom = Some(lengths[2].clone());
            }
            4 => {
                self.padding_top = Some(lengths[0].clone());
                self.padding_right = Some(lengths[1].clone());
                self.padding_bottom = Some(lengths[2].clone());
                self.padding_left = Some(lengths[3].clone());
            }
            _ => {}
        }
    }

    /// [CSS Backgrounds § 3.1 border shorthand](https://www.w3.org/TR/css-backgrounds-3/#the-border-shorthands)
    /// "border: 1px solid #ddd" sets all four borders
    fn apply_border_shorthand(&mut self, values: &[ComponentValue]) {
        let mut width: Option<LengthValue> = None;
        let mut style: Option<String> = None;
        let mut color: Option<ColorValue> = None;

        for v in values {
            // Try to parse as length (width)
            if width.is_none() {
                if let Some(len) = parse_single_length(v) {
                    width = Some(len);
                    continue;
                }
            }

            // Try to parse as color
            if color.is_none() {
                if let Some(c) = parse_single_color(v) {
                    color = Some(c);
                    continue;
                }
            }

            // Try to parse as style keyword
            if style.is_none() {
                if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(
                        lower.as_str(),
                        "solid" | "dashed" | "dotted" | "double" | "none" | "hidden"
                    ) {
                        style = Some(lower);
                        continue;
                    }
                }
            }
        }

        // Build border value if we have at least a width
        if let Some(w) = width {
            let border = BorderValue {
                width: w,
                style: style.unwrap_or_else(|| "solid".to_string()),
                color: color.unwrap_or(ColorValue {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                }),
            };
            self.border_top = Some(border.clone());
            self.border_right = Some(border.clone());
            self.border_bottom = Some(border.clone());
            self.border_left = Some(border);
        }
    }
}

/// [§ 4.2 RGB hex notation](https://www.w3.org/TR/css-color-4/#hex-notation)
/// Parse a color value from component values.
fn parse_color_value(values: &[ComponentValue]) -> Option<ColorValue> {
    for v in values {
        if let Some(color) = parse_single_color(v) {
            return Some(color);
        }
    }
    None
}

/// Parse a single component value as a color.
fn parse_single_color(v: &ComponentValue) -> Option<ColorValue> {
    match v {
        ComponentValue::Token(CSSToken::Hash { value, .. }) => ColorValue::from_hex(value),
        ComponentValue::Token(CSSToken::Ident(name)) => ColorValue::from_named(name),
        // TODO: rgb(), rgba(), hsl() functions
        _ => None,
    }
}

/// [§ 4.1 Lengths](https://www.w3.org/TR/css-values-4/#lengths)
/// Parse a length value from component values.
fn parse_length_value(values: &[ComponentValue]) -> Option<LengthValue> {
    for v in values {
        if let Some(len) = parse_single_length(v) {
            return Some(len);
        }
    }
    None
}

/// Parse a single component value as a length.
fn parse_single_length(v: &ComponentValue) -> Option<LengthValue> {
    match v {
        ComponentValue::Token(CSSToken::Dimension { value, unit, .. }) => {
            if unit.eq_ignore_ascii_case("px") {
                Some(LengthValue::Px(*value))
            } else {
                // TODO: other units (em, rem, %)
                None
            }
        }
        // [§ 4.1.1](https://www.w3.org/TR/css-values-4/#lengths)
        // "0 can be written without a unit..."
        ComponentValue::Token(CSSToken::Number { value, .. }) if *value == 0.0 => {
            Some(LengthValue::Px(0.0))
        }
        _ => None,
    }
}

/// Parse font-family value.
fn parse_font_family(values: &[ComponentValue]) -> Option<String> {
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

/// [CSS Line Layout § 4.2 line-height](https://www.w3.org/TR/css-inline-3/#line-height-property)
/// Parse line-height as a unitless number or length.
fn parse_line_height(values: &[ComponentValue]) -> Option<f64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex_6() {
        let color = ColorValue::from_hex("#ff0000").unwrap();
        assert_eq!(color, ColorValue { r: 255, g: 0, b: 0, a: 255 });
    }

    #[test]
    fn test_color_from_hex_3() {
        let color = ColorValue::from_hex("#f00").unwrap();
        assert_eq!(color, ColorValue { r: 255, g: 0, b: 0, a: 255 });
    }

    #[test]
    fn test_color_from_hex_mixed_case() {
        let color = ColorValue::from_hex("#FfA500").unwrap();
        assert_eq!(color, ColorValue { r: 255, g: 165, b: 0, a: 255 });
    }

    #[test]
    fn test_color_from_hex_without_hash() {
        let color = ColorValue::from_hex("00ff00").unwrap();
        assert_eq!(color, ColorValue { r: 0, g: 255, b: 0, a: 255 });
    }

    #[test]
    fn test_color_from_named() {
        assert_eq!(
            ColorValue::from_named("white"),
            Some(ColorValue { r: 255, g: 255, b: 255, a: 255 })
        );
        assert_eq!(
            ColorValue::from_named("BLACK"),
            Some(ColorValue { r: 0, g: 0, b: 0, a: 255 })
        );
        assert_eq!(ColorValue::from_named("unknown"), None);
    }

    #[test]
    fn test_length_px() {
        let len = LengthValue::Px(16.0);
        assert_eq!(len.to_px(), 16.0);
    }
}
