//! CSS Computed Style
//!
//! [§ 4.4 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
//! "The computed value is the result of resolving the specified value..."

use std::collections::HashMap;

use serde::Serialize;

use crate::layout::float::{ClearSide, FloatSide};
use crate::layout::inline::{FontStyle, TextAlign};
use crate::layout::positioned::PositionType;
use crate::parser::{ComponentValue, Declaration};
use crate::style::substitute::{contains_var, substitute_var};
use crate::tokenizer::CSSToken;
use crate::{AutoLength, BorderValue, BoxShadow, ColorValue, LengthValue};
use koala_common::warning::warn_once;

/// [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
///
/// "This property specifies whether content of a block container element
/// is clipped when it overflows the element's box."
///
/// Values: visible | hidden | scroll | auto
/// Initial: visible
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Overflow {
    /// "The content is not clipped."
    Visible,
    /// "The content is clipped and no scrolling mechanism should be provided."
    Hidden,
    /// "The content is clipped and a scrolling mechanism should be provided."
    Scroll,
    /// "The behavior is UA-dependent, but should provide a scrolling mechanism
    /// for overflowing boxes."
    Auto,
}

/// [§ 5.1 'flex-direction'](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
///
/// "The flex-direction property specifies how flex items are placed in
/// the flex container, by setting the direction of the flex container's
/// main axis."
///
/// Values: row | row-reverse | column | column-reverse
/// Initial: row
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum FlexDirection {
    /// "The flex container's main axis has the same orientation as the
    /// inline axis of the current writing mode."
    #[default]
    Row,
    /// "Same as row, except the main-start and main-end directions are swapped."
    RowReverse,
    /// "The flex container's main axis has the same orientation as the
    /// block axis of the current writing mode."
    Column,
    /// "Same as column, except the main-start and main-end directions are swapped."
    ColumnReverse,
}

/// [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
///
/// "The justify-content property aligns flex items along the main axis
/// of the current line of the flex container."
///
/// Values: flex-start | flex-end | center | space-between | space-around
/// Initial: flex-start
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum JustifyContent {
    /// "Flex items are packed toward the start of the line."
    #[default]
    FlexStart,
    /// "Flex items are packed toward the end of the line."
    FlexEnd,
    /// "Flex items are packed toward the center of the line."
    Center,
    /// "Flex items are evenly distributed in the line."
    SpaceBetween,
    /// "Flex items are evenly distributed in the line, with half-size
    /// spaces on either end."
    SpaceAround,
}

/// [§ 8.3 'align-items'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
///
/// "The align-items property sets the default alignment for all of the
/// flex container's items, including anonymous flex items."
///
/// Values: flex-start | flex-end | center | baseline | stretch
/// Initial: stretch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum AlignItems {
    /// "The cross-start margin edge of the flex item is placed flush with
    /// the cross-start edge of the line."
    FlexStart,
    /// "The cross-end margin edge of the flex item is placed flush with
    /// the cross-end edge of the line."
    FlexEnd,
    /// "The flex item's margin box is centered in the cross axis within
    /// the line."
    Center,
    /// "The flex item participates in baseline alignment."
    Baseline,
    /// "If the cross size property of the flex item computes to auto, and
    /// neither of the cross-axis margins are auto, the flex item is
    /// stretched."
    #[default]
    Stretch,
}

/// [§ 8.3 'align-self'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
///
/// "Flex items can be aligned in the cross axis of the current line of
/// the flex container, similar to justify-content but in the perpendicular
/// direction. align-self sets the alignment for individual flex items."
///
/// Values: auto | flex-start | flex-end | center | baseline | stretch
/// Initial: auto (inherits align-items from container)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum AlignSelf {
    /// "Defers cross-axis alignment control to the value of align-items
    /// on the parent box."
    #[default]
    Auto,
    /// "The cross-start margin edge of the flex item is placed flush with
    /// the cross-start edge of the line."
    FlexStart,
    /// "The cross-end margin edge of the flex item is placed flush with
    /// the cross-end edge of the line."
    FlexEnd,
    /// "The flex item's margin box is centered in the cross axis within
    /// the line."
    Center,
    /// "The flex item participates in baseline alignment."
    Baseline,
    /// "If the cross size property of the flex item computes to auto, and
    /// neither of the cross-axis margins are auto, the flex item is
    /// stretched."
    Stretch,
}

/// [§ 5.2 'flex-wrap'](https://www.w3.org/TR/css-flexbox-1/#flex-wrap-property)
///
/// "The flex-wrap property controls whether the flex container is single-line
/// or multi-line, and the direction of the cross-axis, which determines the
/// direction new lines are stacked in."
///
/// Values: nowrap | wrap | wrap-reverse
/// Initial: nowrap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum FlexWrap {
    /// "The flex container is single-line."
    #[default]
    Nowrap,
    /// "The flex container is multi-line."
    Wrap,
    /// "Same as wrap, except the cross-start and cross-end directions
    /// are swapped."
    WrapReverse,
}

/// [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
///
/// "This property declares how white space inside the element is handled."
///
/// Values: normal | pre | nowrap | pre-wrap | pre-line
/// Initial: normal
/// Inherited: yes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum WhiteSpace {
    /// "This value directs user agents to collapse sequences of white space,
    /// and break lines as necessary to fill line boxes."
    #[default]
    Normal,
    /// "This value prevents user agents from collapsing sequences of white
    /// space. Lines are only broken at preserved newline characters."
    Pre,
    /// "This value collapses white space as for 'normal', but suppresses
    /// line breaks (text wrapping) within text."
    Nowrap,
    /// "This value prevents user agents from collapsing sequences of white
    /// space. Lines are broken at preserved newline characters, and as
    /// necessary to fill line boxes."
    PreWrap,
    /// "This value directs user agents to collapse sequences of white space.
    /// Lines are broken at preserved newline characters, and as necessary
    /// to fill line boxes."
    PreLine,
}

/// [§ 11.2 'visibility'](https://www.w3.org/TR/CSS2/visufx.html#visibility)
///
/// "The 'visibility' property specifies whether the boxes generated by an
/// element are rendered. Invisible boxes still affect layout."
///
/// Values: visible | hidden | collapse
/// Initial: visible
/// Inherited: yes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum Visibility {
    /// "The generated box is visible."
    #[default]
    Visible,
    /// "The generated box is invisible (fully transparent, nothing is drawn),
    /// but still affects layout."
    Hidden,
    /// "For table-related elements, same as 'hidden'. For other elements,
    /// same as 'hidden'."
    Collapse,
}

/// [§ 7.2 Explicit Track Sizing](https://www.w3.org/TR/css-grid-1/#track-sizing)
///
/// "A track sizing function can be specified as a length, a percentage of the
/// grid container's size, a measurement of the contents occupying the column
/// or row, or a fraction of the free space in the grid."
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum TrackSize {
    /// A fixed length in resolved px.
    Fixed(f32),
    /// [§ 7.8.3 Flexible Lengths: the fr unit](https://www.w3.org/TR/css-grid-1/#fr-unit)
    ///
    /// "A flexible length or `<flex>` is a dimension with the fr unit, which
    /// represents a fraction of the leftover space in the grid container."
    Fr(f32),
    /// [§ 7.2.1](https://www.w3.org/TR/css-grid-1/#auto-tracks)
    ///
    /// "As a maximum, identical to max-content. As a minimum, represents the
    /// largest minimum size (as specified by min-width/min-height) of the grid
    /// items occupying the grid track."
    Auto,
}

