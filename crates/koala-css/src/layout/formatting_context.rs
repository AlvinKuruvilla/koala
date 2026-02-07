//! CSS Formatting Contexts.
//!
//! [§ 9.4 Normal flow](https://www.w3.org/TR/CSS2/visuren.html#normal-flow)
//!
//! "Boxes in the normal flow belong to a formatting context, which may be
//! block or inline, but not both simultaneously. Block-level boxes participate
//! in a block formatting context. Inline-level boxes participate in an
//! inline formatting context."

use super::box_model::BoxDimensions;
use super::float::FloatContext;

/// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
///
/// "Floats, absolutely positioned elements, block containers (such as
/// inline-blocks, table-cells, and table-captions) that are not block boxes,
/// and block boxes with 'overflow' other than 'visible' (except when that
/// value has been propagated to the viewport) establish new block formatting
/// contexts for their contents."
///
/// "In a block formatting context, boxes are laid out one after the other,
/// vertically, beginning at the top of a containing block. The vertical
/// distance between two sibling boxes is determined by the 'margin'
/// properties. Vertical margins between adjacent block-level boxes in a
/// block formatting context collapse."
///
/// "In a block formatting context, each box's left outer edge touches the
/// left edge of the containing block (for right-to-left formatting, right
/// edges touch). This is true even in the presence of floats (although a
/// box's line boxes may shrink due to the floats), unless the box
/// establishes a new block formatting context (in which case the box
/// itself may become narrower due to the floats)."
pub struct BlockFormattingContext {
    /// Current Y position for laying out the next block.
    pub current_y: f32,
    /// Width of the containing block.
    pub containing_width: f32,
    /// Float context for tracking floats within this BFC.
    ///
    /// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
    ///
    /// "A float is a box that is shifted to the left or right on the current
    /// line... Floats that are not positioned are placed in the float context
    /// of their nearest block formatting context."
    pub float_context: FloatContext,
}

impl BlockFormattingContext {
    /// Create a new block formatting context.
    #[must_use]
    pub const fn new(containing_width: f32, start_y: f32) -> Self {
        Self {
            current_y: start_y,
            containing_width,
            float_context: FloatContext::new(containing_width),
        }
    }

    /// [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// "In a block formatting context, boxes are laid out one after the other,
    /// vertically, beginning at the top of a containing block."
    ///
    /// "The vertical distance between two sibling boxes is determined by
    /// the 'margin' properties. Vertical margins between adjacent block-level
    /// boxes in a block formatting context collapse."
    ///
    /// TODO: Implement block box layout in BFC:
    ///
    /// STEP 1: Calculate the used width of the block
    ///   // [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///   // "The following constraints must hold among the used values:"
    ///   // 'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
    ///   // 'padding-right' + 'border-right-width' + 'margin-right' = width of containing block
    ///
    /// STEP 2: Determine horizontal position
    ///   // "Each box's left outer edge touches the left edge of the
    ///   //  containing block (for right-to-left formatting, right edges touch)."
    ///   // `box.x = containing_block.x + margin_left + border_left + padding_left`
    ///
    /// STEP 3: Determine vertical position (with margin collapsing)
    ///   // [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///   // `box.y = self.current_y + collapsed_margin_top`
    ///
    /// STEP 4: Layout child boxes recursively
    ///   // For block children: use this BFC (or create new if needed)
    ///   // For inline children: create an `InlineFormattingContext`
    ///
    /// STEP 5: Calculate the used height
    ///   // [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
    ///   // If 'height' is 'auto': sum of children's margin box heights
    ///   // If 'height' is a length: use that value
    ///
    /// STEP 6: Advance `current_y` for the next sibling
    ///   // `self.current_y += box.margin_box().height`
    pub fn layout_block_box(&mut self, _box_dims: &mut BoxDimensions) {
        todo!("Layout a block-level box in block formatting context")
    }

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// "In CSS, the adjoining margins of two or more boxes (which might or
    /// might not be siblings) can combine to form a single margin. Margins
    /// that combine this way are said to collapse, and the resulting combined
    /// margin is called a collapsed margin."
    ///
    /// "Adjoining vertical margins collapse, except:
    ///
    /// - Margins of the root element's box do not collapse.
    /// - If the top and bottom margins of an element with clearance are
    ///   adjoining, its margins collapse with the adjoining margins of
    ///   following siblings but that resulting margin does not collapse
    ///   with the bottom margin of the parent block."
    ///
    /// "Two margins are adjoining if and only if:
    ///
    /// - both belong to in-flow block-level boxes that participate in the
    ///   same block formatting context
    /// - no line boxes, no clearance, no padding and no border separate them
    /// - both belong to vertically-adjacent box edges, i.e. form one of the
    ///   following pairs:
    ///     - top margin of a box and top margin of its first in-flow child
    ///     - bottom margin of box and top margin of its next in-flow sibling
    ///     - bottom margin of a last in-flow child and bottom margin of its
    ///       parent if the parent has 'auto' computed height
    ///     - top and bottom margins of a box that does not establish a new
    ///       block formatting context and that has zero computed 'min-height',
    ///       zero or 'auto' computed 'height', and no in-flow children"
    ///
    /// TODO: Implement margin collapsing:
    ///
    /// STEP 1: Determine if margins are adjoining
    ///   // Check all conditions above
    ///
    /// STEP 2: Calculate the collapsed margin
    ///   // "When two or more margins collapse, the resulting margin width
    ///   //  is the maximum of the collapsing margins' widths."
    ///   // `collapsed = max(margin_a, margin_b)`
    ///   //
    ///   // "If there are no positive margins, the maximum of the absolute
    ///   //  values of the adjoining margins is deducted from zero."
    ///   // For negative margins:
    ///   //   If all negative: collapsed = min(margins)  (most negative)
    ///   //   If mixed: collapsed = max(positive) + min(negative)
    ///
    /// STEP 3: Handle parent-first-child collapsing
    ///   // "The top margin of an in-flow block element collapses with its
    ///   //  first in-flow block-level child's top margin if the element has
    ///   //  no top border, no top padding, and the child has no clearance."
    ///
    /// STEP 4: Handle empty block collapsing
    ///   // "An element that has had clearance applied to it never collapses
    ///   //  its top margin with its parent block's bottom margin."
    #[must_use]
    pub fn collapse_vertical_margins(
        &self,
        _margin_bottom_above: f32,
        _margin_top_below: f32,
    ) -> f32 {
        todo!("Collapse vertical margins per CSS 2.1 § 8.3.1")
    }

