//! CSS Computed Style representation and value parsing
//!
//! This module implements CSS value types and computed style representation per:
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//! - [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)
//! - [CSS Display Module Level 3](https://www.w3.org/TR/css-display-3/)

use serde::Serialize;

// [§ 2 Box Layout Modes: the display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
//
// "The display property defines an element's display type, which consists of
// the two basic qualities of how an element generates boxes:
//   - the inner display type, which defines the kind of formatting context
//     it generates, dictating how its descendant boxes are laid out.
//   - the outer display type, which dictates how the principal box itself
//     participates in flow layout."

/// [§ 2.1 Outer Display Roles](https://www.w3.org/TR/css-display-3/#outer-role)
///
/// "The <display-outside> keywords specify the element's outer display type,
/// which is essentially its principal box's role in flow layout."
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum OuterDisplayType {
    /// "The element generates a block-level box when placed in flow layout."
    Block,
    /// "The element generates an inline-level box when placed in flow layout."
    Inline,
    /// "The element generates a run-in box, which is a type of inline-level box."
    RunIn,
}

/// [§ 2.2 Inner Display Layout Models](https://www.w3.org/TR/css-display-3/#inner-model)
///
/// "The <display-inside> keywords specify the element's inner display type,
/// which defines the type of formatting context that lays out its contents."
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum InnerDisplayType {
    /// "The element lays out its contents using flow layout (block-and-inline layout)."
    Flow,
    /// "The element lays out its contents using flow layout (block-and-inline layout)."
    /// Same as Flow but establishes a new block formatting context.
    FlowRoot,
    /// "The element lays out its contents using table layout."
    Table,
    /// "The element lays out its contents using flex layout."
    Flex,
    /// "The element lays out its contents using grid layout."
    Grid,
}

/// Combined display value
/// [§ 2 Box Layout Modes](https://www.w3.org/TR/css-display-3/#the-display-properties)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct DisplayValue {
    /// "The outer display type, which dictates how the box participates in flow layout."
    pub outer: OuterDisplayType,
    /// "The inner display type, which dictates how its descendant boxes are laid out."
    pub inner: InnerDisplayType,
}

impl DisplayValue {
    /// `display: block` - block outer, flow inner
    pub fn block() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Flow,
        }
    }

    /// `display: inline` - inline outer, flow inner
    pub fn inline() -> Self {
        Self {
            outer: OuterDisplayType::Inline,
            inner: InnerDisplayType::Flow,
        }
    }

    /// `display: inline-block` - inline outer, flow-root inner
    pub fn inline_block() -> Self {
        Self {
            outer: OuterDisplayType::Inline,
            inner: InnerDisplayType::FlowRoot,
        }
    }

    /// `display: flex` - block outer, flex inner
    pub fn flex() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Flex,
        }
    }

    /// `display: grid` - block outer, grid inner
    pub fn grid() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Grid,
        }
    }
}

use crate::parser::{ComponentValue, Declaration};
use crate::tokenizer::CSSToken;
use koala_common::warning::warn_once;
/// User agent default font size.
/// [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
/// "Initial: medium" - we define medium as 16px per common browser convention.
/// TODO: If this is user-agent defined we eed to parse this from the user agent at some point I would imagine
const DEFAULT_FONT_SIZE_PX: f64 = 16.0;
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
    // TODO: Rem, Percent for future work
}

