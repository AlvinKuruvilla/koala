//! CSS Color values and parsing
//!
//! [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)

use serde::Serialize;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;

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
        // TODO: Implement color functions (rgb(), hsl(), etc.)
        _ => None,
    }
}