    /// [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// Determine whether an element establishes a new block formatting context.
    ///
    /// "Floats, absolutely positioned elements, block containers (such as
    /// inline-blocks, table-cells, and table-captions) that are not block
    /// boxes, and block boxes with 'overflow' other than 'visible' establish
    /// new block formatting contexts for their contents."
    ///
    /// [CSS Display Module Level 3 § 4.2](https://www.w3.org/TR/css-display-3/#establish-an-independent-fc)
    ///
    /// "A block container that establishes a new block formatting context
    /// is said to establish an independent formatting context."
    ///
    /// Additional CSS3 triggers:
    /// - display: flow-root
    /// - display: flex/grid (establish flex/grid formatting contexts)
    /// - contain: layout/content/paint
    /// - column-count or column-width != auto
    /// - overflow != visible (and != clip)
    ///
    /// TODO: Check all conditions that establish a new BFC.
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn establishes_bfc(
        _is_float: bool,
        _is_absolutely_positioned: bool,
        _is_inline_block: bool,
        _overflow_not_visible: bool,
    ) -> bool {
        todo!("Determine if element establishes a new BFC per CSS 2.1 § 9.4.1")
    }
}

/// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
///
/// "In an inline formatting context, boxes are laid out horizontally, one
/// after the other, beginning at the top of a containing block. Horizontal
/// margins, borders, and padding are respected between these boxes."
///
/// "The boxes may be aligned vertically in different ways: their bottoms
/// or tops may be aligned, or the baselines of text within them may be
/// aligned."
///
/// "The rectangular area that contains the boxes that form a line is called
/// a line box."
pub struct InlineFormattingContext {
    /// Current X position on the current line.
    pub current_x: f32,
    /// Current line's Y position.
    pub current_y: f32,
    /// Maximum width before wrapping.
    pub max_width: f32,
    /// Height of the current line.
    pub line_height: f32,
}

impl InlineFormattingContext {
    /// Create a new inline formatting context.
    #[must_use]
    pub const fn new(max_width: f32, start_y: f32, line_height: f32) -> Self {
        Self {
            current_x: 0.0,
            current_y: start_y,
            max_width,
            line_height,
        }
    }

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// "Horizontal margins, borders, and padding are respected between
    /// inline boxes."
    ///
    /// TODO: Implement inline box layout:
    ///
    /// STEP 1: Calculate the inline box's content width
    ///   // For text: measure text width using font metrics
    ///   // For replaced elements (img): use intrinsic width
    ///
    /// STEP 2: Add horizontal margin, border, padding
    ///   // `inline_box_width = margin_left + border_left + padding_left`
    ///   //                  `+ content_width`
    ///   //                  `+ padding_right + border_right + margin_right`
    ///
    /// STEP 3: Check if box fits on current line
    ///   // `if current_x + inline_box_width > max_width {`
    ///   //     `self.wrap_line();`
    ///   // }
    ///
    /// STEP 4: Position box horizontally
    ///   // `box.x = self.current_x;`
    ///   // `self.current_x += inline_box_width;`
    ///
    /// STEP 5: Calculate vertical alignment within line box
    ///   // Based on vertical-align property (baseline, top, middle, etc.)
    pub fn layout_inline_box(&mut self, _box_dims: &mut BoxDimensions) {
        todo!("Layout an inline-level box in inline formatting context")
    }

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// "When the total width of the inline-level boxes on a line is less
    /// than the width of the line box containing them, their horizontal
    /// distribution within the line box is determined by the 'text-align'
    /// property."
    ///
    /// TODO: Implement line wrapping:
    ///
    /// STEP 1: Finalize current line box
    ///   // Calculate line box height from tallest inline box
    ///   // Apply text-align for horizontal distribution
    ///   // `self.line_boxes.push(current_line);`
    ///
    /// STEP 2: Start new line
    ///   // `self.current_x = 0;`
    ///   // `self.current_y += line_box_height;`
    ///   // `self.current_line = Vec::new();`
    ///
    /// STEP 3: Handle word breaking
    ///   // [§ 5.5.1 Line Breaking](https://www.w3.org/TR/css-text-3/#line-breaking)
    ///   // Check for soft wrap opportunities (whitespace, hyphens)
    ///   // Handle overflow-wrap: break-word
    pub fn wrap_line(&mut self) {
        todo!("Wrap to next line when inline content exceeds max_width")
    }
}
