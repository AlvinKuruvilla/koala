//! CSS Positioned Layout.
//!
//! [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
//!
//! "In CSS 2, a box may be laid out according to three positioning schemes:
//!
//! 1. Normal flow. In CSS 2, normal flow includes block formatting of block-level
//!    boxes, inline formatting of inline-level boxes, and relative positioning of
//!    block-level and inline-level boxes.
//!
//! 2. Floats. In the float model, a box is first laid out according to the normal
//!    flow, then taken out of the flow and shifted to the left or right as far as
//!    possible.
//!
//! 3. Absolute positioning. In the absolute positioning model, a box is removed
//!    from the normal flow entirely and assigned a position with respect to a
//!    containing block."

use super::box_model::{BoxDimensions, Rect};

/// [§ 9.3.1 Choosing a positioning scheme: 'position' property](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
///
/// "The 'position' and 'float' properties determine which of the CSS 2
/// positioning algorithms is used to calculate the position of a box."
///
/// "Values have the following meanings:
///
/// static
///   The box is a normal box, laid out according to the normal flow. The
///   'top', 'right', 'bottom', and 'left' properties do not apply.
///
/// relative
///   The box's position is calculated according to the normal flow. Then
///   the box is offset relative to its normal position.
///
/// absolute
///   The box's position (and possibly size) is specified with the 'top',
///   'right', 'bottom', and 'left' properties. These properties specify
///   offsets with respect to the box's containing block.
///
/// fixed
///   The box's position is calculated according to the 'absolute' model,
///   but in addition, the box is fixed with respect to some reference.
///
/// sticky (CSS Positioned Layout Module Level 3)
///   The box's position is calculated according to the normal flow, then
///   offset relative to its nearest scrolling ancestor."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionType {
    /// "The box is a normal box, laid out according to the normal flow."
    #[default]
    Static,
    /// "The box's position is calculated according to the normal flow.
    /// Then the box is offset relative to its normal position."
    Relative,
    /// "The box's position (and possibly size) is specified with the
    /// 'top', 'right', 'bottom', and 'left' properties."
    Absolute,
    /// "The box's position is calculated according to the 'absolute' model,
    /// but the box is fixed with respect to some reference."
    Fixed,
    /// [CSS Positioned Layout Module Level 3 § 3.2](https://www.w3.org/TR/css-position-3/#sticky-position)
    ///
    /// "A stickily positioned box is positioned similarly to a relatively
    /// positioned box, but the offset is computed with reference to the
    /// nearest ancestor with a scrolling mechanism."
    Sticky,
}

/// [§ 9.3.2 Box offsets: 'top', 'right', 'bottom', 'left'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
///
/// "An element is said to be positioned if its 'position' property has
/// a value other than 'static'. Positioned elements generate positioned
/// boxes, laid out according to four properties: top, right, bottom, left."
///
/// "Values have the following meanings:
///
/// `<length>`
///   The offset is a fixed distance from the reference edge.
///
/// `<percentage>`
///   The offset is a percentage of the containing block's width (for
///   'left' or 'right') or height (for 'top' or 'bottom').
///
/// auto
///   For non-replaced elements, the effect of this value depends on which
///   of related properties have the value 'auto' as well."
#[derive(Debug, Clone, Default)]
pub struct BoxOffsets {
    /// "The 'top' property specifies how far the top margin edge of the
    /// box is offset below the top edge of the box's containing block."
    pub top: Option<f32>,
    /// "The 'right' property specifies how far the right margin edge of
    /// the box is offset to the left of the right edge of the box's
    /// containing block."
    pub right: Option<f32>,
    /// "The 'bottom' property specifies how far the bottom margin edge of
    /// the box is offset above the bottom edge of the box's containing block."
    pub bottom: Option<f32>,
    /// "The 'left' property specifies how far the left margin edge of the
    /// box is offset to the right of the left edge of the box's containing block."
    pub left: Option<f32>,
}

/// Layout engine for positioned elements.
///
/// [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
pub struct PositionedLayout;

