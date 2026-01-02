//! CSS Computed Style representation and value parsing.
//!
//! This module implements computed style aggregation and CSS value parsing per:
//! - [CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/)
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//!
//! Value types are defined in the [`crate::values`] module.

use serde::Serialize;

use crate::parser::{ComponentValue, Declaration};
use crate::tokenizer::CSSToken;
use crate::values::{
    AutoLength, BorderValue, ColorValue, DisplayValue, LengthValue, DEFAULT_FONT_SIZE_PX,
};
use koala_common::warning::warn_once;

// Re-export value types for convenience (prefer importing from crate::values directly)
pub use crate::values::{LengthOrPercentage, Percentage, ResolutionContext};

/// Computed styles for an element.
///
/// [§ 4.4 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
///
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
    ///
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

    /// Resolve relative length units (em) to absolute units (px) during cascade.
    ///
    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// NOTE: This is a partial resolution during cascade. Full resolution to used
    /// values (including viewport units) happens during layout via [`ResolutionContext`].
    fn resolve_length(&self, len: LengthValue) -> LengthValue {
        match len {
            LengthValue::Em(em) => {
                // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
                //
                // "em: Equal to the computed value of the font-size property of the
                // element on which it is used."
                let ctx = ResolutionContext::default();
                let base = self
                    .font_size
                    .as_ref()
                    .map(|fs| fs.to_px(&ctx))
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

// ============================================================================
// Parsing Functions
// ============================================================================

/// [§ 4.2 RGB hex notation](https://www.w3.org/TR/css-color-4/#hex-notation)
///
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
///
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
            // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
            // "Equal to the computed value of font-size on the root element"
            else if unit.eq_ignore_ascii_case("rem") {
                Some(LengthValue::Rem(*value))
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
///
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
