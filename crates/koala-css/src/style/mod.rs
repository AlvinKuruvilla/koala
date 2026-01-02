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
            // TODO: Implement full CSS named color list
            // [§ 6.1 Named Colors](https://www.w3.org/TR/css-color-4/#named-colors)
            //
            // STEP 1: Add basic color keywords (16 HTML colors)
            //   aqua, fuchsia, lime, maroon, navy, olive, purple, silver, teal
            //
            // STEP 2: Add extended color keywords (X11 colors, ~140 total)
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
    ///
    /// Can be 'auto' or a specific length. 'auto' is resolved during layout.
    pub margin_top: Option<AutoLength>,
    /// [§ 6.1 'margin-right'](https://www.w3.org/TR/css-box-4/#margin-physical)
    ///
    /// Can be 'auto' or a specific length. 'auto' is resolved during layout.
    pub margin_right: Option<AutoLength>,
    /// [§ 6.1 'margin-bottom'](https://www.w3.org/TR/css-box-4/#margin-physical)
    ///
    /// Can be 'auto' or a specific length. 'auto' is resolved during layout.
    pub margin_bottom: Option<AutoLength>,
    /// [§ 6.1 'margin-left'](https://www.w3.org/TR/css-box-4/#margin-physical)
    ///
    /// Can be 'auto' or a specific length. 'auto' is resolved during layout.
    pub margin_left: Option<AutoLength>,

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

    /// [§ 10.2 'width'](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
    ///
    /// "This property specifies the content width of boxes."
    /// "Value: <length> | <percentage> | auto | inherit"
    pub width: Option<AutoLength>,

    /// [§ 10.5 'height'](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    ///
    /// "This property specifies the content height of boxes."
    /// "Value: <length> | <percentage> | auto | inherit"
    pub height: Option<AutoLength>,
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
            // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
            //
            // "Value: <margin-width> | inherit"
            // "<margin-width> = <length> | <percentage> | auto"
            "margin-top" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.margin_top = Some(self.resolve_auto_length(al));
                }
            }
            "margin-right" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.margin_right = Some(self.resolve_auto_length(al));
                }
            }
            "margin-bottom" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.margin_bottom = Some(self.resolve_auto_length(al));
                }
            }
            "margin-left" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.margin_left = Some(self.resolve_auto_length(al));
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
            // [§ 10.2 'width'](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
            //
            // "This property specifies the content width of boxes."
            // "Value: <length> | <percentage> | auto | inherit"
            "width" => {
                if let Some(first) = decl.value.first() {
                    if let Some(auto_len) = parse_single_auto_length(first) {
                        self.width = Some(self.resolve_auto_length(auto_len));
                    }
                }
            }
            // [§ 10.5 'height'](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
            //
            // "This property specifies the content height of boxes."
            // "Value: <length> | <percentage> | auto | inherit"
            "height" => {
                if let Some(first) = decl.value.first() {
                    if let Some(auto_len) = parse_single_auto_length(first) {
                        self.height = Some(self.resolve_auto_length(auto_len));
                    }
                }
            }
            unknown => {
                warn_once("CSS", &format!("unknown property '{unknown}'"));
            }
        }
    }

    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS21/box.html#margin-properties)
    ///
    /// "The 'margin' property is a shorthand property for setting 'margin-top',
    /// 'margin-right', 'margin-bottom', and 'margin-left' at the same place in
    /// the style sheet."
    ///
    /// "Value: <margin-width>{1,4} | inherit"
    /// "<margin-width> = <length> | <percentage> | auto"
    fn apply_margin_shorthand(&mut self, values: &[ComponentValue]) {
        // STEP 1: Parse all <margin-width> values from the declaration.
        // [§ 8.3](https://www.w3.org/TR/CSS2/box.html#margin-properties)
        //
        // "<margin-width> = <length> | <percentage> | auto"
        let auto_lengths: Vec<AutoLength> =
            values.iter().filter_map(parse_single_auto_length).collect();

        // STEP 2: Apply the shorthand expansion rules.
        // [§ 8.3](https://www.w3.org/TR/CSS2/box.html#margin-properties)
        //
        // "If there is only one component value, it applies to all sides.
        // If there are two values, the top and bottom margins are set to the
        // first value and the right and left margins are set to the second.
        // If there are three values, the top is set to the first value, the
        // left and right are set to the second, and the bottom is set to the
        // third. If there are four values, they apply to the top, right,
        // bottom, and left, respectively."
        match auto_lengths.len() {
            // RULE 1-VALUE: "it applies to all sides."
            1 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[0].clone()));
            }
            // RULE 2-VALUE: "the top and bottom margins are set to the first value
            //               and the right and left margins are set to the second."
            2 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[1].clone()));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[1].clone()));
            }
            // RULE 3-VALUE: "the top is set to the first value, the left and right
            //               are set to the second, and the bottom is set to the third."
            3 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[1].clone()));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[1].clone()));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[2].clone()));
            }
            // RULE 4-VALUE: "they apply to the top, right, bottom, and left, respectively."
            4 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0].clone()));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[1].clone()));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[2].clone()));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[3].clone()));
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

    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// Resolve relative length units (em) to absolute units (px) for AutoLength.
    /// 'auto' values are preserved unchanged.
    fn resolve_auto_length(&self, al: AutoLength) -> AutoLength {
        match al {
            AutoLength::Auto => AutoLength::Auto,
            AutoLength::Length(len) => AutoLength::Length(self.resolve_length(len)),
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
fn parse_length_value(values: &[ComponentValue]) -> Option<LengthValue> {
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
fn parse_single_length(v: &ComponentValue) -> Option<LengthValue> {
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
fn parse_auto_length_value(values: &[ComponentValue]) -> Option<AutoLength> {
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
fn parse_single_auto_length(v: &ComponentValue) -> Option<AutoLength> {
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

    #[test]
    fn test_auto_length() {
        // [§ 4.4 Automatic values](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
        // Test that AutoLength::Auto is properly handled
        let auto = AutoLength::Auto;
        assert!(auto.is_auto());
        assert_eq!(auto.to_px(), 0.0); // auto resolves to 0 when asked for px

        // Test that AutoLength::Length properly wraps a length
        let len = AutoLength::Length(LengthValue::Px(20.0));
        assert!(!len.is_auto());
        assert_eq!(len.to_px(), 20.0);
    }

    #[test]
    fn test_parse_margin_auto() {
        // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
        // "<margin-width> = <length> | <percentage> | auto"
        use crate::parser::ComponentValue;
        use crate::tokenizer::{CSSToken, NumericType};

        // Test parsing "auto" value
        let auto_values = vec![ComponentValue::Token(CSSToken::Ident("auto".to_string()))];
        let result = parse_auto_length_value(&auto_values);
        assert!(result.is_some());
        assert!(matches!(result, Some(AutoLength::Auto)));

        // Test parsing "AUTO" (case insensitive)
        let auto_upper = vec![ComponentValue::Token(CSSToken::Ident("AUTO".to_string()))];
        let result_upper = parse_auto_length_value(&auto_upper);
        assert!(result_upper.is_some());
        assert!(matches!(result_upper, Some(AutoLength::Auto)));

        // Test parsing a length value
        let length_values = vec![ComponentValue::Token(CSSToken::Dimension {
            value: 20.0,
            unit: "px".to_string(),
            int_value: Some(20),
            numeric_type: NumericType::Integer,
        })];
        let result2 = parse_auto_length_value(&length_values);
        assert!(result2.is_some());
        assert!(matches!(
            result2,
            Some(AutoLength::Length(LengthValue::Px(_)))
        ));
    }

    #[test]
    fn test_viewport_units() {
        // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
        // "1vw = 1% of viewport width"
        // "1vh = 1% of viewport height"

        // Test vw: 60vw on a 1000px wide viewport = 600px
        let vw = LengthValue::Vw(60.0);
        assert_eq!(vw.to_px_with_viewport(1000.0, 800.0), 600.0);

        // Test vh: 15vh on a 800px tall viewport = 120px
        let vh = LengthValue::Vh(15.0);
        assert_eq!(vh.to_px_with_viewport(1000.0, 800.0), 120.0);

        // Test 100vw = full viewport width
        let full_vw = LengthValue::Vw(100.0);
        assert_eq!(full_vw.to_px_with_viewport(1280.0, 720.0), 1280.0);

        // Test 100vh = full viewport height
        let full_vh = LengthValue::Vh(100.0);
        assert_eq!(full_vh.to_px_with_viewport(1280.0, 720.0), 720.0);
    }
}
