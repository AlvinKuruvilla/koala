//! CSS value types and resolution per [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/).
//!
//! This module defines the core CSS value types and their resolution to used values.
//!
//! # Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                           CSS Value Pipeline                            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  CSS Text ("20px", "2em")                                               │
//! │         │                                                               │
//! │         ▼                                                               │
//! │  ┌─────────────────┐                                                    │
//! │  │ style/mod.rs    │  parse_single_length() parses tokens into         │
//! │  │ (parsing)       │  LengthValue enum variants                         │
//! │  └────────┬────────┘                                                    │
//! │           │                                                             │
//! │           ▼                                                             │
//! │  ┌─────────────────┐                                                    │
//! │  │ values/mod.rs   │  LengthValue stored in ComputedStyle               │
//! │  │ (this module)   │  (em/rem/vw/vh remain unresolved)                  │
//! │  └────────┬────────┘                                                    │
//! │           │                                                             │
//! │           ▼  During layout, when we have context                        │
//! │  ┌─────────────────┐                                                    │
//! │  │ layout.rs       │  Creates ResolutionContext with viewport/font      │
//! │  │ (resolution)    │  Calls LengthValue::to_px(&ctx) → f64              │
//! │  └─────────────────┘                                                    │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Adding a New Length Unit
//!
//! To add a new unit (e.g., `vmin`), update these locations:
//!
//! ## Step 1: Add the variant to `LengthValue` (this file, ~line 220)
//!
//! ```ignore
//! pub enum LengthValue {
//!     // ... existing variants ...
//!
//!     /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
//!     ///
//!     /// "vmin: Equal to the smaller of vw or vh"
//!     Vmin(f64),
//! }
//! ```
//!
//! ## Step 2: Add resolution logic in `LengthValue::to_px()` (this file, ~line 270)
//!
//! ```ignore
//! LengthValue::Vmin(v) => {
//!     let vw = ctx.viewport_width / 100.0;
//!     let vh = ctx.viewport_height / 100.0;
//!     *v * vw.min(vh)
//! }
//! ```
//!
//! ## Step 3: Add context fields if needed in `ResolutionContext` (this file, ~line 30)
//!
//! For vmin, no new fields needed (uses existing viewport_width/height).
//! For units like `ch`, you'd need to add `ch_width: f64` to the context.
//!
//! ## Step 4: Add parsing in `parse_single_length()` (style/mod.rs, ~line 475)
//!
//! ```ignore
//! else if unit.eq_ignore_ascii_case("vmin") {
//!     Some(LengthValue::Vmin(*value))
//! }
//! ```
//!
//! ## Step 5: Add tests (tests/style_tests.rs)
//!
//! ```ignore
//! #[test]
//! fn test_vmin_units() {
//!     let vmin = LengthValue::Vmin(50.0);
//!     let ctx = ResolutionContext::with_viewport(800.0, 600.0);
//!     assert_eq!(vmin.to_px(&ctx), 300.0); // 50% of min(800,600) = 50% of 600
//! }
//! ```
//!
//! That's it! The callers in layout.rs, renderer.rs, etc. don't need changes
//! because they already use `to_px(&ctx)` generically.
//!
//! # Type Design
//!
//! | Type | Spec Grammar | Use Case |
//! |------|--------------|----------|
//! | `LengthValue` | `<length>` | Actual distance values with units |
//! | `Percentage` | `<percentage>` | Ratios that resolve relative to a base |
//! | `LengthOrPercentage` | `<length-percentage>` | Properties accepting either |
//! | `AutoLength` | `auto \| <length>` | Properties where `auto` has special meaning |
//!
//! # Value Processing Stages
//!
//! [§ 4.4 Used Values](https://www.w3.org/TR/css-cascade-4/#used-value)
//!
//! "The used value is the result of taking the computed value and completing
//! any remaining calculations to make it the absolute theoretical value
//! used in the formatting of the document."
//!
//! Resolution of relative units (em, rem, vw, vh, %) happens when converting
//! computed values to used values, which requires layout context.

use serde::Serialize;

// ============================================================================
// Resolution Context
// ============================================================================

