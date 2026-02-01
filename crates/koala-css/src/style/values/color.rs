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
    /// Black (#000000)
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };

    /// White (#ffffff)
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

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
        ComponentValue::Function { name, value } => parse_color_function(name, value),
        _ => None,
    }
}

/// [§ 4.1 The RGB Functions: rgb() and rgba()](https://www.w3.org/TR/css-color-4/#rgb-functions)
/// [§ 4.1 The HSL Functions: hsl() and hsla()](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// "For legacy reasons, rgb() also supports an alternate syntax that
/// separates all of its arguments with commas."
///
/// "For legacy reasons, hsl() also supports an alternate syntax that
/// separates all of its arguments with commas."
///
/// Per CSS Color 4, rgb()/rgba() and hsl()/hsla() are aliases.
fn parse_color_function(name: &str, args: &[ComponentValue]) -> Option<ColorValue> {
    match name.to_ascii_lowercase().as_str() {
        "rgb" | "rgba" => parse_rgb_function(args),
        "hsl" | "hsla" => parse_hsl_function(args),
        _ => None,
    }
}

/// A numeric value extracted from a color function argument.
///
/// Color function arguments can be either plain numbers (0-255 for RGB)
/// or percentages (0%-100%).
#[derive(Debug, Clone, Copy)]
enum ColorArg {
    Number(f64),
    Percentage(f64),
}

/// Extract numeric arguments from a color function's component values,
/// skipping whitespace and commas.
///
/// Handles both modern syntax (space-separated with optional `/ alpha`)
/// and legacy syntax (comma-separated).
///
/// Returns up to 4 arguments: the 3 color channels and an optional alpha.
fn extract_color_args(args: &[ComponentValue]) -> Vec<ColorArg> {
    let mut result = Vec::new();
    let mut saw_slash = false;

    for arg in args {
        match arg {
            // Plain number
            ComponentValue::Token(CSSToken::Number { value, .. }) => {
                result.push(ColorArg::Number(*value));
            }
            // Percentage
            ComponentValue::Token(CSSToken::Percentage { value, .. }) => {
                result.push(ColorArg::Percentage(*value));
            }
            // [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
            //
            // "/ <alpha-value>" — the slash separator before alpha in
            // modern syntax. We just skip it; the next number is alpha.
            ComponentValue::Token(CSSToken::Delim('/')) => {
                saw_slash = true;
            }
            // Skip whitespace and commas (both legacy and modern syntax)
            ComponentValue::Token(CSSToken::Whitespace | CSSToken::Comma) => {}
            _ => {}
        }
    }

    // If we saw a slash but only got 3 args, that means the alpha
    // was already captured as the 4th element. The `saw_slash` flag
    // is informational only (both syntaxes produce the same result).
    let _ = saw_slash;

    result
}

/// [§ 4.1 The RGB Functions](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// "rgb() = rgb( <percentage>{3} [ / <alpha-value> ]? ) |
///          rgb( <number>{3} [ / <alpha-value> ]? )"
///
/// Legacy: "rgb( <percentage>#{3} , <alpha-value>? ) |
///          rgb( <number>#{3} , <alpha-value>? )"
///
/// "Values outside these ranges are not invalid, but are clamped to the
/// ranges defined here at parsed-value time."
fn parse_rgb_function(args: &[ComponentValue]) -> Option<ColorValue> {
    let vals = extract_color_args(args);
    if vals.len() < 3 {
        return None;
    }

    let r = color_channel_to_u8(vals[0]);
    let g = color_channel_to_u8(vals[1]);
    let b = color_channel_to_u8(vals[2]);

    // [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
    //
    // "The final argument, <alpha-value>, specifies the alpha of the color."
    // "If omitted, it defaults to 100%."
    let a = if vals.len() >= 4 {
        alpha_to_u8(vals[3])
    } else {
        255
    };

    Some(ColorValue { r, g, b, a })
}

/// [§ 4.1 The HSL Functions](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// "hsl() = hsl( <hue> <percentage> <percentage> [ / <alpha-value> ]? )"
///
/// Legacy: "hsl( <hue>, <percentage>, <percentage>, <alpha-value>? )"
///
/// "<hue> is a <number> or <angle>, interpreted as degrees."
fn parse_hsl_function(args: &[ComponentValue]) -> Option<ColorValue> {
    let vals = extract_color_args(args);
    if vals.len() < 3 {
        return None;
    }

    // [§ 4.1](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
    //
    // "The first argument specifies the hue angle."
    // "Because this value is so often given in degrees, the argument
    // can also be given as a number, which is interpreted as degrees."
    let hue = match vals[0] {
        ColorArg::Number(v) => v,
        ColorArg::Percentage(v) => v * 3.6, // 100% = 360 degrees
    };

    // "The second argument is the saturation... interpreted as a percentage."
    let saturation = match vals[1] {
        ColorArg::Percentage(v) => v / 100.0,
        ColorArg::Number(v) => v / 100.0,
    };

    // "The third argument is the lightness... interpreted as a percentage."
    let lightness = match vals[2] {
        ColorArg::Percentage(v) => v / 100.0,
        ColorArg::Number(v) => v / 100.0,
    };

    let a = if vals.len() >= 4 {
        alpha_to_u8(vals[3])
    } else {
        255
    };

    let (r, g, b) = hsl_to_rgb(hue, saturation, lightness);
    Some(ColorValue { r, g, b, a })
}

/// Convert a color channel argument to a u8 (0-255).
///
/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// "Values outside these ranges are not invalid, but are clamped."
///
/// Numbers are clamped to 0-255; percentages map 0%-100% to 0-255.
fn color_channel_to_u8(arg: ColorArg) -> u8 {
    let v = match arg {
        ColorArg::Number(n) => n,
        // "100% = 255"
        ColorArg::Percentage(p) => p * 255.0 / 100.0,
    };
    v.round().clamp(0.0, 255.0) as u8
}

/// Convert an alpha argument to a u8 (0-255).
///
/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// "The <alpha-value> can be a <number> (clamped to [0, 1]) or a
/// <percentage> (clamped to [0%, 100%])."
fn alpha_to_u8(arg: ColorArg) -> u8 {
    let v = match arg {
        // Numbers: 0.0 = transparent, 1.0 = opaque
        ColorArg::Number(n) => n * 255.0,
        // Percentages: 0% = transparent, 100% = opaque
        ColorArg::Percentage(p) => p * 255.0 / 100.0,
    };
    v.round().clamp(0.0, 255.0) as u8
}

/// [§ 4.2.4 HSL-to-RGB](https://www.w3.org/TR/css-color-4/#hsl-to-rgb)
///
/// Convert HSL color to RGB.
///
/// - hue: angle in degrees (0-360, wraps)
/// - saturation: 0.0-1.0
/// - lightness: 0.0-1.0
///
/// Returns (r, g, b) as u8 values.
fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> (u8, u8, u8) {
    // Normalize hue to [0, 360)
    let h = ((hue % 360.0) + 360.0) % 360.0;
    let s = saturation.clamp(0.0, 1.0);
    let l = lightness.clamp(0.0, 1.0);

    // [§ 4.2.4](https://www.w3.org/TR/css-color-4/#hsl-to-rgb)
    //
    // "HOW TO RETURN hsl.h, hsl.s, hsl.l converted to an idealized-rgb color"
    //
    // Standard algorithm using chroma and intermediate value.
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());

    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    let m = l - c / 2.0;
    let to_u8 = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;

    (to_u8(r1), to_u8(g1), to_u8(b1))
}