impl PositionedLayout {
    /// [§ 9.4.3 Relative positioning](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
    ///
    /// "Once a box has been laid out according to the normal flow, it may be
    /// shifted relative to its normal position. This is called relative
    /// positioning."
    ///
    /// "Offsetting a box (B1) in this way has no effect on the box (B2) that
    /// follows: B2 is given a position as if B1 were not offset and B2 is
    /// not re-positioned after B1's offset is applied."
    ///
    /// "For relatively positioned elements, 'left' and 'right' move the box(es)
    /// horizontally, without changing their size."
    pub fn layout_relative(box_dims: &mut BoxDimensions, offsets: &BoxOffsets) {
        // STEP 1: Layout the box in normal flow (already done before this is called)

        // STEP 2: Apply horizontal offset
        // [§ 9.4.3](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
        //
        // "If both 'left' and 'right' are 'auto', the used values are both 0
        //  (i.e., the boxes stay in their original position)."
        //
        // "If 'left' is 'auto', its used value is minus the value of 'right'
        //  (i.e., the box is moved to the left by the value of 'right')."
        //
        // "If 'right' is 'auto', its used value is minus the value of 'left'
        //  (i.e., the box is moved to the right by the value of 'left')."
        //
        // "If neither 'left' nor 'right' is 'auto', the position is
        //  over-constrained, and one of them has to be ignored. If the
        //  'direction' property of the containing block is 'ltr', the value
        //  of 'left' wins and 'right' becomes -'left'. If 'direction' of the
        //  containing block is 'rtl', 'right' wins and 'left' is ignored."
        let offset_x = match (offsets.left, offsets.right) {
            // Both auto: no offset
            (None, None) => 0.0,
            // left specified, right auto: move right by left
            (Some(left), None) => left,
            // left auto, right specified: move left by right
            (None, Some(right)) => -right,
            // Both specified (over-constrained): left wins for LTR
            // NOTE: We assume LTR direction. When 'direction' property is
            // implemented, this should check the containing block's direction.
            (Some(left), Some(_right)) => left,
        };
        box_dims.content.x += offset_x;

        // STEP 3: Apply vertical offset
        // [§ 9.4.3](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
        //
        // "If both are 'auto', their used values are both 0."
        //
        // "If one of them is 'auto', it becomes the negative of the other."
        //
        // "If neither is 'auto', 'bottom' is ignored (i.e., the used value
        //  of 'bottom' will be minus the value of 'top')."
        let offset_y = match (offsets.top, offsets.bottom) {
            // Both auto: no offset
            (None, None) => 0.0,
            // top specified, bottom auto: move down by top
            (Some(top), None) => top,
            // top auto, bottom specified: move up by bottom
            (None, Some(bottom)) => -bottom,
            // Both specified (over-constrained): top wins
            (Some(top), Some(_bottom)) => top,
        };
        box_dims.content.y += offset_y;
    }

