//! CSS Computed Style
//!
//! [§ 4.4 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
//! "The computed value is the result of resolving the specified value..."

use serde::Serialize;

use crate::parser::{ComponentValue, Declaration};
use crate::tokenizer::CSSToken;
use crate::{AutoLength, BorderValue, ColorValue, LengthValue};
use koala_common::warning::warn_once;

use super::display::{DisplayValue, is_display_none, parse_display_value};
use super::values::{
    DEFAULT_FONT_SIZE_PX, parse_auto_length_value, parse_color_value, parse_font_family,
    parse_font_weight, parse_length_value, parse_line_height, parse_single_auto_length,
    parse_single_color, parse_single_length,
};
use super::writing_mode::{PhysicalSide, WritingMode, parse_writing_mode};

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

    /// [§ 2 Block Flow Direction](https://www.w3.org/TR/css-writing-modes-4/#block-flow)
    ///
    /// "The writing-mode property specifies whether lines of text are laid out
    /// horizontally or vertically and the direction in which blocks progress."
    ///
    /// This determines how logical properties (margin-block-start, etc.) map to
    /// physical properties (margin-top, etc.).
    ///
    /// Initial: horizontal-tb
    /// Inherited: yes
    pub writing_mode: WritingMode,

    /// [§ 3.1 'color'](https://www.w3.org/TR/css-color-4/#the-color-property)
    pub color: Option<ColorValue>,
    /// [§ 3.1 'font-family'](https://www.w3.org/TR/css-fonts-4/#font-family-prop)
    pub font_family: Option<String>,
    /// [§ 3.5 'font-size'](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
    pub font_size: Option<LengthValue>,
    /// [§ 3.2 'font-weight'](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
    pub font_weight: Option<u16>,
    /// [§ 3.3 'font-style'](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
    ///
    /// "This property allows italic or oblique faces to be selected."
    /// Values: normal | italic | oblique
    /// Inherited: yes
    pub font_style: Option<String>,
    /// [§ 4.2 'line-height'](https://www.w3.org/TR/css-inline-3/#line-height-property)
    pub line_height: Option<f64>,

    /// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
    ///
    /// "This property describes how inline-level content of a block
    /// container is aligned."
    ///
    /// Stored as a lowercase keyword string. Converted to `TextAlign`
    /// enum during layout tree construction.
    pub text_align: Option<String>,

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

    /// [§ 4.2 Flow-Relative Margins](https://drafts.csswg.org/css-logical-1/#margin-properties)
    ///
    /// "These properties correspond to the margin-top, margin-bottom, margin-left,
    /// and margin-right properties. The mapping depends on the element's writing-mode,
    /// direction, and text-orientation."
    ///
    /// For horizontal-tb (default): block-start = top, block-end = bottom
    pub margin_block_start: Option<AutoLength>,
    /// [§ 4.2 Flow-Relative Margins](https://drafts.csswg.org/css-logical-1/#margin-properties)
    pub margin_block_end: Option<AutoLength>,

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
    /// "Value: `<length>` | `<percentage>` | auto | inherit"
    pub width: Option<AutoLength>,

    /// [§ 10.5 'height'](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    ///
    /// "This property specifies the content height of boxes."
    /// "Value: `<length>` | `<percentage>` | auto | inherit"
    pub height: Option<AutoLength>,

    /// [§ 10.4 'min-width'](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
    ///
    /// "Value: `<length>` | `<percentage>` | inherit"
    /// Initial: 0
    /// None means initial (0 — no minimum constraint).
    pub min_width: Option<LengthValue>,

    /// [§ 10.4 'max-width'](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
    ///
    /// "Value: `<length>` | `<percentage>` | none | inherit"
    /// Initial: none
    /// None means initial (none — no maximum constraint).
    pub max_width: Option<LengthValue>,

    /// [§ 10.7 'min-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
    ///
    /// "Value: `<length>` | `<percentage>` | inherit"
    /// Initial: 0
    /// None means initial (0 — no minimum constraint).
    pub min_height: Option<LengthValue>,

    /// [§ 10.7 'max-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
    ///
    /// "Value: `<length>` | `<percentage>` | none | inherit"
    /// Initial: none
    /// None means initial (none — no maximum constraint).
    pub max_height: Option<LengthValue>,

    // [§ 2 Flex Layout Box Model](https://www.w3.org/TR/css-flexbox-1/#box-model)
    /// [§ 5.1 'flex-direction'](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
    ///
    /// "The flex-direction property specifies how flex items are placed in
    /// the flex container, by setting the direction of the flex container's
    /// main axis."
    ///
    /// Values: row | row-reverse | column | column-reverse
    /// Initial: row
    pub flex_direction: Option<String>,

    /// [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
    ///
    /// "The justify-content property aligns flex items along the main axis
    /// of the current line of the flex container."
    ///
    /// Values: flex-start | flex-end | center | space-between | space-around
    /// Initial: flex-start
    pub justify_content: Option<String>,

    /// [§ 7.2 'flex-grow'](https://www.w3.org/TR/css-flexbox-1/#flex-grow-property)
    ///
    /// "The flex-grow property sets the flex grow factor to the provided
    /// `<number>`. Negative values are invalid."
    ///
    /// Initial: 0
    pub flex_grow: Option<f32>,

    /// [§ 7.3 'flex-shrink'](https://www.w3.org/TR/css-flexbox-1/#flex-shrink-property)
    ///
    /// "The flex-shrink property sets the flex shrink factor to the provided
    /// `<number>`. Negative values are invalid."
    ///
    /// Initial: 1
    pub flex_shrink: Option<f32>,

    /// [§ 7.1 'flex-basis'](https://www.w3.org/TR/css-flexbox-1/#flex-basis-property)
    ///
    /// "The flex-basis property sets the flex basis: the initial main size
    /// of the flex item, before free space is distributed."
    ///
    /// Values: auto | `<length>`
    /// Initial: auto
    pub flex_basis: Option<AutoLength>,

    // ===== Positioning properties =====
    /// [§ 9.3.1 'position'](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
    ///
    /// "The 'position' and 'float' properties determine which of the CSS 2
    /// positioning algorithms is used to calculate the position of a box."
    ///
    /// Values: static | relative | absolute | fixed | sticky
    /// Initial: static
    /// Inherited: no
    pub position: Option<String>,

    /// [§ 9.3.2 'top'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
    ///
    /// "The 'top' property specifies how far the top margin edge of the
    /// box is offset below the top edge of the box's containing block."
    ///
    /// Values: `<length>` | `<percentage>` | auto
    /// Initial: auto
    /// Inherited: no
    pub top: Option<AutoLength>,

    /// [§ 9.3.2 'right'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
    ///
    /// "The 'right' property specifies how far the right margin edge of
    /// the box is offset to the left of the right edge of the box's
    /// containing block."
    pub right: Option<AutoLength>,

    /// [§ 9.3.2 'bottom'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
    ///
    /// "The 'bottom' property specifies how far the bottom margin edge of
    /// the box is offset above the bottom edge of the box's containing block."
    pub bottom: Option<AutoLength>,

    /// [§ 9.3.2 'left'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
    ///
    /// "The 'left' property specifies how far the left margin edge of the
    /// box is offset to the right of the left edge of the box's containing block."
    pub left: Option<AutoLength>,

    /// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
    ///
    /// "The 'float' property specifies whether a box should float to the
    /// left, right, or not at all."
    ///
    /// Values: left | right | none
    /// Initial: none
    /// Inherited: no
    pub float: Option<String>,

    /// [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
    ///
    /// "This property indicates which sides of an element's box(es) may not
    /// be adjacent to an earlier floating box."
    ///
    /// Values: left | right | both | none
    /// Initial: none
    /// Inherited: no
    pub clear: Option<String>,

    /// [§ 3.1 'list-style-type'](https://www.w3.org/TR/css-lists-3/#list-style-type)
    ///
    /// "The list-style-type property specifies a counter style or string for
    /// the element's marker."
    ///
    /// Values: disc | circle | square | decimal | lower-alpha | upper-alpha |
    ///         lower-roman | upper-roman | none
    /// Initial: disc
    /// Inherited: yes
    pub list_style_type: Option<String>,

    // ─────────────────────────────────────────────────────────────────────────
    // Source order tracking for cascade resolution of logical property groups
    // ─────────────────────────────────────────────────────────────────────────
    //
    // [§ 4 Logical Property Groups](https://drafts.csswg.org/css-logical-1/#logical-property-groups)
    //
    // "Logical properties and their physical equivalents are considered part of
    // the same logical property group. Within a logical property group, declarations
    // compete based on specificity and source order, regardless of whether they
    // use logical or physical property names."
    //
    // These fields track which declaration (by source_order) set each physical margin,
    // allowing proper cascade resolution when both logical and physical properties
    // target the same computed value.
    /// [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    ///
    /// "This property specifies whether content of a block container element
    /// is clipped when it overflows the element's box."
    ///
    /// Values: visible | hidden | scroll | auto
    /// Initial: visible
    /// Inherited: no
    pub overflow: Option<String>,

    /// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
    ///
    /// "The box-sizing property defines whether the width and height (and
    /// respective min/max properties) on an element include padding and
    /// borders or not."
    ///
    /// "Inherited: no"
    /// "Initial: content-box"
    ///
    /// None = not set (initial = content-box).
    /// Some(true) = border-box, Some(false) = content-box.
    pub box_sizing_border_box: Option<bool>,

    /// Source order of the declaration that set `margin_top` (for cascade resolution)
    #[serde(skip)]
    pub margin_top_source_order: Option<u32>,
    /// Source order of the declaration that set `margin_right` (for cascade resolution)
    #[serde(skip)]
    pub margin_right_source_order: Option<u32>,
    /// Source order of the declaration that set `margin_bottom` (for cascade resolution)
    #[serde(skip)]
    pub margin_bottom_source_order: Option<u32>,
    /// Source order of the declaration that set `margin_left` (for cascade resolution)
    #[serde(skip)]
    pub margin_left_source_order: Option<u32>,
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
            // [§ 2 Block Flow Direction](https://www.w3.org/TR/css-writing-modes-4/#block-flow)
            //
            // "The writing-mode property specifies whether lines of text are laid out
            // horizontally or vertically and the direction in which blocks progress."
            //
            // Values: horizontal-tb | vertical-rl | vertical-lr
            // Initial: horizontal-tb
            "writing-mode" => {
                if let Some(wm) = parse_writing_mode(&decl.value) {
                    self.writing_mode = wm;
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
            // [§ 3.2 font-weight](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
            "font-weight" => {
                if let Some(weight) = parse_font_weight(&decl.value) {
                    self.font_weight = Some(weight);
                }
            }
            // [§ 3.3 font-style](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
            //
            // "This property allows italic or oblique faces to be selected."
            // Values: normal | italic | oblique
            "font-style" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(lower.as_str(), "normal" | "italic" | "oblique") {
                        self.font_style = Some(lower);
                    }
                }
            }
            // [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
            //
            // "Value: left | right | center | justify | inherit"
            "text-align" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(lower.as_str(), "left" | "right" | "center" | "justify") {
                        self.text_align = Some(lower);
                    }
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
            //
            // [§ 4 Logical Property Groups](https://drafts.csswg.org/css-logical-1/#logical-property-groups)
            //
            // Physical and logical properties compete in the cascade. We track
            // source_order to determine which declaration wins.
            "margin-top" => {
                if let Some(al) = parse_auto_length_value(&decl.value)
                    && self.should_update_margin(PhysicalSide::Top, decl.source_order)
                {
                    self.margin_top = Some(self.resolve_auto_length(al));
                    self.margin_top_source_order = Some(decl.source_order);
                }
            }
            "margin-right" => {
                if let Some(al) = parse_auto_length_value(&decl.value)
                    && self.should_update_margin(PhysicalSide::Right, decl.source_order)
                {
                    self.margin_right = Some(self.resolve_auto_length(al));
                    self.margin_right_source_order = Some(decl.source_order);
                }
            }
            "margin-bottom" => {
                if let Some(al) = parse_auto_length_value(&decl.value)
                    && self.should_update_margin(PhysicalSide::Bottom, decl.source_order)
                {
                    self.margin_bottom = Some(self.resolve_auto_length(al));
                    self.margin_bottom_source_order = Some(decl.source_order);
                }
            }
            "margin-left" => {
                if let Some(al) = parse_auto_length_value(&decl.value)
                    && self.should_update_margin(PhysicalSide::Left, decl.source_order)
                {
                    self.margin_left = Some(self.resolve_auto_length(al));
                    self.margin_left_source_order = Some(decl.source_order);
                }
            }
            // [§ 4.2 Flow-Relative Margins](https://drafts.csswg.org/css-logical-1/#margin-properties)
            //
            // "These properties correspond to the margin-top, margin-bottom,
            // margin-left, and margin-right properties. The mapping depends on
            // the element's writing-mode, direction, and text-orientation."
            //
            // Logical and physical properties are in the same "logical property group"
            // and compete in the cascade based on source_order.
            "margin-block-start" => {
                // STEP 1: Parse the value.
                //   [§ 4.2](https://drafts.csswg.org/css-logical-1/#margin-properties)
                //   "Value: <'margin-top'>"
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    // STEP 2: Map to the physical side based on writing-mode.
                    let physical_side = self.writing_mode.block_start_physical();

                    // STEP 3: Check cascade - only update if we win.
                    //   [§ 4 Logical Property Groups](https://drafts.csswg.org/css-logical-1/#logical-property-groups)
                    if self.should_update_margin(physical_side, decl.source_order) {
                        // STEP 4: Apply to both the logical field (for reference)
                        // and the corresponding physical property.
                        self.margin_block_start = Some(self.resolve_auto_length(al));
                        self.set_margin_for_side(physical_side, al, decl.source_order);
                    }
                }
            }
            // [§ 4.2 Flow-Relative Margins](https://drafts.csswg.org/css-logical-1/#margin-properties)
            "margin-block-end" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    let physical_side = self.writing_mode.block_end_physical();

                    if self.should_update_margin(physical_side, decl.source_order) {
                        self.margin_block_end = Some(self.resolve_auto_length(al));
                        self.set_margin_for_side(physical_side, al, decl.source_order);
                    }
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
            // [§ 4.4 border-top](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            //
            // "The 'border-top' shorthand property sets the width, style, and color
            // of the top border."
            //
            // Syntax: <line-width> || <line-style> || <color>
            // (values can appear in any order)
            "border-top" => {
                if let Some(border) = self.parse_border_side(&decl.value) {
                    self.border_top = Some(border);
                }
            }
            // [§ 4.4 border-right](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            "border-right" => {
                if let Some(border) = self.parse_border_side(&decl.value) {
                    self.border_right = Some(border);
                }
            }
            // [§ 4.4 border-bottom](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            "border-bottom" => {
                if let Some(border) = self.parse_border_side(&decl.value) {
                    self.border_bottom = Some(border);
                }
            }
            // [§ 4.4 border-left](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            "border-left" => {
                if let Some(border) = self.parse_border_side(&decl.value) {
                    self.border_left = Some(border);
                }
            }
            // [§ 4.1 'border-top-color', etc.](https://www.w3.org/TR/css-backgrounds-3/#border-color)
            //
            // "These properties set the foreground color of the border
            // specified by the border-top, border-right, border-bottom,
            // and border-left properties respectively."
            "border-top-color" => {
                if let Some(color) = parse_color_value(&decl.value) {
                    self.ensure_border_top().color = color;
                }
            }
            "border-right-color" => {
                if let Some(color) = parse_color_value(&decl.value) {
                    self.ensure_border_right().color = color;
                }
            }
            "border-bottom-color" => {
                if let Some(color) = parse_color_value(&decl.value) {
                    self.ensure_border_bottom().color = color;
                }
            }
            "border-left-color" => {
                if let Some(color) = parse_color_value(&decl.value) {
                    self.ensure_border_left().color = color;
                }
            }
            // [§ 4.3 'border-top-width', etc.](https://www.w3.org/TR/css-backgrounds-3/#border-width)
            //
            // "These properties set the thickness of the border."
            // "<line-width> = <length [0,∞]> | thin | medium | thick"
            "border-top-width" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.ensure_border_top().width = self.resolve_length(len);
                }
            }
            "border-right-width" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.ensure_border_right().width = self.resolve_length(len);
                }
            }
            "border-bottom-width" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.ensure_border_bottom().width = self.resolve_length(len);
                }
            }
            "border-left-width" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.ensure_border_left().width = self.resolve_length(len);
                }
            }
            // [§ 4.2 'border-top-style', etc.](https://www.w3.org/TR/css-backgrounds-3/#border-style)
            //
            // "These properties set the style of the border."
            // "<line-style> = none | hidden | dotted | dashed | solid | double |
            //                 groove | ridge | inset | outset"
            "border-top-style" => {
                if let Some(first) = decl.value.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_top().style = s;
                }
            }
            "border-right-style" => {
                if let Some(first) = decl.value.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_right().style = s;
                }
            }
            "border-bottom-style" => {
                if let Some(first) = decl.value.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_bottom().style = s;
                }
            }
            "border-left-style" => {
                if let Some(first) = decl.value.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_left().style = s;
                }
            }
            // [§ 4.1 'border-color'](https://www.w3.org/TR/css-backgrounds-3/#border-color)
            //
            // "The 'border-color' property is a shorthand for setting
            // 'border-top-color', 'border-right-color', 'border-bottom-color',
            // and 'border-left-color'."
            "border-color" => {
                self.apply_border_color_shorthand(&decl.value);
            }
            // [§ 4.3 'border-width'](https://www.w3.org/TR/css-backgrounds-3/#border-width)
            //
            // "The 'border-width' property is a shorthand for setting
            // 'border-top-width', 'border-right-width', 'border-bottom-width',
            // and 'border-left-width'."
            "border-width" => {
                self.apply_border_width_shorthand(&decl.value);
            }
            // [§ 4.2 'border-style'](https://www.w3.org/TR/css-backgrounds-3/#border-style)
            //
            // "The 'border-style' property is a shorthand for setting
            // 'border-top-style', 'border-right-style', 'border-bottom-style',
            // and 'border-left-style'."
            "border-style" => {
                self.apply_border_style_shorthand(&decl.value);
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
            // "Value: `<length>` | `<percentage>` | auto | inherit"
            "width" => {
                if let Some(first) = decl.value.first()
                    && let Some(auto_len) = parse_single_auto_length(first)
                {
                    self.width = Some(self.resolve_auto_length(auto_len));
                }
            }
            // [§ 10.5 'height'](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
            //
            // "This property specifies the content height of boxes."
            // "Value: `<length>` | `<percentage>` | auto | inherit"
            "height" => {
                if let Some(first) = decl.value.first()
                    && let Some(auto_len) = parse_single_auto_length(first)
                {
                    self.height = Some(self.resolve_auto_length(auto_len));
                }
            }
            // [§ 10.4 'min-width'](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
            //
            // "Value: <length> | <percentage> | inherit"
            // Initial: 0
            "min-width" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.min_width = Some(self.resolve_length(len));
                }
            }
            // [§ 10.4 'max-width'](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
            //
            // "Value: <length> | <percentage> | none | inherit"
            // Initial: none
            "max-width" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first()
                    && ident.eq_ignore_ascii_case("none")
                {
                    self.max_width = None;
                } else if let Some(len) = parse_length_value(&decl.value) {
                    self.max_width = Some(self.resolve_length(len));
                }
            }
            // [§ 10.7 'min-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
            //
            // "Value: <length> | <percentage> | inherit"
            // Initial: 0
            "min-height" => {
                if let Some(len) = parse_length_value(&decl.value) {
                    self.min_height = Some(self.resolve_length(len));
                }
            }
            // [§ 10.7 'max-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
            //
            // "Value: <length> | <percentage> | none | inherit"
            // Initial: none
            "max-height" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first()
                    && ident.eq_ignore_ascii_case("none")
                {
                    self.max_height = None;
                } else if let Some(len) = parse_length_value(&decl.value) {
                    self.max_height = Some(self.resolve_length(len));
                }
            }
            // [§ 5.1 'flex-direction'](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
            //
            // "Values: row | row-reverse | column | column-reverse"
            "flex-direction" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(
                        lower.as_str(),
                        "row" | "row-reverse" | "column" | "column-reverse"
                    ) {
                        self.flex_direction = Some(lower);
                    }
                }
            }
            // [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
            //
            // "Values: flex-start | flex-end | center | space-between | space-around"
            "justify-content" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(
                        lower.as_str(),
                        "flex-start" | "flex-end" | "center" | "space-between" | "space-around"
                    ) {
                        self.justify_content = Some(lower);
                    }
                }
            }
            // [§ 7.2 'flex-grow'](https://www.w3.org/TR/css-flexbox-1/#flex-grow-property)
            //
            // "The flex-grow property sets the flex grow factor to the provided
            // `<number>`. Negative values are invalid."
            #[allow(clippy::cast_possible_truncation)]
            "flex-grow" => {
                if let Some(ComponentValue::Token(CSSToken::Number { value, .. })) =
                    decl.value.first()
                {
                    let val = *value as f32;
                    if val >= 0.0 {
                        self.flex_grow = Some(val);
                    }
                }
            }
            // [§ 7.3 'flex-shrink'](https://www.w3.org/TR/css-flexbox-1/#flex-shrink-property)
            //
            // "The flex-shrink property sets the flex shrink factor to the provided
            // `<number>`. Negative values are invalid."
            #[allow(clippy::cast_possible_truncation)]
            "flex-shrink" => {
                if let Some(ComponentValue::Token(CSSToken::Number { value, .. })) =
                    decl.value.first()
                {
                    let val = *value as f32;
                    if val >= 0.0 {
                        self.flex_shrink = Some(val);
                    }
                }
            }
            // [§ 7.1 'flex-basis'](https://www.w3.org/TR/css-flexbox-1/#flex-basis-property)
            //
            // "Values: auto | <length>"
            "flex-basis" => {
                if let Some(first) = decl.value.first()
                    && let Some(auto_len) = parse_single_auto_length(first)
                {
                    self.flex_basis = Some(self.resolve_auto_length(auto_len));
                }
            }
            // [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
            //
            // "Values: left | right | none | inherit"
            "float" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(lower.as_str(), "left" | "right" | "none") {
                        self.float = Some(lower);
                    }
                }
            }
            // [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
            //
            // "Values: left | right | both | none | inherit"
            "clear" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(lower.as_str(), "left" | "right" | "both" | "none") {
                        self.clear = Some(lower);
                    }
                }
            }
            // [§ 9.3.1 'position'](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
            //
            // "Values: static | relative | absolute | fixed"
            // [CSS Positioned Layout Module Level 3 § 3](https://www.w3.org/TR/css-position-3/#position-property)
            // adds "sticky"
            "position" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(
                        lower.as_str(),
                        "static" | "relative" | "absolute" | "fixed" | "sticky"
                    ) {
                        self.position = Some(lower);
                    }
                }
            }
            // [§ 9.3.2 Box offsets: 'top', 'right', 'bottom', 'left'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
            //
            // "Values: <length> | <percentage> | auto | inherit"
            "top" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.top = Some(self.resolve_auto_length(al));
                }
            }
            "right" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.right = Some(self.resolve_auto_length(al));
                }
            }
            "bottom" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.bottom = Some(self.resolve_auto_length(al));
                }
            }
            "left" => {
                if let Some(al) = parse_auto_length_value(&decl.value) {
                    self.left = Some(self.resolve_auto_length(al));
                }
            }
            // [§ 3.1 'list-style-type'](https://www.w3.org/TR/css-lists-3/#list-style-type)
            //
            // "The list-style-type property specifies a counter style or string
            // for the element's marker."
            // Values: disc | circle | square | decimal | lower-alpha | upper-alpha |
            //         lower-roman | upper-roman | none
            "list-style-type" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(
                        lower.as_str(),
                        "disc"
                            | "circle"
                            | "square"
                            | "decimal"
                            | "lower-alpha"
                            | "upper-alpha"
                            | "lower-roman"
                            | "upper-roman"
                            | "none"
                    ) {
                        self.list_style_type = Some(lower);
                    }
                }
            }
            // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
            //
            // "Values: visible | hidden | scroll | auto"
            "overflow" | "overflow-x" | "overflow-y" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    let lower = ident.to_ascii_lowercase();
                    if matches!(lower.as_str(), "visible" | "hidden" | "scroll" | "auto") {
                        self.overflow = Some(lower);
                    }
                }
            }
            // [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
            //
            // "Values: content-box | border-box"
            "box-sizing" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = decl.value.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "border-box" => self.box_sizing_border_box = Some(true),
                        "content-box" => self.box_sizing_border_box = Some(false),
                        _ => {}
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
    /// "Value: `<margin-width>`{1,4} | inherit"
    /// "`<margin-width>` = `<length>` | `<percentage>` | auto"
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
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[0]));
            }
            // RULE 2-VALUE: "the top and bottom margins are set to the first value
            //               and the right and left margins are set to the second."
            2 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[1]));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[1]));
            }
            // RULE 3-VALUE: "the top is set to the first value, the left and right
            //               are set to the second, and the bottom is set to the third."
            3 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[1]));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[1]));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[2]));
            }
            // RULE 4-VALUE: "they apply to the top, right, bottom, and left, respectively."
            4 => {
                self.margin_top = Some(self.resolve_auto_length(auto_lengths[0]));
                self.margin_right = Some(self.resolve_auto_length(auto_lengths[1]));
                self.margin_bottom = Some(self.resolve_auto_length(auto_lengths[2]));
                self.margin_left = Some(self.resolve_auto_length(auto_lengths[3]));
            }
            _ => {}
        }
    }

    /// [§ 6.2 Padding](https://www.w3.org/TR/css-box-4/#paddings)
    fn apply_padding_shorthand(&mut self, values: &[ComponentValue]) {
        let lengths: Vec<LengthValue> = values.iter().filter_map(parse_single_length).collect();

        match lengths.len() {
            1 => {
                self.padding_top = Some(self.resolve_length(lengths[0]));
                self.padding_right = Some(self.resolve_length(lengths[0]));
                self.padding_bottom = Some(self.resolve_length(lengths[0]));
                self.padding_left = Some(self.resolve_length(lengths[0]));
            }
            2 => {
                self.padding_top = Some(self.resolve_length(lengths[0]));
                self.padding_bottom = Some(self.resolve_length(lengths[0]));
                self.padding_right = Some(self.resolve_length(lengths[1]));
                self.padding_left = Some(self.resolve_length(lengths[1]));
            }
            3 => {
                self.padding_top = Some(self.resolve_length(lengths[0]));
                self.padding_right = Some(self.resolve_length(lengths[1]));
                self.padding_left = Some(self.resolve_length(lengths[1]));
                self.padding_bottom = Some(self.resolve_length(lengths[2]));
            }
            4 => {
                self.padding_top = Some(self.resolve_length(lengths[0]));
                self.padding_right = Some(self.resolve_length(lengths[1]));
                self.padding_bottom = Some(self.resolve_length(lengths[2]));
                self.padding_left = Some(self.resolve_length(lengths[3]));
            }
            _ => {}
        }
    }

    /// [§ 3.1 border shorthand](https://www.w3.org/TR/css-backgrounds-3/#the-border-shorthands)
    /// "border: 1px solid #ddd" sets all four borders
    fn apply_border_shorthand(&mut self, values: &[ComponentValue]) {
        if let Some(border) = self.parse_border_side(values) {
            self.border_top = Some(border.clone());
            self.border_right = Some(border.clone());
            self.border_bottom = Some(border.clone());
            self.border_left = Some(border);
        }
    }

    /// [§ 4 Borders](https://www.w3.org/TR/css-backgrounds-3/#borders)
    ///
    /// Get or create the border value for a side with spec initial values.
    ///
    /// [§ 4.1](https://www.w3.org/TR/css-backgrounds-3/#border-color): Initial color = currentcolor
    /// [§ 4.2](https://www.w3.org/TR/css-backgrounds-3/#border-style): Initial style = none
    /// [§ 4.3](https://www.w3.org/TR/css-backgrounds-3/#border-width): Initial width = medium (3px)
    fn default_border(&self) -> BorderValue {
        BorderValue {
            width: LengthValue::Px(3.0),
            style: "none".to_string(),
            color: self.color.clone().unwrap_or(ColorValue::BLACK),
        }
    }

    /// Get or create the `border_top` value.
    fn ensure_border_top(&mut self) -> &mut BorderValue {
        if self.border_top.is_none() {
            self.border_top = Some(self.default_border());
        }
        self.border_top.as_mut().unwrap()
    }

    /// Get or create the `border_right` value.
    fn ensure_border_right(&mut self) -> &mut BorderValue {
        if self.border_right.is_none() {
            self.border_right = Some(self.default_border());
        }
        self.border_right.as_mut().unwrap()
    }

    /// Get or create the `border_bottom` value.
    fn ensure_border_bottom(&mut self) -> &mut BorderValue {
        if self.border_bottom.is_none() {
            self.border_bottom = Some(self.default_border());
        }
        self.border_bottom.as_mut().unwrap()
    }

    /// Get or create the `border_left` value.
    fn ensure_border_left(&mut self) -> &mut BorderValue {
        if self.border_left.is_none() {
            self.border_left = Some(self.default_border());
        }
        self.border_left.as_mut().unwrap()
    }

    /// [§ 4.1 'border-color'](https://www.w3.org/TR/css-backgrounds-3/#border-color)
    ///
    /// "Value: <color>{1,4}"
    ///
    /// Shorthand following the same 1-4 value expansion as margin/padding.
    fn apply_border_color_shorthand(&mut self, values: &[ComponentValue]) {
        let colors: Vec<ColorValue> = values.iter().filter_map(parse_single_color).collect();

        match colors.len() {
            1 => {
                self.ensure_border_top().color = colors[0].clone();
                self.ensure_border_right().color = colors[0].clone();
                self.ensure_border_bottom().color = colors[0].clone();
                self.ensure_border_left().color = colors[0].clone();
            }
            2 => {
                self.ensure_border_top().color = colors[0].clone();
                self.ensure_border_bottom().color = colors[0].clone();
                self.ensure_border_right().color = colors[1].clone();
                self.ensure_border_left().color = colors[1].clone();
            }
            3 => {
                self.ensure_border_top().color = colors[0].clone();
                self.ensure_border_right().color = colors[1].clone();
                self.ensure_border_left().color = colors[1].clone();
                self.ensure_border_bottom().color = colors[2].clone();
            }
            4 => {
                self.ensure_border_top().color = colors[0].clone();
                self.ensure_border_right().color = colors[1].clone();
                self.ensure_border_bottom().color = colors[2].clone();
                self.ensure_border_left().color = colors[3].clone();
            }
            _ => {}
        }
    }

    /// [§ 4.3 'border-width'](https://www.w3.org/TR/css-backgrounds-3/#border-width)
    ///
    /// "Value: <line-width>{1,4}"
    ///
    /// Shorthand following the same 1-4 value expansion as margin/padding.
    fn apply_border_width_shorthand(&mut self, values: &[ComponentValue]) {
        let lengths: Vec<LengthValue> = values.iter().filter_map(parse_single_length).collect();

        match lengths.len() {
            1 => {
                let w = self.resolve_length(lengths[0]);
                self.ensure_border_top().width = w;
                self.ensure_border_right().width = w;
                self.ensure_border_bottom().width = w;
                self.ensure_border_left().width = w;
            }
            2 => {
                let tb = self.resolve_length(lengths[0]);
                let lr = self.resolve_length(lengths[1]);
                self.ensure_border_top().width = tb;
                self.ensure_border_bottom().width = tb;
                self.ensure_border_right().width = lr;
                self.ensure_border_left().width = lr;
            }
            3 => {
                let t = self.resolve_length(lengths[0]);
                let lr = self.resolve_length(lengths[1]);
                let b = self.resolve_length(lengths[2]);
                self.ensure_border_top().width = t;
                self.ensure_border_right().width = lr;
                self.ensure_border_left().width = lr;
                self.ensure_border_bottom().width = b;
            }
            4 => {
                let t = self.resolve_length(lengths[0]);
                let r = self.resolve_length(lengths[1]);
                let b = self.resolve_length(lengths[2]);
                let l = self.resolve_length(lengths[3]);
                self.ensure_border_top().width = t;
                self.ensure_border_right().width = r;
                self.ensure_border_bottom().width = b;
                self.ensure_border_left().width = l;
            }
            _ => {}
        }
    }

    /// [§ 4.2 'border-style'](https://www.w3.org/TR/css-backgrounds-3/#border-style)
    ///
    /// "Value: <line-style>{1,4}"
    ///
    /// Shorthand following the same 1-4 value expansion as margin/padding.
    fn apply_border_style_shorthand(&mut self, values: &[ComponentValue]) {
        let styles: Vec<String> = values
            .iter()
            .filter_map(Self::parse_border_style)
            .collect();

        match styles.len() {
            1 => {
                self.ensure_border_top().style = styles[0].clone();
                self.ensure_border_right().style = styles[0].clone();
                self.ensure_border_bottom().style = styles[0].clone();
                self.ensure_border_left().style = styles[0].clone();
            }
            2 => {
                self.ensure_border_top().style = styles[0].clone();
                self.ensure_border_bottom().style = styles[0].clone();
                self.ensure_border_right().style = styles[1].clone();
                self.ensure_border_left().style = styles[1].clone();
            }
            3 => {
                self.ensure_border_top().style = styles[0].clone();
                self.ensure_border_right().style = styles[1].clone();
                self.ensure_border_left().style = styles[1].clone();
                self.ensure_border_bottom().style = styles[2].clone();
            }
            4 => {
                self.ensure_border_top().style = styles[0].clone();
                self.ensure_border_right().style = styles[1].clone();
                self.ensure_border_bottom().style = styles[2].clone();
                self.ensure_border_left().style = styles[3].clone();
            }
            _ => {}
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
                    .map_or(DEFAULT_FONT_SIZE_PX, LengthValue::to_px);
                LengthValue::Px(em * base)
            }
            other => other,
        }
    }

    /// [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    ///
    /// Resolve relative length units (em) to absolute units (px) for `AutoLength`.
    /// 'auto' values are preserved unchanged.
    fn resolve_auto_length(&self, al: AutoLength) -> AutoLength {
        match al {
            AutoLength::Auto => AutoLength::Auto,
            AutoLength::Length(len) => AutoLength::Length(self.resolve_length(len)),
        }
    }

    /// [§ 4 Logical Property Groups](https://drafts.csswg.org/css-logical-1/#logical-property-groups)
    ///
    /// Check if a margin declaration should update the value for a physical side.
    /// Returns true if:
    /// - No value has been set yet (`source_order` is None), or
    /// - The new `source_order` is greater than or equal to the existing one
    ///
    /// This implements cascade resolution: later declarations win, and logical
    /// and physical properties compete based on source order.
    const fn should_update_margin(&self, side: PhysicalSide, new_order: u32) -> bool {
        let existing_order = match side {
            PhysicalSide::Top => self.margin_top_source_order,
            PhysicalSide::Right => self.margin_right_source_order,
            PhysicalSide::Bottom => self.margin_bottom_source_order,
            PhysicalSide::Left => self.margin_left_source_order,
        };

        match existing_order {
            None => true,
            Some(existing) => new_order >= existing,
        }
    }

    /// [§ 4.2 Flow-Relative Margins](https://drafts.csswg.org/css-logical-1/#margin-properties)
    ///
    /// Set the margin value for a physical side and update its `source_order`.
    /// Used by logical properties to update the corresponding physical property.
    fn set_margin_for_side(&mut self, side: PhysicalSide, value: AutoLength, source_order: u32) {
        let resolved = self.resolve_auto_length(value);
        match side {
            PhysicalSide::Top => {
                self.margin_top = Some(resolved);
                self.margin_top_source_order = Some(source_order);
            }
            PhysicalSide::Right => {
                self.margin_right = Some(resolved);
                self.margin_right_source_order = Some(source_order);
            }
            PhysicalSide::Bottom => {
                self.margin_bottom = Some(resolved);
                self.margin_bottom_source_order = Some(source_order);
            }
            PhysicalSide::Left => {
                self.margin_left = Some(resolved);
                self.margin_left_source_order = Some(source_order);
            }
        }
    }

    /// [§ 4.4 border-top, border-right, border-bottom, border-left](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
    ///
    /// Parse a single border side value.
    ///
    /// Syntax: `<line-width> || <line-style> || <color>`
    ///
    /// [§ 4.1 Line Width](https://www.w3.org/TR/css-backgrounds-3/#border-width)
    /// "`<line-width>` = `<length>` \[0,Inf\] | thin | medium | thick"
    ///
    /// [§ 4.2 Line Style](https://www.w3.org/TR/css-backgrounds-3/#border-style)
    /// "`<line-style>` = none | hidden | dotted | dashed | solid | double | groove | ridge | inset | outset"
    ///
    /// [§ 4.3 Line Color](https://www.w3.org/TR/css-backgrounds-3/#border-color)
    /// "`<color>`"
    ///
    /// Values can appear in any order. Missing values use initial values:
    /// - width: medium (typically 3px)
    /// - style: solid
    /// - color: currentcolor (use computed 'color' property, or black)
    fn parse_border_side(&self, values: &[ComponentValue]) -> Option<BorderValue> {
        let mut width = None;
        let mut style = None;
        let mut color = None;

        for v in values {
            if width.is_none()
                && let Some(len) = parse_single_length(v)
            {
                width = Some(self.resolve_length(len));
            } else if color.is_none()
                && let Some(c) = parse_single_color(v)
            {
                color = Some(c);
            } else if style.is_none()
                && let Some(s) = Self::parse_border_style(v)
            {
                style = Some(s);
            }
        }

        // Return Some if at least one value was parsed
        (width.is_some() || style.is_some() || color.is_some()).then(|| BorderValue {
            width: width.unwrap_or(LengthValue::Px(3.0)),
            style: style.unwrap_or_else(|| "solid".to_string()),
            color: color.unwrap_or_else(|| self.color.clone().unwrap_or(ColorValue::BLACK)),
        })
    }

    /// Parse a border-style keyword.
    fn parse_border_style(v: &ComponentValue) -> Option<String> {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
            let lower = ident.to_ascii_lowercase();
            matches!(
                lower.as_str(),
                "solid"
                    | "dashed"
                    | "dotted"
                    | "double"
                    | "none"
                    | "hidden"
                    | "groove"
                    | "ridge"
                    | "inset"
                    | "outset"
            )
            .then_some(lower)
        } else {
            None
        }
    }
}