impl LengthValue {
    /// Get the value in pixels.
    pub fn to_px(&self) -> f64 {
        match self {
            LengthValue::Px(px) => *px,
            LengthValue::Em(em) => *em * DEFAULT_FONT_SIZE_PX, // assuming 1em = default font size
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

/// Computed styles for an element.
///
/// [§ 4.4 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
/// "The computed value is the result of resolving the specified value..."
///
/// All values are Option - None means "not set" (use inherited or initial value).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ComputedStyle {
    /// [§ 2 'display'](https://www.w3.org/TR/css-display-3/#the-display-properties)
    ///
    /// "The display property defines an element's display type, which consists of
    /// the two basic qualities of how an element generates boxes."
    ///
    /// None means use the element's default display value.
    pub display: Option<DisplayValue>,

    /// [§ 2.6 display: none](https://www.w3.org/TR/css-display-3/#valdef-display-none)
    ///
    /// "The element and its descendants generate no boxes or text runs."
    ///
    /// This is tracked separately because `display: none` is fundamentally different
    /// from other display values - it prevents box generation entirely.
    #[serde(default)]
    pub display_none: bool,

    /// [§ 3.1 'color'](https://www.w3.org/TR/css-color-4/#the-color-property)
    pub color: Option<ColorValue>,
    /// [§ 3.1 'font-family'](https://www.w3.org/TR/css-fonts-4/#font-family-prop)
    pub font_family: Option<String>,
    /// [§ 3.5 'font-size'](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
    pub font_size: Option<LengthValue>,
    /// [§ 4.2 'line-height'](https://www.w3.org/TR/css-inline-3/#line-height-property)
    pub line_height: Option<f64>,

    /// [§ 3.2 'background-color'](https://www.w3.org/TR/css-backgrounds-3/#background-color)
    pub background_color: Option<ColorValue>,

    /// [§ 6.1 'margin-top'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_top: Option<LengthValue>,
    /// [§ 6.1 'margin-right'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_right: Option<LengthValue>,
    /// [§ 6.1 'margin-bottom'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_bottom: Option<LengthValue>,
    /// [§ 6.1 'margin-left'](https://www.w3.org/TR/css-box-4/#margin-physical)
    pub margin_left: Option<LengthValue>,

    /// [§ 6.2 'padding-top'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_top: Option<LengthValue>,
    /// [§ 6.2 'padding-right'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_right: Option<LengthValue>,
    /// [§ 6.2 'padding-bottom'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_bottom: Option<LengthValue>,
    /// [§ 6.2 'padding-left'](https://www.w3.org/TR/css-box-4/#padding-physical)
    pub padding_left: Option<LengthValue>,

    /// [§ 4 'border-top'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_top: Option<BorderValue>,
    /// [§ 4 'border-right'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_right: Option<BorderValue>,
    /// [§ 4 'border-bottom'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_bottom: Option<BorderValue>,
    /// [§ 4 'border-left'](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    pub border_left: Option<BorderValue>,
}

impl ComputedStyle {
    /// Apply a CSS declaration to update this computed style.
    pub fn apply_declaration(&mut self, decl: &Declaration) {
        match decl.name.to_ascii_lowercase().as_str() {
            // [§ 2 The display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
            //
            // "The display property defines an element's display type..."
            "display" => {
                if let Some(display) = parse_display_value(&decl.value) {
                    self.display = Some(display);
                    self.display_none = false;
                } else if is_display_none(&decl.value) {
                    // [§ 2.6 display: none](https://www.w3.org/TR/css-display-3/#valdef-display-none)
                    // "The element and its descendants generate no boxes or text runs."
                    self.display = None;
                    self.display_none = true;
                }
            }
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
                    self.margin_top = Some(self.resolve_length(len));
                }
            }
            "margin-right" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_right = Some(self.resolve_length(len));
                }
            }
            "margin-bottom" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_bottom = Some(self.resolve_length(len));
                }
            }
            "margin-left" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.margin_left = Some(self.resolve_length(len));
                }
            }
            "padding" => {
                self.apply_padding_shorthand(&decl.value);
            }
            "padding-top" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_top = Some(self.resolve_length(len));
                }
            }
            "padding-right" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_right = Some(self.resolve_length(len));
                }
            }
            "padding-bottom" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_bottom = Some(self.resolve_length(len));
                }
            }
            "padding-left" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.padding_left = Some(self.resolve_length(len));
                }
            }
            "border" => {
                self.apply_border_shorthand(&decl.value);
            }
            "background" => {
                self.apply_background_shorthand(&decl.value);
            }
            "font-size" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.font_size = Some(self.resolve_length(len));
                }
            }
            unknown => {
                warn_once("CSS", &format!("unknown property '{unknown}'"));
            }
        }
    }

    /// [§ 6.3 Margins](https://www.w3.org/TR/css-box-4/#margins)
    ///
    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS21/box.html#margin-properties)
    /// "The 'margin' property is a shorthand property for setting 'margin-top',
    /// 'margin-right', 'margin-bottom', and 'margin-left' at the same place in
    /// the style sheet."
    fn apply_margin_shorthand(&mut self, values: &[ComponentValue]) {
        let lengths: Vec<LengthValue> = values.iter().filter_map(parse_single_length).collect();

        match lengths.len() {
            // "If there is only one component value, it applies to all sides."
            1 => {
                self.margin_top = Some(self.resolve_length(lengths[0].clone()));
                self.margin_right = Some(self.resolve_length(lengths[0].clone()));
                self.margin_bottom = Some(self.resolve_length(lengths[0].clone()));
                self.margin_left = Some(self.resolve_length(lengths[0].clone()));
            }
            // "If there are two values, the top and bottom margins are set to the
            // first value and the right and left margins are set to the second."
            2 => {
                self.margin_top = Some(self.resolve_length(lengths[0].clone()));
                self.margin_bottom = Some(self.resolve_length(lengths[0].clone()));
                self.margin_right = Some(self.resolve_length(lengths[1].clone()));
                self.margin_left = Some(self.resolve_length(lengths[1].clone()));
            }
            // "If there are three values, the top is set to the first value, the
            // left and right are set to the second, and the bottom is set to the third."
            3 => {
                self.margin_top = Some(self.resolve_length(lengths[0].clone()));
                self.margin_right = Some(self.resolve_length(lengths[1].clone()));
                self.margin_left = Some(self.resolve_length(lengths[1].clone()));
                self.margin_bottom = Some(self.resolve_length(lengths[2].clone()));
            }
            // "If there are four values, they apply to the top, right, bottom, and
            // left, respectively."
            4 => {
                self.margin_top = Some(self.resolve_length(lengths[0].clone()));
                self.margin_right = Some(self.resolve_length(lengths[1].clone()));
                self.margin_bottom = Some(self.resolve_length(lengths[2].clone()));
                self.margin_left = Some(self.resolve_length(lengths[3].clone()));
            }
            _ => {}
        }
    }

    /// [§ 6.2 Padding](https://www.w3.org/TR/css-box-4/#paddings)
    fn apply_padding_shorthand(&mut self, values: &[ComponentValue]) {
        let lengths: Vec<LengthValue> = values.iter().filter_map(parse_single_length).collect();

        match lengths.len() {
            1 => {
                self.padding_top = Some(self.resolve_length(lengths[0].clone()));
                self.padding_right = Some(self.resolve_length(lengths[0].clone()));
                self.padding_bottom = Some(self.resolve_length(lengths[0].clone()));
                self.padding_left = Some(self.resolve_length(lengths[0].clone()));
            }
            2 => {
                self.padding_top = Some(self.resolve_length(lengths[0].clone()));
                self.padding_bottom = Some(self.resolve_length(lengths[0].clone()));
                self.padding_right = Some(self.resolve_length(lengths[1].clone()));
                self.padding_left = Some(self.resolve_length(lengths[1].clone()));
            }
            3 => {
                self.padding_top = Some(self.resolve_length(lengths[0].clone()));
                self.padding_right = Some(self.resolve_length(lengths[1].clone()));
                self.padding_left = Some(self.resolve_length(lengths[1].clone()));
                self.padding_bottom = Some(self.resolve_length(lengths[2].clone()));
            }
            4 => {
                self.padding_top = Some(self.resolve_length(lengths[0].clone()));
                self.padding_right = Some(self.resolve_length(lengths[1].clone()));
                self.padding_bottom = Some(self.resolve_length(lengths[2].clone()));
                self.padding_left = Some(self.resolve_length(lengths[3].clone()));
            }
            _ => {}
        }
    }

    /// [§ 3.1 border shorthand](https://www.w3.org/TR/css-backgrounds-3/#the-border-shorthands)
    /// "border: 1px solid #ddd" sets all four borders
    fn apply_border_shorthand(&mut self, values: &[ComponentValue]) {
        let mut width: Option<LengthValue> = None;
        let mut style: Option<String> = None;
        let mut color: Option<ColorValue> = None;

        for v in values {
            // Try to parse as length (width)
            if width.is_none() {
                if let Some(len) = parse_single_length(v) {
                    width = Some(self.resolve_length(len));
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
    /// [§ 3.10 Background](https://www.w3.org/TR/css-backgrounds-3/#background)
    ///
    /// "The 'background' property is a shorthand property for setting most
    /// background properties at the same place in the style sheet."
    ///
    /// TODO: Currently only handles background-color. Full shorthand supports:
    /// background-image, background-position, background-size, background-repeat,
    /// background-attachment, background-origin, background-clip
    fn apply_background_shorthand(&mut self, values: &[ComponentValue]) {
        if let Some(color) = parse_color_value(values) {
            self.background_color = Some(color);
        }
    }
    /// Resolve relative length units (em) to absolute units (px).
    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    fn resolve_length(&self, len: LengthValue) -> LengthValue {
        match len {
            LengthValue::Em(em) => {
                let base = self
                    .font_size
                    .as_ref()
                    .map(|fs| fs.to_px())
                    .unwrap_or(DEFAULT_FONT_SIZE_PX);
                LengthValue::Px(em * base)
            }
            other => other,
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
            } else if unit.eq_ignore_ascii_case("em") {
                Some(LengthValue::Em(*value))
            } else {
                warn_once(
                    "CSS",
                    &format!("unsupported unit '{unit}' in value {value}{unit}"),
                );
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

/// [§ 4.2 line-height](https://www.w3.org/TR/css-inline-3/#line-height-property)
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

/// [§ 2 The display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
///
/// Parse a display value from component values.
/// Returns None if the value is "none" or unrecognized (use is_display_none for "none").
fn parse_display_value(values: &[ComponentValue]) -> Option<DisplayValue> {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
            let lower = ident.to_ascii_lowercase();
            match lower.as_str() {
                // [§ 2.1 Outer Display Roles]
                // "block: The element generates a block-level box."
                "block" => return Some(DisplayValue::block()),

                // "inline: The element generates an inline-level box."
                "inline" => return Some(DisplayValue::inline()),

                // [§ 2.4 Combination Display Keywords]
                // "inline-block: This value causes an element to generate an inline-level
                // block container."
                "inline-block" => return Some(DisplayValue::inline_block()),

                // [§ 2.2 Inner Display Layout Models]
                // "flex: The element generates a principal flex container box."
                "flex" => return Some(DisplayValue::flex()),

                // "grid: The element generates a principal grid container box."
                "grid" => return Some(DisplayValue::grid()),

                // "none" is handled separately by is_display_none
                "none" => return None,

                _ => {
                    warn_once("CSS", &format!("unsupported display value '{}'", ident));
                }
            }
        }
    }
    None
}

/// [§ 2.6 display: none](https://www.w3.org/TR/css-display-3/#valdef-display-none)
///
/// Check if the display value is "none".
fn is_display_none(values: &[ComponentValue]) -> bool {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
            if ident.eq_ignore_ascii_case("none") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex_6() {
        let color = ColorValue::from_hex("#ff0000").unwrap();
        assert_eq!(
            color,
            ColorValue {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            }
        );
    }

    #[test]
    fn test_color_from_hex_3() {
        let color = ColorValue::from_hex("#f00").unwrap();
        assert_eq!(
            color,
            ColorValue {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            }
        );
    }

    #[test]
    fn test_color_from_hex_mixed_case() {
        let color = ColorValue::from_hex("#FfA500").unwrap();
        assert_eq!(
            color,
            ColorValue {
                r: 255,
                g: 165,
                b: 0,
                a: 255
            }
        );
    }

    #[test]
    fn test_color_from_hex_without_hash() {
        let color = ColorValue::from_hex("00ff00").unwrap();
        assert_eq!(
            color,
            ColorValue {
                r: 0,
                g: 255,
                b: 0,
                a: 255
            }
        );
    }

    #[test]
    fn test_color_from_named() {
        assert_eq!(
            ColorValue::from_named("white"),
            Some(ColorValue {
                r: 255,
                g: 255,
                b: 255,
                a: 255
            })
        );
        assert_eq!(
            ColorValue::from_named("BLACK"),
            Some(ColorValue {
                r: 0,
                g: 0,
                b: 0,
                a: 255
            })
        );
        assert_eq!(ColorValue::from_named("unknown"), None);
    }

    #[test]
    fn test_length_px() {
        let len = LengthValue::Px(16.0);
        assert_eq!(len.to_px(), 16.0);
    }
}