    /// [§ 10.3.7 Absolutely positioned, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
    ///
    /// "The constraint that determines the used values for these elements is:
    ///
    /// 'left' + 'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
    /// 'padding-right' + 'border-right-width' + 'margin-right' + 'right'
    /// = width of containing block"
    ///
    /// [§ 10.6.4 Absolutely positioned, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-height)
    ///
    /// "For absolutely positioned elements, the used values of the vertical
    /// dimensions must satisfy this constraint:
    ///
    /// 'top' + 'margin-top' + 'border-top-width' + 'padding-top' + 'height' +
    /// 'padding-bottom' + 'border-bottom-width' + 'margin-bottom' + 'bottom'
    /// = height of containing block"
    ///
    /// TODO: Implement absolute positioning:
    ///
    /// STEP 1: Find the containing block
    ///   // [§ 10.1 Definition of "containing block"](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
    ///   // "If the element has 'position: absolute', the containing block is
    ///   //  established by the nearest ancestor with a 'position' of 'absolute',
    ///   //  'relative', or 'fixed'."
    ///   // "If there is no such ancestor, the containing block is the initial
    ///   //  containing block."
    ///
    /// STEP 2: Resolve the horizontal constraint equation
    ///   // [§ 10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
    ///   //
    ///   // CASE 1: "If all three of 'left', 'width', and 'right' are 'auto':
    ///   //   First set any 'auto' values for 'margin-left' and 'margin-right' to 0.
    ///   //   Then, if the 'direction' is 'ltr' set 'left' to the static position..."
    ///   //
    ///   // CASE 2: "If none of the three is 'auto': the values are over-constrained,
    ///   //   and one must be ignored."
    ///   //
    ///   // CASE 3: "Otherwise, set auto margins to 0, and solve for the remaining
    ///   //   auto value."
    ///
    /// STEP 3: Resolve the vertical constraint equation
    ///   // [§ 10.6.4](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-height)
    ///   // Same pattern as horizontal but with top/height/bottom.
    ///
    /// STEP 4: Position the box relative to the containing block
    ///   `// box.dimensions.content.x = containing_block.x + left + margin_left + ...`
    ///   `// box.dimensions.content.y = containing_block.y + top + margin_top + ...`
    pub fn layout_absolute(
        _box_dims: &mut BoxDimensions,
        _offsets: &BoxOffsets,
        _containing_block: Rect,
    ) {
        todo!("Absolute positioning: position box relative to containing block")
    }

    /// [§ 9.3.1 Fixed positioning](https://www.w3.org/TR/CSS2/visuren.html#fixed-positioning)
    ///
    /// "Fixed positioning is a subcategory of absolute positioning. The only
    /// difference is that for a fixed positioned box, the containing block is
    /// established by the viewport."
    ///
    /// "Fixed backgrounds on the root element are fixed with respect to the
    /// canvas and do not move during scrolling."
    ///
    /// TODO: Implement fixed positioning:
    ///
    /// STEP 1: Use the viewport as the containing block
    ///   `// let containing_block = viewport;`
    ///
    /// STEP 2: Apply the same algorithm as absolute positioning
    ///   `// Self::layout_absolute(box_dims, offsets, viewport);`
    ///
    /// STEP 3: Mark the box as fixed (for scroll behavior)
    ///   // During painting, fixed boxes are positioned relative to the viewport
    ///   // regardless of scroll position.
    pub fn layout_fixed(_box_dims: &mut BoxDimensions, _offsets: &BoxOffsets, _viewport: Rect) {
        todo!("Fixed positioning: position box relative to viewport")
    }

    /// [CSS Positioned Layout Module Level 3 § 3.2 Sticky positioning](https://www.w3.org/TR/css-position-3/#sticky-position)
    ///
    /// "A stickily positioned box is positioned similarly to a relatively
    /// positioned box, but the offset is computed with reference to the
    /// nearest ancestor with a scrolling mechanism (created when overflow
    /// is hidden, scroll, auto, or overlay)."
    ///
    /// "The stickily positioned box sticks to the relevant edge of the scroll
    /// container, provided the box would otherwise be visible within the
    /// scrollport."
    ///
    /// TODO: Implement sticky positioning:
    ///
    /// STEP 1: Layout the box in normal flow (already done)
    ///
    /// STEP 2: Find the nearest scroll ancestor
    ///   // Walk up the tree to find an ancestor with overflow != visible
    ///
    /// STEP 3: Compute the sticky constraint rectangle
    ///   // The box sticks within the intersection of:
    ///   //   - Its containing block (minus margins)
    ///   //   - The scroll container's visible area (minus sticky offsets)
    ///
    /// STEP 4: Apply sticky offset based on scroll position
    ///   // If scrolled past the sticky threshold, offset the box so it
    ///   // appears to stick to the edge of the scroll container.
    ///   // The box should never leave its containing block.
    pub fn layout_sticky(
        _box_dims: &mut BoxDimensions,
        _offsets: &BoxOffsets,
        _scroll_container: Rect,
        _scroll_offset: f32,
    ) {
        todo!("Sticky positioning: offset box within scroll container")
    }
}