/// [§ 4.4 Used Values](https://www.w3.org/TR/css-cascade-4/#used-value)
///
/// "The used value is the result of taking the computed value and completing
/// any remaining calculations to make it the absolute theoretical value."
///
/// Context required to resolve relative CSS units to absolute pixel values.
#[derive(Debug, Clone, Copy)]
pub struct ResolutionContext {
    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// "em: Equal to the computed value of the font-size property of the element
    /// on which it is used."
    pub font_size_px: f64,

    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// "rem: Equal to the computed value of font-size on the root element."
    pub root_font_size_px: f64,

    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    ///
    /// "The viewport-percentage lengths are relative to the size of the
    /// initial containing block."
    pub viewport_width: f64,

    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    pub viewport_height: f64,
}

impl ResolutionContext {
    /// Create a context with default font sizes (16px) and specified viewport.
    pub fn with_viewport(viewport_width: f64, viewport_height: f64) -> Self {
        Self {
            font_size_px: DEFAULT_FONT_SIZE_PX,
            root_font_size_px: DEFAULT_FONT_SIZE_PX,
            viewport_width,
            viewport_height,
        }
    }

    /// Create a context with all parameters specified.
    pub fn new(
        font_size_px: f64,
        root_font_size_px: f64,
        viewport_width: f64,
        viewport_height: f64,
    ) -> Self {
        Self {
            font_size_px,
            root_font_size_px,
            viewport_width,
            viewport_height,
        }
    }
}

impl Default for ResolutionContext {
    fn default() -> Self {
        Self {
            font_size_px: DEFAULT_FONT_SIZE_PX,
            root_font_size_px: DEFAULT_FONT_SIZE_PX,
            viewport_width: 0.0,
            viewport_height: 0.0,
        }
    }
}

// ============================================================================
// Constants
// ============================================================================

/// User agent default font size.
///
/// [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
///
/// "Initial: medium" - we define medium as 16px per common browser convention.
pub const DEFAULT_FONT_SIZE_PX: f64 = 16.0;

// ============================================================================
// Display Types
// ============================================================================

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
///
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

// ============================================================================
// Length Values
// ============================================================================

/// [§ 5 Distance Units](https://www.w3.org/TR/css-values-4/#lengths)
///
/// "Lengths refer to distance measurements and are denoted by <length> in the
/// property definitions."
///
/// This enum represents actual length values with units. Percentages are
/// handled separately by [`Percentage`] since they resolve relative to a
/// context-dependent base value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum LengthValue {
    /// [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
    ///
    /// "1px = 1/96th of 1in"
    Px(f64),

    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// "Equal to the computed value of the font-size property of the element
    /// on which it is used."
    Em(f64),

    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// "Equal to the computed value of font-size on the root element."
    Rem(f64),

    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    ///
    /// "1vw = 1% of viewport width"
    Vw(f64),

    /// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    ///
    /// "1vh = 1% of viewport height"
    Vh(f64),
    // TODO: Add additional length units:
    //
    // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    // - ex: "Equal to the used x-height of the first available font"
    // - ch: "Equal to the used advance measure of the '0' glyph"
    // - cap, ic, lh, rlh
    //
    // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    // - vmin: "Equal to the smaller of vw or vh"
    // - vmax: "Equal to the larger of vw or vh"
    // - vi, vb, svw, svh, lvw, lvh, dvw, dvh
    //
    // [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
    // - cm, mm, Q, in, pt, pc
}

impl LengthValue {
    /// [§ 4.4 Used Values](https://www.w3.org/TR/css-cascade-4/#used-value)
    ///
    /// "The used value is the result of taking the computed value and completing
    /// any remaining calculations to make it the absolute theoretical value."
    ///
    /// Resolve this length to an absolute pixel value using the provided context.
    pub fn to_px(&self, ctx: &ResolutionContext) -> f64 {
        match self {
            // [§ 6.1 Absolute lengths](https://www.w3.org/TR/css-values-4/#absolute-lengths)
            //
            // "1px = 1/96th of 1in"
            // Pixels are already absolute.
            LengthValue::Px(v) => *v,

            // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
            //
            // "em: Equal to the computed value of the font-size property of the
            // element on which it is used."
            LengthValue::Em(v) => *v * ctx.font_size_px,

            // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
            //
            // "rem: Equal to the computed value of font-size on the root element."
            LengthValue::Rem(v) => *v * ctx.root_font_size_px,

            // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
            //
            // "1vw = 1% of viewport width"
            LengthValue::Vw(v) => *v * ctx.viewport_width / 100.0,

            // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
            //
            // "1vh = 1% of viewport height"
            LengthValue::Vh(v) => *v * ctx.viewport_height / 100.0,
        }
    }
}