/// [§ 7.2 Explicit Track Sizing](https://www.w3.org/TR/css-grid-1/#track-sizing)
///
/// A list of track sizes defining either the column or row template.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct TrackList {
    /// The individual track sizing functions in order.
    pub sizes: Vec<TrackSize>,
}

/// [§ 8.3 Line-based Placement](https://www.w3.org/TR/css-grid-1/#line-placement)
///
/// "Grid items can be placed by specifying a grid line by its numeric index
/// or by its name."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GridLine {
    /// "Contributes nothing to the grid item's placement."
    Auto,
    /// "Refers to the Nth grid line."
    /// 1-based; negative values count from the end.
    Line(i32),
    /// "Contributes a grid span to the grid item's placement."
    Span(u32),
}

/// [§ 7.6 Automatic Placement](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
///
/// "Grid items that aren't explicitly placed are automatically placed into
/// an unoccupied space in the grid container."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum GridAutoFlow {
    /// "The auto-placement algorithm places items by filling each row in
    /// turn, adding new rows as necessary."
    #[default]
    Row,
    /// "The auto-placement algorithm places items by filling each column in
    /// turn, adding new columns as necessary."
    Column,
}

/// [§ 3.1 'list-style-type'](https://www.w3.org/TR/css-lists-3/#list-style-type)
///
/// "The list-style-type property specifies a counter style or string for
/// the element's marker."
///
/// Values: disc | circle | square | decimal | lower-alpha | upper-alpha |
///         lower-roman | upper-roman | none
/// Initial: disc
/// Inherited: yes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum ListStyleType {
    /// Filled circle bullet.
    #[default]
    Disc,
    /// Open circle bullet.
    Circle,
    /// Filled square bullet.
    Square,
    /// Decimal numbers (1, 2, 3, ...).
    Decimal,
    /// Lowercase alphabetic (a, b, c, ...).
    LowerAlpha,
    /// Uppercase alphabetic (A, B, C, ...).
    UpperAlpha,
    /// Lowercase Roman numerals (i, ii, iii, ...).
    LowerRoman,
    /// Uppercase Roman numerals (I, II, III, ...).
    UpperRoman,
    /// No marker.
    None,
}

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
    pub font_style: Option<FontStyle>,
    /// [§ 4.2 'line-height'](https://www.w3.org/TR/css-inline-3/#line-height-property)
    pub line_height: Option<f64>,

    /// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
    ///
    /// "This property describes how inline-level content of a block
    /// container is aligned."
    ///
    pub text_align: Option<TextAlign>,

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
    pub flex_direction: Option<FlexDirection>,

    /// [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
    ///
    /// "The justify-content property aligns flex items along the main axis
    /// of the current line of the flex container."
    ///
    /// Values: flex-start | flex-end | center | space-between | space-around
    /// Initial: flex-start
    pub justify_content: Option<JustifyContent>,

    /// [§ 8.3 'align-items'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
    ///
    /// "The align-items property sets the default alignment for all of the
    /// flex container's items."
    ///
    /// Values: flex-start | flex-end | center | baseline | stretch
    /// Initial: stretch
    pub align_items: Option<AlignItems>,

    /// [§ 8.3 'align-self'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
    ///
    /// "align-self allows this default alignment to be overridden for
    /// individual flex items."
    ///
    /// Values: auto | flex-start | flex-end | center | baseline | stretch
    /// Initial: auto
    pub align_self: Option<AlignSelf>,

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

    /// [§ 5.2 'flex-wrap'](https://www.w3.org/TR/css-flexbox-1/#flex-wrap-property)
    ///
    /// "The flex-wrap property controls whether the flex container is
    /// single-line or multi-line."
    ///
    /// Values: nowrap | wrap | wrap-reverse
    /// Initial: nowrap
    /// Inherited: no
    pub flex_wrap: Option<FlexWrap>,

    // ===== Grid layout properties =====

    /// [§ 7.2 'grid-template-columns'](https://www.w3.org/TR/css-grid-1/#track-sizing)
    ///
    /// "These properties specify, as a space-separated track list, the line
    /// names and track sizing functions of the grid."
    /// Inherited: no
    pub grid_template_columns: Option<TrackList>,

    /// [§ 7.2 'grid-template-rows'](https://www.w3.org/TR/css-grid-1/#track-sizing)
    /// Inherited: no
    pub grid_template_rows: Option<TrackList>,

    /// [§ 7.6 'grid-auto-flow'](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
    ///
    /// "Controls how the auto-placement algorithm works."
    /// Values: row | column
    /// Initial: row
    /// Inherited: no
    pub grid_auto_flow: Option<GridAutoFlow>,

    /// [§ 10.1 'row-gap'](https://www.w3.org/TR/css-align-3/#row-gap)
    /// Inherited: no
    pub row_gap: Option<LengthValue>,

    /// [§ 10.1 'column-gap'](https://www.w3.org/TR/css-align-3/#column-gap)
    /// Inherited: no
    pub column_gap: Option<LengthValue>,

    /// [§ 8.3 'grid-column-start'](https://www.w3.org/TR/css-grid-1/#line-placement)
    /// Inherited: no
    pub grid_column_start: Option<GridLine>,

    /// [§ 8.3 'grid-column-end'](https://www.w3.org/TR/css-grid-1/#line-placement)
    /// Inherited: no
    pub grid_column_end: Option<GridLine>,

    /// [§ 8.3 'grid-row-start'](https://www.w3.org/TR/css-grid-1/#line-placement)
    /// Inherited: no
    pub grid_row_start: Option<GridLine>,

    /// [§ 8.3 'grid-row-end'](https://www.w3.org/TR/css-grid-1/#line-placement)
    /// Inherited: no
    pub grid_row_end: Option<GridLine>,

    // ===== Positioning properties =====
    /// [§ 9.3.1 'position'](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
    ///
    /// "The 'position' and 'float' properties determine which of the CSS 2
    /// positioning algorithms is used to calculate the position of a box."
    ///
    /// Values: static | relative | absolute | fixed | sticky
    /// Initial: static
    /// Inherited: no
    pub position: Option<PositionType>,

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
    pub float: Option<FloatSide>,

    /// [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
    ///
    /// "This property indicates which sides of an element's box(es) may not
    /// be adjacent to an earlier floating box."
    ///
    /// Values: left | right | both | none
    /// Initial: none
    /// Inherited: no
    pub clear: Option<ClearSide>,

    /// [§ 3.1 'list-style-type'](https://www.w3.org/TR/css-lists-3/#list-style-type)
    ///
    /// "The list-style-type property specifies a counter style or string for
    /// the element's marker."
    ///
    /// Values: disc | circle | square | decimal | lower-alpha | upper-alpha |
    ///         lower-roman | upper-roman | none
    /// Initial: disc
    /// Inherited: yes
    pub list_style_type: Option<ListStyleType>,

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
    pub overflow: Option<Overflow>,

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

    /// [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
    ///
    /// "This property declares how white space inside the element is handled."
    ///
    /// Values: normal | pre | nowrap | pre-wrap | pre-line
    /// Initial: normal
    /// Inherited: yes
    pub white_space: Option<WhiteSpace>,

    /// [§ 11.2 'visibility'](https://www.w3.org/TR/CSS2/visufx.html#visibility)
    ///
    /// "The 'visibility' property specifies whether the boxes generated by an
    /// element are rendered. Invisible boxes still affect layout."
    ///
    /// Values: visible | hidden | collapse
    /// Initial: visible
    /// Inherited: yes
    pub visibility: Option<Visibility>,

    /// [§ 3.2 'opacity'](https://www.w3.org/TR/css-color-4/#transparency)
    ///
    /// "Opacity can be thought of as a postprocessing operation. Conceptually,
    /// after the element is rendered into an RGBA offscreen image, the opacity
    /// setting specifies how to blend the offscreen rendering into the current
    /// composite rendering."
    ///
    /// Values: `<number>` (0.0 to 1.0)
    /// Initial: 1
    /// Inherited: no
    pub opacity: Option<f32>,

    /// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
    ///
    /// "The 'box-shadow' property attaches one or more drop-shadows to the box."
    ///
    /// Values: none | `<shadow>#`
    /// Initial: none
    /// Inherited: no
    pub box_shadow: Option<Vec<BoxShadow>>,

    /// [§ 2 Custom Properties](https://www.w3.org/TR/css-variables-1/#defining-variables)
    ///
    /// "A custom property is any property whose name starts with two dashes."
    /// Custom properties are inherited by default (§ 2: "Inherited: yes").
    /// Values are stored as resolved component values (`var()` already substituted).
    #[serde(skip)]
    pub custom_properties: HashMap<String, Vec<ComponentValue>>,

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
        // [§ 2 Custom Properties](https://www.w3.org/TR/css-variables-1/#defining-variables)
        //
        // "A custom property is any property whose name starts with two dashes."
        // Custom property names are case-sensitive (§ 2) — do NOT lowercase.
        // Store raw component values; var() resolution happens after all
        // declarations are applied (see resolve_custom_properties).
        if decl.name.starts_with("--") {
            let _ = self
                .custom_properties
                .insert(decl.name.clone(), decl.value.clone());
            return;
        }

        // [§ 3](https://www.w3.org/TR/css-variables-1/#using-variables)
        //
        // "If a property contains one or more var() functions, and those functions
        // are syntactically valid, the entire property's grammar must be assumed
        // to be valid at parse time. It is only syntax-checked at computed-value
        // time, after var() functions have been substituted."
        let resolved_values: Vec<ComponentValue>;
        let values: &[ComponentValue] = if contains_var(&decl.value) {
            match substitute_var(&decl.value, &self.custom_properties, 0) {
                Some(v) => {
                    resolved_values = v;
                    &resolved_values
                }
                None => return, // Invalid at computed-value time
            }
        } else {
            &decl.value
        };

        match decl.name.to_ascii_lowercase().as_str() {
            // [§ 2 The display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
            //
            // "The display property defines an element's display type..."
            "display" => {
                if let Some(display) = parse_display_value(values) {
                    self.display = Some(display);
                    self.display_none = false;
                } else if is_display_none(values) {
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
                if let Some(wm) = parse_writing_mode(values) {
                    self.writing_mode = wm;
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(values) {
                    self.color = Some(color);
                }
            }
            "background-color" => {
                if let Some(color) = parse_color_value(values) {
                    self.background_color = Some(color);
                }
            }
            "font-family" => {
                if let Some(family) = parse_font_family(values) {
                    self.font_family = Some(family);
                }
            }
            "line-height" => {
                if let Some(lh) = parse_line_height(values) {
                    self.line_height = Some(lh);
                }
            }
            // [§ 3.2 font-weight](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
            "font-weight" => {
                if let Some(weight) = parse_font_weight(values) {
                    self.font_weight = Some(weight);
                }
            }
            // [§ 3.3 font-style](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
            //
            // "This property allows italic or oblique faces to be selected."
            // Values: normal | italic | oblique
            "font-style" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "normal" => self.font_style = Some(FontStyle::Normal),
                        "italic" => self.font_style = Some(FontStyle::Italic),
                        "oblique" => self.font_style = Some(FontStyle::Oblique),
                        _ => {}
                    }
                }
            }
            // [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
            //
            // "Value: left | right | center | justify | inherit"
            "text-align" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "left" => self.text_align = Some(TextAlign::Left),
                        "right" => self.text_align = Some(TextAlign::Right),
                        "center" => self.text_align = Some(TextAlign::Center),
                        "justify" => self.text_align = Some(TextAlign::Justify),
                        _ => {}
                    }
                }
            }
            // [§ 9.2 Shorthand properties](https://www.w3.org/TR/css-cascade-4/#shorthand)
            "margin" => {
                self.apply_margin_shorthand(values);
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
                if let Some(al) = parse_auto_length_value(values)
                    && self.should_update_margin(PhysicalSide::Top, decl.source_order)
                {
                    self.margin_top = Some(self.resolve_auto_length(al));
                    self.margin_top_source_order = Some(decl.source_order);
                }
            }
            "margin-right" => {
                if let Some(al) = parse_auto_length_value(values)
                    && self.should_update_margin(PhysicalSide::Right, decl.source_order)
                {
                    self.margin_right = Some(self.resolve_auto_length(al));
                    self.margin_right_source_order = Some(decl.source_order);
                }
            }
            "margin-bottom" => {
                if let Some(al) = parse_auto_length_value(values)
                    && self.should_update_margin(PhysicalSide::Bottom, decl.source_order)
                {
                    self.margin_bottom = Some(self.resolve_auto_length(al));
                    self.margin_bottom_source_order = Some(decl.source_order);
                }
            }
            "margin-left" => {
                if let Some(al) = parse_auto_length_value(values)
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
                if let Some(al) = parse_auto_length_value(values) {
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
                if let Some(al) = parse_auto_length_value(values) {
                    let physical_side = self.writing_mode.block_end_physical();

                    if self.should_update_margin(physical_side, decl.source_order) {
                        self.margin_block_end = Some(self.resolve_auto_length(al));
                        self.set_margin_for_side(physical_side, al, decl.source_order);
                    }
                }
            }

            "padding" => {
                self.apply_padding_shorthand(values);
            }
            "padding-top" => {
                if let Some(len) = parse_length_value(values) {
                    self.padding_top = Some(self.resolve_length(len));
                }
            }
            "padding-right" => {
                if let Some(len) = parse_length_value(values) {
                    self.padding_right = Some(self.resolve_length(len));
                }
            }
            "padding-bottom" => {
                if let Some(len) = parse_length_value(values) {
                    self.padding_bottom = Some(self.resolve_length(len));
                }
            }
            "padding-left" => {
                if let Some(len) = parse_length_value(values) {
                    self.padding_left = Some(self.resolve_length(len));
                }
            }
            "border" => {
                self.apply_border_shorthand(values);
            }
            // [§ 4.4 border-top](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            //
            // "The 'border-top' shorthand property sets the width, style, and color
            // of the top border."
            //
            // Syntax: <line-width> || <line-style> || <color>
            // (values can appear in any order)
            "border-top" => {
                if let Some(border) = self.parse_border_side(values) {
                    self.border_top = Some(border);
                }
            }
            // [§ 4.4 border-right](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            "border-right" => {
                if let Some(border) = self.parse_border_side(values) {
                    self.border_right = Some(border);
                }
            }
            // [§ 4.4 border-bottom](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            "border-bottom" => {
                if let Some(border) = self.parse_border_side(values) {
                    self.border_bottom = Some(border);
                }
            }
            // [§ 4.4 border-left](https://www.w3.org/TR/css-backgrounds-3/#border-shorthands)
            "border-left" => {
                if let Some(border) = self.parse_border_side(values) {
                    self.border_left = Some(border);
                }
            }
            // [§ 4.1 'border-top-color', etc.](https://www.w3.org/TR/css-backgrounds-3/#border-color)
            //
            // "These properties set the foreground color of the border
            // specified by the border-top, border-right, border-bottom,
            // and border-left properties respectively."
            "border-top-color" => {
                if let Some(color) = parse_color_value(values) {
                    self.ensure_border_top().color = color;
                }
            }
            "border-right-color" => {
                if let Some(color) = parse_color_value(values) {
                    self.ensure_border_right().color = color;
                }
            }
            "border-bottom-color" => {
                if let Some(color) = parse_color_value(values) {
                    self.ensure_border_bottom().color = color;
                }
            }
            "border-left-color" => {
                if let Some(color) = parse_color_value(values) {
                    self.ensure_border_left().color = color;
                }
            }
            // [§ 4.3 'border-top-width', etc.](https://www.w3.org/TR/css-backgrounds-3/#border-width)
            //
            // "These properties set the thickness of the border."
            // "<line-width> = <length [0,∞]> | thin | medium | thick"
            "border-top-width" => {
                if let Some(len) = parse_length_value(values) {
                    self.ensure_border_top().width = self.resolve_length(len);
                }
            }
            "border-right-width" => {
                if let Some(len) = parse_length_value(values) {
                    self.ensure_border_right().width = self.resolve_length(len);
                }
            }
            "border-bottom-width" => {
                if let Some(len) = parse_length_value(values) {
                    self.ensure_border_bottom().width = self.resolve_length(len);
                }
            }
            "border-left-width" => {
                if let Some(len) = parse_length_value(values) {
                    self.ensure_border_left().width = self.resolve_length(len);
                }
            }
            // [§ 4.2 'border-top-style', etc.](https://www.w3.org/TR/css-backgrounds-3/#border-style)
            //
            // "These properties set the style of the border."
            // "<line-style> = none | hidden | dotted | dashed | solid | double |
            //                 groove | ridge | inset | outset"
            "border-top-style" => {
                if let Some(first) = values.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_top().style = s;
                }
            }
            "border-right-style" => {
                if let Some(first) = values.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_right().style = s;
                }
            }
            "border-bottom-style" => {
                if let Some(first) = values.first()
                    && let Some(s) = Self::parse_border_style(first)
                {
                    self.ensure_border_bottom().style = s;
                }
            }
            "border-left-style" => {
                if let Some(first) = values.first()
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
                self.apply_border_color_shorthand(values);
            }
            // [§ 4.3 'border-width'](https://www.w3.org/TR/css-backgrounds-3/#border-width)
            //
            // "The 'border-width' property is a shorthand for setting
            // 'border-top-width', 'border-right-width', 'border-bottom-width',
            // and 'border-left-width'."
            "border-width" => {
                self.apply_border_width_shorthand(values);
            }
            // [§ 4.2 'border-style'](https://www.w3.org/TR/css-backgrounds-3/#border-style)
            //
            // "The 'border-style' property is a shorthand for setting
            // 'border-top-style', 'border-right-style', 'border-bottom-style',
            // and 'border-left-style'."
            "border-style" => {
                self.apply_border_style_shorthand(values);
            }
            "background" => {
                self.apply_background_shorthand(values);
            }
            "font-size" => {
                if let Some(len) = parse_length_value(values) {
                    self.font_size = Some(self.resolve_length(len));
                }
            }
            // [§ 10.2 'width'](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
            //
            // "This property specifies the content width of boxes."
            // "Value: `<length>` | `<percentage>` | auto | inherit"
            "width" => {
                if let Some(first) = values.first()
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
                if let Some(first) = values.first()
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
                if let Some(len) = parse_length_value(values) {
                    self.min_width = Some(self.resolve_length(len));
                }
            }
            // [§ 10.4 'max-width'](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
            //
            // "Value: <length> | <percentage> | none | inherit"
            // Initial: none
            "max-width" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first()
                    && ident.eq_ignore_ascii_case("none")
                {
                    self.max_width = None;
                } else if let Some(len) = parse_length_value(values) {
                    self.max_width = Some(self.resolve_length(len));
                }
            }
            // [§ 10.7 'min-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
            //
            // "Value: <length> | <percentage> | inherit"
            // Initial: 0
            "min-height" => {
                if let Some(len) = parse_length_value(values) {
                    self.min_height = Some(self.resolve_length(len));
                }
            }
            // [§ 10.7 'max-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
            //
            // "Value: <length> | <percentage> | none | inherit"
            // Initial: none
            "max-height" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first()
                    && ident.eq_ignore_ascii_case("none")
                {
                    self.max_height = None;
                } else if let Some(len) = parse_length_value(values) {
                    self.max_height = Some(self.resolve_length(len));
                }
            }
            // [§ 5.1 'flex-direction'](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
            //
            // "Values: row | row-reverse | column | column-reverse"
            "flex-direction" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "row" => self.flex_direction = Some(FlexDirection::Row),
                        "row-reverse" => self.flex_direction = Some(FlexDirection::RowReverse),
                        "column" => self.flex_direction = Some(FlexDirection::Column),
                        "column-reverse" => {
                            self.flex_direction = Some(FlexDirection::ColumnReverse)
                        }
                        _ => {}
                    }
                }
            }
            // [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
            //
            // "Values: flex-start | flex-end | center | space-between | space-around"
            "justify-content" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "flex-start" => self.justify_content = Some(JustifyContent::FlexStart),
                        "flex-end" => self.justify_content = Some(JustifyContent::FlexEnd),
                        "center" => self.justify_content = Some(JustifyContent::Center),
                        "space-between" => {
                            self.justify_content = Some(JustifyContent::SpaceBetween)
                        }
                        "space-around" => self.justify_content = Some(JustifyContent::SpaceAround),
                        _ => {}
                    }
                }
            }
            // [§ 8.3 'align-items'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
            //
            // "Values: flex-start | flex-end | center | baseline | stretch"
            "align-items" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "flex-start" | "start" => self.align_items = Some(AlignItems::FlexStart),
                        "flex-end" | "end" => self.align_items = Some(AlignItems::FlexEnd),
                        "center" => self.align_items = Some(AlignItems::Center),
                        "baseline" => self.align_items = Some(AlignItems::Baseline),
                        "stretch" => self.align_items = Some(AlignItems::Stretch),
                        _ => {}
                    }
                }
            }
            // [§ 8.3 'align-self'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
            //
            // "Values: auto | flex-start | flex-end | center | baseline | stretch"
            "align-self" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "auto" => self.align_self = Some(AlignSelf::Auto),
                        "flex-start" | "start" => self.align_self = Some(AlignSelf::FlexStart),
                        "flex-end" | "end" => self.align_self = Some(AlignSelf::FlexEnd),
                        "center" => self.align_self = Some(AlignSelf::Center),
                        "baseline" => self.align_self = Some(AlignSelf::Baseline),
                        "stretch" => self.align_self = Some(AlignSelf::Stretch),
                        _ => {}
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
                    values.first()
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
                    values.first()
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
                if let Some(first) = values.first()
                    && let Some(auto_len) = parse_single_auto_length(first)
                {
                    self.flex_basis = Some(self.resolve_auto_length(auto_len));
                }
            }
            // [§ 7 'flex' shorthand](https://www.w3.org/TR/css-flexbox-1/#flex-property)
            //
            // "The flex property specifies the components of a flexible length:
            // the flex grow factor and flex shrink factor, and the flex basis."
            //
            // "Value: none | [ <'flex-grow'> <'flex-shrink'>? || <'flex-basis'> ]"
            //
            // Special values:
            //   flex: none  → flex: 0 0 auto
            //   flex: auto  → flex: 1 1 auto
            //   flex: <number> → flex: <number> 1 0 (note: basis is 0, not auto!)
            #[allow(clippy::cast_possible_truncation)]
            "flex" => {
                self.parse_flex_shorthand(values);
            }
            // [§ 5.2 'flex-wrap'](https://www.w3.org/TR/css-flexbox-1/#flex-wrap-property)
            //
            // "Values: nowrap | wrap | wrap-reverse"
            "flex-wrap" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "nowrap" => self.flex_wrap = Some(FlexWrap::Nowrap),
                        "wrap" => self.flex_wrap = Some(FlexWrap::Wrap),
                        "wrap-reverse" => self.flex_wrap = Some(FlexWrap::WrapReverse),
                        _ => {}
                    }
                }
            }
            // [§ 5.3 'flex-flow' shorthand](https://www.w3.org/TR/css-flexbox-1/#flex-flow-property)
            //
            // "Value: <'flex-direction'> || <'flex-wrap'>"
            "flex-flow" => {
                for cv in values {
                    if let ComponentValue::Token(CSSToken::Ident(ident)) = cv {
                        match ident.to_ascii_lowercase().as_str() {
                            "row" => self.flex_direction = Some(FlexDirection::Row),
                            "row-reverse" => self.flex_direction = Some(FlexDirection::RowReverse),
                            "column" => self.flex_direction = Some(FlexDirection::Column),
                            "column-reverse" => {
                                self.flex_direction = Some(FlexDirection::ColumnReverse);
                            }
                            "nowrap" => self.flex_wrap = Some(FlexWrap::Nowrap),
                            "wrap" => self.flex_wrap = Some(FlexWrap::Wrap),
                            "wrap-reverse" => self.flex_wrap = Some(FlexWrap::WrapReverse),
                            _ => {}
                        }
                    }
                }
            }
            // [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
            //
            // "Values: left | right | none | inherit"
            "float" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "left" => self.float = Some(FloatSide::Left),
                        "right" => self.float = Some(FloatSide::Right),
                        "none" => self.float = None,
                        _ => {}
                    }
                }
            }
            // [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
            //
            // "Values: left | right | both | none | inherit"
            "clear" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "left" => self.clear = Some(ClearSide::Left),
                        "right" => self.clear = Some(ClearSide::Right),
                        "both" => self.clear = Some(ClearSide::Both),
                        "none" => self.clear = None,
                        _ => {}
                    }
                }
            }
            // [§ 9.3.1 'position'](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
            //
            // "Values: static | relative | absolute | fixed"
            // [CSS Positioned Layout Module Level 3 § 3](https://www.w3.org/TR/css-position-3/#position-property)
            // adds "sticky"
            "position" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "static" => self.position = Some(PositionType::Static),
                        "relative" => self.position = Some(PositionType::Relative),
                        "absolute" => self.position = Some(PositionType::Absolute),
                        "fixed" => self.position = Some(PositionType::Fixed),
                        "sticky" => self.position = Some(PositionType::Sticky),
                        _ => {}
                    }
                }
            }
            // [§ 9.3.2 Box offsets: 'top', 'right', 'bottom', 'left'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
            //
            // "Values: <length> | <percentage> | auto | inherit"
            "top" => {
                if let Some(al) = parse_auto_length_value(values) {
                    self.top = Some(self.resolve_auto_length(al));
                }
            }
            "right" => {
                if let Some(al) = parse_auto_length_value(values) {
                    self.right = Some(self.resolve_auto_length(al));
                }
            }
            "bottom" => {
                if let Some(al) = parse_auto_length_value(values) {
                    self.bottom = Some(self.resolve_auto_length(al));
                }
            }
            "left" => {
                if let Some(al) = parse_auto_length_value(values) {
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
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "disc" => self.list_style_type = Some(ListStyleType::Disc),
                        "circle" => self.list_style_type = Some(ListStyleType::Circle),
                        "square" => self.list_style_type = Some(ListStyleType::Square),
                        "decimal" => self.list_style_type = Some(ListStyleType::Decimal),
                        "lower-alpha" => self.list_style_type = Some(ListStyleType::LowerAlpha),
                        "upper-alpha" => self.list_style_type = Some(ListStyleType::UpperAlpha),
                        "lower-roman" => self.list_style_type = Some(ListStyleType::LowerRoman),
                        "upper-roman" => self.list_style_type = Some(ListStyleType::UpperRoman),
                        "none" => self.list_style_type = Some(ListStyleType::None),
                        _ => {}
                    }
                }
            }
            // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
            //
            // "Values: visible | hidden | scroll | auto"
            "overflow" | "overflow-x" | "overflow-y" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "visible" => self.overflow = Some(Overflow::Visible),
                        "hidden" => self.overflow = Some(Overflow::Hidden),
                        "scroll" => self.overflow = Some(Overflow::Scroll),
                        "auto" => self.overflow = Some(Overflow::Auto),
                        _ => {}
                    }
                }
            }
            // [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
            //
            // "Values: content-box | border-box"
            "box-sizing" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "border-box" => self.box_sizing_border_box = Some(true),
                        "content-box" => self.box_sizing_border_box = Some(false),
                        _ => {}
                    }
                }
            }
            // [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
            //
            // "This property declares how white space inside the element is handled."
            // Values: normal | pre | nowrap | pre-wrap | pre-line
            "white-space" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "normal" => self.white_space = Some(WhiteSpace::Normal),
                        "pre" => self.white_space = Some(WhiteSpace::Pre),
                        "nowrap" => self.white_space = Some(WhiteSpace::Nowrap),
                        "pre-wrap" => self.white_space = Some(WhiteSpace::PreWrap),
                        "pre-line" => self.white_space = Some(WhiteSpace::PreLine),
                        _ => {}
                    }
                }
            }
            // [§ 11.2 'visibility'](https://www.w3.org/TR/CSS2/visufx.html#visibility)
            //
            // "Values: visible | hidden | collapse"
            "visibility" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "visible" => self.visibility = Some(Visibility::Visible),
                        "hidden" => self.visibility = Some(Visibility::Hidden),
                        "collapse" => self.visibility = Some(Visibility::Collapse),
                        _ => {}
                    }
                }
            }
            // [§ 3.2 'opacity'](https://www.w3.org/TR/css-color-4/#transparency)
            //
            // "Value: <number>"
            // "Clamped to the range [0, 1]"
            #[allow(clippy::cast_possible_truncation)]
            "opacity" => {
                if let Some(ComponentValue::Token(CSSToken::Number { value, .. })) =
                    values.first()
                {
                    self.opacity = Some((*value as f32).clamp(0.0, 1.0));
                }
            }
            // [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
            //
            // "The 'box-shadow' property attaches one or more drop-shadows to the box."
            // Values: none | <shadow>#
            "box-shadow" => {
                // "none" keyword clears shadows
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first()
                    && ident.eq_ignore_ascii_case("none")
                {
                    self.box_shadow = None;
                } else {
                    self.box_shadow = self.parse_box_shadow(values);
                }
            }

            // ===== Grid layout properties =====

            // [§ 7.2 'grid-template-columns'](https://www.w3.org/TR/css-grid-1/#track-sizing)
            //
            // "These properties specify, as a space-separated track list, the line
            // names and track sizing functions of the grid."
            "grid-template-columns" => {
                if let Some(tl) = self.parse_track_list(values) {
                    self.grid_template_columns = Some(tl);
                }
            }
            // [§ 7.2 'grid-template-rows'](https://www.w3.org/TR/css-grid-1/#track-sizing)
            "grid-template-rows" => {
                if let Some(tl) = self.parse_track_list(values) {
                    self.grid_template_rows = Some(tl);
                }
            }
            // [§ 7.6 'grid-auto-flow'](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
            //
            // "Values: row | column | row dense | column dense"
            "grid-auto-flow" => {
                if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = values.first() {
                    match ident.to_ascii_lowercase().as_str() {
                        "row" => self.grid_auto_flow = Some(GridAutoFlow::Row),
                        "column" => self.grid_auto_flow = Some(GridAutoFlow::Column),
                        _ => {}
                    }
                }
            }
            // [§ 10.1 'row-gap'](https://www.w3.org/TR/css-align-3/#row-gap)
            "row-gap" | "grid-row-gap" => {
                if let Some(len) = parse_length_value(values) {
                    self.row_gap = Some(self.resolve_length(len));
                }
            }
            // [§ 10.1 'column-gap'](https://www.w3.org/TR/css-align-3/#column-gap)
            "column-gap" | "grid-column-gap" => {
                if let Some(len) = parse_length_value(values) {
                    self.column_gap = Some(self.resolve_length(len));
                }
            }
            // [§ 10.1 'gap'](https://www.w3.org/TR/css-align-3/#gap-shorthand)
            //
            // "The gap property, and its grid-gap alias, set the row-gap and
            // column-gap properties in one declaration."
            // "Value: <'row-gap'> <'column-gap'>?"
            "gap" | "grid-gap" => {
                let lengths: Vec<LengthValue> =
                    values.iter().filter_map(parse_single_length).collect();
                match lengths.len() {
                    1 => {
                        let resolved = self.resolve_length(lengths[0]);
                        self.row_gap = Some(resolved);
                        self.column_gap = Some(resolved);
                    }
                    2 => {
                        self.row_gap = Some(self.resolve_length(lengths[0]));
                        self.column_gap = Some(self.resolve_length(lengths[1]));
                    }
                    _ => {}
                }
            }
            // [§ 8.3 'grid-column-start'](https://www.w3.org/TR/css-grid-1/#line-placement)
            "grid-column-start" => {
                if let Some(gl) = Self::parse_grid_line(values) {
                    self.grid_column_start = Some(gl);
                }
            }
            // [§ 8.3 'grid-column-end'](https://www.w3.org/TR/css-grid-1/#line-placement)
            "grid-column-end" => {
                if let Some(gl) = Self::parse_grid_line(values) {
                    self.grid_column_end = Some(gl);
                }
            }
            // [§ 8.3 'grid-row-start'](https://www.w3.org/TR/css-grid-1/#line-placement)
            "grid-row-start" => {
                if let Some(gl) = Self::parse_grid_line(values) {
                    self.grid_row_start = Some(gl);
                }
            }
            // [§ 8.3 'grid-row-end'](https://www.w3.org/TR/css-grid-1/#line-placement)
            "grid-row-end" => {
                if let Some(gl) = Self::parse_grid_line(values) {
                    self.grid_row_end = Some(gl);
                }
            }
            // [§ 8.4 'grid-column' shorthand](https://www.w3.org/TR/css-grid-1/#propdef-grid-column)
            //
            // "Value: <grid-line> [ / <grid-line> ]?"
            "grid-column" => {
                let (start, end) = Self::parse_grid_line_shorthand(values);
                if let Some(s) = start {
                    self.grid_column_start = Some(s);
                }
                self.grid_column_end = Some(end.unwrap_or(GridLine::Auto));
            }
            // [§ 8.4 'grid-row' shorthand](https://www.w3.org/TR/css-grid-1/#propdef-grid-row)
            //
            // "Value: <grid-line> [ / <grid-line> ]?"
            "grid-row" => {
                let (start, end) = Self::parse_grid_line_shorthand(values);
                if let Some(s) = start {
                    self.grid_row_start = Some(s);
                }
                self.grid_row_end = Some(end.unwrap_or(GridLine::Auto));
            }
            unknown => {
                warn_once("CSS", &format!("unknown property '{unknown}'"));
            }
        }
    }

    /// [§ 2.3 Resolving Dependency Cycles](https://www.w3.org/TR/css-variables-1/#cycles)
    ///
    /// "Custom properties resolve any var() functions in their values at
    /// computed-value time, which occurs before the value is inherited."
    ///
    /// Resolve all `var()` references within custom property values.
    /// Must be called after all declarations are applied and before
    /// children inherit.
    pub fn resolve_custom_properties(&mut self) {
        let keys: Vec<String> = self.custom_properties.keys().cloned().collect();
        for key in keys {
            let raw_value = self.custom_properties[&key].clone();
            if contains_var(&raw_value) {
                match substitute_var(&raw_value, &self.custom_properties, 0) {
                    Some(resolved) => {
                        let _ = self.custom_properties.insert(key, resolved);
                    }
                    None => {
                        // [§ 2.3] "all the custom properties in the cycle
                        // are invalid at computed-value time"
                        let _ = self.custom_properties.remove(&key);
                    }
                }
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
        let styles: Vec<String> = values.iter().filter_map(Self::parse_border_style).collect();

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

    /// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
    ///
    /// Parse a comma-separated list of `<shadow>` values.
    ///
    /// `<shadow> = inset? && <length>{2,4} && <color>?`
    #[allow(clippy::cast_possible_truncation)]
    fn parse_box_shadow(&self, values: &[ComponentValue]) -> Option<Vec<BoxShadow>> {
        // Split on commas to get individual shadow groups
        let mut groups: Vec<Vec<&ComponentValue>> = Vec::new();
        let mut current: Vec<&ComponentValue> = Vec::new();
        for v in values {
            if matches!(v, ComponentValue::Token(CSSToken::Comma)) {
                if !current.is_empty() {
                    groups.push(current);
                    current = Vec::new();
                }
            } else {
                current.push(v);
            }
        }
        if !current.is_empty() {
            groups.push(current);
        }

        let mut shadows = Vec::new();
        for group in &groups {
            if let Some(shadow) = self.parse_single_shadow(group) {
                shadows.push(shadow);
            } else {
                return None; // Invalid shadow = entire property invalid
            }
        }

        if shadows.is_empty() {
            None
        } else {
            Some(shadows)
        }
    }

    /// Parse a single `<shadow>` value.
    ///
    /// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
    ///
    /// `<shadow> = inset? && <length>{2,4} && <color>?`
    ///
    /// "The lengths are interpreted as follows:
    /// - The first length is the horizontal offset (positive = right).
    /// - The second length is the vertical offset (positive = down).
    /// - The third length is the blur radius (must be >= 0, default 0).
    /// - The fourth length is the spread distance (default 0)."
    #[allow(clippy::cast_possible_truncation)]
    fn parse_single_shadow(&self, values: &[&ComponentValue]) -> Option<BoxShadow> {
        let mut inset = false;
        let mut lengths: Vec<f32> = Vec::new();
        let mut color: Option<ColorValue> = None;

        for &v in values {
            // Skip whitespace tokens
            if matches!(v, ComponentValue::Token(CSSToken::Whitespace)) {
                continue;
            }

            // Check for "inset" keyword
            if let ComponentValue::Token(CSSToken::Ident(ident)) = v
                && ident.eq_ignore_ascii_case("inset")
            {
                inset = true;
                continue;
            }

            // Try to parse as a length
            if let Some(len) = parse_single_length(v) {
                lengths.push(self.resolve_length(len).to_px() as f32);
                continue;
            }

            // Try to parse as a color
            if color.is_none()
                && let Some(c) = parse_single_color(v)
            {
                color = Some(c);
                continue;
            }
        }

        // Need at least 2 lengths (offset-x, offset-y)
        if lengths.len() < 2 {
            return None;
        }

        let offset_x = lengths[0];
        let offset_y = lengths[1];
        let blur_radius = lengths.get(2).copied().unwrap_or(0.0).max(0.0);
        let spread_radius = lengths.get(3).copied().unwrap_or(0.0);

        // "If the color is absent, the used color is taken from the 'color' property."
        let color = color.unwrap_or_else(|| self.color.clone().unwrap_or(ColorValue::BLACK));

        Some(BoxShadow {
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            color,
            inset,
        })
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

    // ===== Grid parsing helpers =====

    /// [§ 7.2 Explicit Track Sizing](https://www.w3.org/TR/css-grid-1/#track-sizing)
    ///
    /// Parse a track list from `grid-template-columns` / `grid-template-rows`.
    ///
    /// Supported syntax:
    /// - `<length>` (px, em)
    /// - `<flex>` (fr unit)
    /// - `auto`
    /// - `repeat(<integer>, <track-list>)`
    #[allow(clippy::cast_possible_truncation)]
    fn parse_track_list(&self, values: &[ComponentValue]) -> Option<TrackList> {
        let mut sizes = Vec::new();

        for v in values {
            match v {
                // "100px", "1fr", "2em"
                ComponentValue::Token(CSSToken::Dimension { value, unit, .. }) => {
                    if unit.eq_ignore_ascii_case("fr") {
                        sizes.push(TrackSize::Fr(*value as f32));
                    } else if unit.eq_ignore_ascii_case("px") {
                        sizes.push(TrackSize::Fixed(*value as f32));
                    } else if unit.eq_ignore_ascii_case("em") {
                        // Resolve em to px
                        let base = self
                            .font_size
                            .as_ref()
                            .map_or(DEFAULT_FONT_SIZE_PX, LengthValue::to_px);
                        sizes.push(TrackSize::Fixed((*value * base) as f32));
                    }
                }
                // "auto"
                ComponentValue::Token(CSSToken::Ident(ident))
                    if ident.eq_ignore_ascii_case("auto") =>
                {
                    sizes.push(TrackSize::Auto);
                }
                // "repeat(3, 100px)"
                ComponentValue::Function { name, value } => {
                    if name.eq_ignore_ascii_case("repeat") {
                        if let Some(expanded) = self.parse_repeat_function(value) {
                            sizes.extend(expanded);
                        }
                    }
                }
                // Skip whitespace
                ComponentValue::Token(CSSToken::Whitespace) => {}
                _ => {}
            }
        }

        if sizes.is_empty() {
            None
        } else {
            Some(TrackList { sizes })
        }
    }

    /// [§ 7.5 repeat()](https://www.w3.org/TR/css-grid-1/#funcdef-repeat)
    ///
    /// "The repeat() notation represents a repeated fragment of the track
    /// list, allowing a large number of columns or rows that exhibit a
    /// recurring pattern to be written in a more compact form."
    ///
    /// MVP: `repeat(<positive-integer>, <track-size>+)`
    #[allow(clippy::cast_possible_truncation)]
    fn parse_repeat_function(&self, args: &[ComponentValue]) -> Option<Vec<TrackSize>> {
        // STEP 1: Find the integer count (first Number token).
        let mut count: Option<u32> = None;
        let mut found_comma = false;
        let mut track_sizes = Vec::new();

        for arg in args {
            match arg {
                // The repeat count
                ComponentValue::Token(CSSToken::Number { int_value: Some(n), .. })
                    if count.is_none() && *n > 0 =>
                {
                    count = Some(*n as u32);
                }
                // Comma separator between count and track sizes
                ComponentValue::Token(CSSToken::Comma) => {
                    found_comma = true;
                }
                // Track sizes after the comma
                ComponentValue::Token(CSSToken::Dimension { value, unit, .. }) if found_comma => {
                    if unit.eq_ignore_ascii_case("fr") {
                        track_sizes.push(TrackSize::Fr(*value as f32));
                    } else if unit.eq_ignore_ascii_case("px") {
                        track_sizes.push(TrackSize::Fixed(*value as f32));
                    } else if unit.eq_ignore_ascii_case("em") {
                        let base = self
                            .font_size
                            .as_ref()
                            .map_or(DEFAULT_FONT_SIZE_PX, LengthValue::to_px);
                        track_sizes.push(TrackSize::Fixed((*value * base) as f32));
                    }
                }
                ComponentValue::Token(CSSToken::Ident(ident))
                    if found_comma && ident.eq_ignore_ascii_case("auto") =>
                {
                    track_sizes.push(TrackSize::Auto);
                }
                ComponentValue::Token(CSSToken::Whitespace) => {}
                _ => {}
            }
        }

        let count = count?;
        if track_sizes.is_empty() {
            return None;
        }

        // STEP 2: Expand the repetition.
        let mut result = Vec::with_capacity(count as usize * track_sizes.len());
        for _ in 0..count {
            result.extend_from_slice(&track_sizes);
        }
        Some(result)
    }

    /// [§ 8.3 Line-based Placement](https://www.w3.org/TR/css-grid-1/#line-placement)
    ///
    /// Parse a `<grid-line>` value.
    ///
    /// Syntax: `auto | <integer> | span && <integer>`
    fn parse_grid_line(values: &[ComponentValue]) -> Option<GridLine> {
        // Filter out whitespace
        let tokens: Vec<&ComponentValue> = values
            .iter()
            .filter(|v| !matches!(v, ComponentValue::Token(CSSToken::Whitespace)))
            .collect();

        if tokens.is_empty() {
            return None;
        }

        // "auto"
        if let Some(ComponentValue::Token(CSSToken::Ident(ident))) = tokens.first() {
            let lower = ident.to_ascii_lowercase();
            if lower == "auto" {
                return Some(GridLine::Auto);
            }
            // "span <integer>"
            if lower == "span" {
                if let Some(ComponentValue::Token(CSSToken::Number { int_value: Some(n), .. })) =
                    tokens.get(1)
                {
                    if *n > 0 {
                        return Some(GridLine::Span(*n as u32));
                    }
                }
                return None;
            }
        }

        // <integer> (line number)
        if let Some(ComponentValue::Token(CSSToken::Number { int_value: Some(n), .. })) =
            tokens.first()
        {
            if *n != 0 {
                #[allow(clippy::cast_possible_truncation)]
                return Some(GridLine::Line(*n as i32));
            }
        }

        None
    }

    /// [§ 8.4 Placement Shorthands](https://www.w3.org/TR/css-grid-1/#placement-shorthands)
    ///
    /// Parse `grid-column` / `grid-row` shorthand.
    /// "Value: `<grid-line>` [ / `<grid-line>` ]?"
    ///
    /// Split on `/` delimiter, parse each half as a `GridLine`.
    fn parse_grid_line_shorthand(
        values: &[ComponentValue],
    ) -> (Option<GridLine>, Option<GridLine>) {
        // Find the `/` delimiter position
        let slash_pos = values
            .iter()
            .position(|v| matches!(v, ComponentValue::Token(CSSToken::Delim('/'))));

        match slash_pos {
            Some(pos) => {
                let start = Self::parse_grid_line(&values[..pos]);
                let end = Self::parse_grid_line(&values[pos + 1..]);
                (start, end)
            }
            None => {
                // No `/` — start only, end defaults to Auto
                let start = Self::parse_grid_line(values);
                (start, None)
            }
        }
    }

    /// [§ 7.1.1 Basic Values of flex](https://www.w3.org/TR/css-flexbox-1/#flex-common)
    ///
    /// "The flex property specifies the components of a flexible length:
    /// the flex grow factor and flex shrink factor, and the flex basis."
    ///
    /// "Value: none | [ <'flex-grow'> <'flex-shrink'>? || <'flex-basis'> ]"
    ///
    /// [§ 7.1.1](https://www.w3.org/TR/css-flexbox-1/#flex-common):
    ///
    /// - `flex: initial` — "Equivalent to flex: 0 1 auto. Sizes the item
    ///   based on the width/height properties."
    /// - `flex: auto` — "Equivalent to flex: 1 1 auto. Sizes the item
    ///   based on the width/height properties, but makes them fully
    ///   flexible."
    /// - `flex: none` — "Equivalent to flex: 0 0 auto. Sizes the item
    ///   according to the width/height properties, but makes the flex item
    ///   fully inflexible."
    /// - `flex: <positive-number>` — "Equivalent to flex: <positive-number>
    ///   1 0. Makes the flex item flexible and sets the flex basis to zero,
    ///   resulting in an item that receives the specified proportion of the
    ///   free space in the flex container."
    #[allow(clippy::cast_possible_truncation)]
    fn parse_flex_shorthand(&mut self, values: &[ComponentValue]) {
        // Filter whitespace
        let tokens: Vec<&ComponentValue> = values
            .iter()
            .filter(|v| !matches!(v, ComponentValue::Token(CSSToken::Whitespace)))
            .collect();

        if tokens.is_empty() {
            return;
        }

        // Check for keyword values
        if tokens.len() == 1 {
            if let ComponentValue::Token(CSSToken::Ident(ident)) = tokens[0] {
                match ident.to_ascii_lowercase().as_str() {
                    // [§ 7.1.1](https://www.w3.org/TR/css-flexbox-1/#flex-common)
                    //
                    // "Equivalent to flex: 0 0 auto."
                    "none" => {
                        self.flex_grow = Some(0.0);
                        self.flex_shrink = Some(0.0);
                        self.flex_basis = Some(AutoLength::Auto);
                        return;
                    }
                    // [§ 7.1.1](https://www.w3.org/TR/css-flexbox-1/#flex-common)
                    //
                    // "Equivalent to flex: 1 1 auto."
                    "auto" => {
                        self.flex_grow = Some(1.0);
                        self.flex_shrink = Some(1.0);
                        self.flex_basis = Some(AutoLength::Auto);
                        return;
                    }
                    // [§ 7.1.1](https://www.w3.org/TR/css-flexbox-1/#flex-common)
                    //
                    // "Equivalent to flex: 0 1 auto."
                    "initial" => {
                        self.flex_grow = Some(0.0);
                        self.flex_shrink = Some(1.0);
                        self.flex_basis = Some(AutoLength::Auto);
                        return;
                    }
                    _ => return,
                }
            }
        }

        // Parse numeric and length values
        // [§ 7](https://www.w3.org/TR/css-flexbox-1/#flex-property)
        //
        // "When omitted, flex-shrink defaults to 1."
        // "A unitless zero that is not already preceded by flex-grow or
        // flex-shrink must be interpreted as a flex factor."
        let mut numbers: Vec<f32> = Vec::new();
        let mut basis: Option<AutoLength> = None;

        for token in &tokens {
            match token {
                ComponentValue::Token(CSSToken::Number { value, .. }) => {
                    numbers.push(*value as f32);
                }
                ComponentValue::Token(CSSToken::Dimension { .. })
                | ComponentValue::Token(CSSToken::Percentage { .. }) => {
                    if let Some(auto_len) = parse_single_auto_length(token) {
                        basis = Some(self.resolve_auto_length(auto_len));
                    }
                }
                ComponentValue::Token(CSSToken::Ident(ident))
                    if ident.eq_ignore_ascii_case("auto") =>
                {
                    basis = Some(AutoLength::Auto);
                }
                _ => {}
            }
        }

        match numbers.len() {
            // flex: <grow>  (or flex: <grow> <basis>)
            1 => {
                self.flex_grow = Some(numbers[0]);
                self.flex_shrink = Some(1.0);
                // [§ 7](https://www.w3.org/TR/css-flexbox-1/#flex-property)
                //
                // "When flex is specified with a unitless number, the flex basis
                // is 0 (not auto as in the initial value)."
                self.flex_basis = basis.or(Some(AutoLength::Length(LengthValue::Px(0.0))));
            }
            // flex: <grow> <shrink>  (or flex: <grow> <shrink> <basis>)
            2 => {
                self.flex_grow = Some(numbers[0]);
                self.flex_shrink = Some(numbers[1]);
                self.flex_basis = basis.or(Some(AutoLength::Length(LengthValue::Px(0.0))));
            }
            // flex: <grow> <shrink> <basis-as-number-0>
            3 => {
                self.flex_grow = Some(numbers[0]);
                self.flex_shrink = Some(numbers[1]);
                // Third number is 0 (unitless zero for flex-basis)
                self.flex_basis =
                    basis.or(Some(AutoLength::Length(LengthValue::Px(f64::from(numbers[2])))));
            }
            _ => {}
        }
    }
}