// ============================================================================
// Percentage Values
// ============================================================================

/// [§ 5.1.3 Percentages](https://www.w3.org/TR/css-values-4/#percentages)
///
/// "A <percentage> value is denoted by <percentage>, and consists of a <number>
/// immediately followed by a percent sign '%'. Percentage values are always
/// relative to another quantity, for example a length."
///
/// The reference quantity depends on the property:
/// - `width`, `margin-*`, `padding-*`: containing block width
/// - `height`: containing block height (if definite)
/// - `font-size`: inherited font-size
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Percentage(pub f64);

impl Percentage {
    /// Create a new percentage value.
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// Get the raw percentage value (e.g., 50.0 for 50%).
    pub fn value(&self) -> f64 {
        self.0
    }

    /// [§ 5.1.3 Percentages](https://www.w3.org/TR/css-values-4/#percentages)
    ///
    /// "Percentage values are always relative to another quantity."
    ///
    /// Resolve this percentage to an absolute value given the reference base.
    pub fn resolve(&self, base: f64) -> f64 {
        self.0 * base / 100.0
    }
}

// ============================================================================
// Length or Percentage
// ============================================================================

/// [§ 5 <length-percentage>](https://www.w3.org/TR/css-values-4/#typedef-length-percentage)
///
/// "Where <length-percentage> is used, it represents a value that can be either
/// a <length> or a <percentage>."
///
/// Many CSS properties accept either a length or a percentage. This type
/// represents that combined grammar production.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum LengthOrPercentage {
    /// An absolute or relative length value.
    Length(LengthValue),
    /// A percentage value, resolved relative to a context-dependent base.
    Percent(Percentage),
}

impl LengthOrPercentage {
    /// [§ 4.4 Used Values](https://www.w3.org/TR/css-cascade-4/#used-value)
    ///
    /// Resolve to an absolute pixel value.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Resolution context for font-relative and viewport units
    /// * `percent_base` - The reference value for percentage resolution
    ///   (e.g., containing block width for `width`, `margin-*`, `padding-*`)
    pub fn to_px(&self, ctx: &ResolutionContext, percent_base: f64) -> f64 {
        match self {
            LengthOrPercentage::Length(len) => len.to_px(ctx),
            LengthOrPercentage::Percent(pct) => pct.resolve(percent_base),
        }
    }

    /// Check if this is a percentage value.
    pub fn is_percent(&self) -> bool {
        matches!(self, LengthOrPercentage::Percent(_))
    }

    /// Check if this is a length value.
    pub fn is_length(&self) -> bool {
        matches!(self, LengthOrPercentage::Length(_))
    }
}

impl From<LengthValue> for LengthOrPercentage {
    fn from(len: LengthValue) -> Self {
        LengthOrPercentage::Length(len)
    }
}

impl From<Percentage> for LengthOrPercentage {
    fn from(pct: Percentage) -> Self {
        LengthOrPercentage::Percent(pct)
    }
}

// ============================================================================
// Auto Length
// ============================================================================

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
/// Used for properties like margin and width where 'auto' has special meaning.
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

    /// Check if the value is a specific length.
    pub fn is_length(&self) -> bool {
        matches!(self, AutoLength::Length(_))
    }

    /// Get the length value, if not 'auto'.
    pub fn as_length(&self) -> Option<&LengthValue> {
        match self {
            AutoLength::Length(len) => Some(len),
            AutoLength::Auto => None,
        }
    }
}

impl From<LengthValue> for AutoLength {
    fn from(len: LengthValue) -> Self {
        AutoLength::Length(len)
    }
}

// ============================================================================
// Color Values
// ============================================================================

/// [§ 4 Color syntax](https://www.w3.org/TR/css-color-4/#color-syntax)
///
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
    ///
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
    ///
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

// ============================================================================
// Border Values
// ============================================================================

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
