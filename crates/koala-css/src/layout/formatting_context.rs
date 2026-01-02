//! CSS Formatting Contexts.
//!
//! [§ 9.4 Normal flow](https://www.w3.org/TR/CSS2/visuren.html#normal-flow)

use super::box_model::BoxDimensions;

/// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
///
/// "In a block formatting context, boxes are laid out one after the other,
/// vertically, beginning at the top of a containing block."
pub struct BlockFormattingContext {
    /// Current Y position for laying out the next block
    pub current_y: f32,
    /// Width of the containing block
    pub containing_width: f32,
}

impl BlockFormattingContext {
    /// [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    /// "The vertical distance between two sibling boxes is determined by
    /// the 'margin' properties. Vertical margins between adjacent block-level
    /// boxes in a block formatting context collapse."
    ///
    /// TODO: Implement block box layout algorithm:
    ///
    /// STEP 1: Calculate the used width of the block
    ///   [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///   "The following constraints must hold among the used values of the other properties:"
    ///   'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
    ///   'padding-right' + 'border-right-width' + 'margin-right' = width of containing block
    ///
    ///   // If width is not 'auto' and total > containing_width, treat auto margins as 0
    ///   // If exactly one value is 'auto', solve for that value
    ///   // If width is 'auto', any other 'auto' values become 0, width fills remaining
    ///   // If no 'auto' values, margin-right becomes 'auto' (overconstrained)
    ///
    /// STEP 2: Determine the horizontal position
    ///   // box.x = containing_block.x + margin_left + border_left + padding_left
    ///
    /// STEP 3: Determine the vertical position
    ///   // box.y = self.current_y + margin_top (after collapsing)
    ///   // See margin collapsing rules in § 8.3.1
    ///
    /// STEP 4: Layout child boxes recursively
    ///   // For block children: create new BFC or use current
    ///   // For inline children: create IFC, layout inline content
    ///   // Track the height consumed by children
    ///
    /// STEP 5: Calculate the used height
    ///   [§ 10.6.3 Block-level non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    ///   // If 'height' is 'auto': height = distance from top content edge to bottom of last child
    ///   // If 'height' is a length: use that value
    ///   // If 'height' is a percentage and containing block height is definite: compute percentage
    ///
    /// STEP 6: Update BFC state for next sibling
    ///   // self.current_y += box.margin_box().height
    ///   // Handle margin collapsing with next sibling
    pub fn layout_block_box(&mut self, _box_dims: &mut BoxDimensions) {
        todo!("Layout a block-level box in block formatting context")
    }
}

/// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
///
/// "In an inline formatting context, boxes are laid out horizontally, one
/// after the other, beginning at the top of a containing block."
pub struct InlineFormattingContext {
    /// Current X position on the current line
    pub current_x: f32,
    /// Current line's Y position
    pub current_y: f32,
    /// Maximum width before wrapping
    pub max_width: f32,
    /// Height of the current line
    pub line_height: f32,
}

impl InlineFormattingContext {
    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    /// "Horizontal margins, borders, and padding are respected between
    /// inline boxes."
    ///
    /// TODO: Implement inline box layout:
    ///
    /// STEP 1: Calculate the inline box's content width
    ///   - For text: measure text width using font metrics
    ///   - For replaced elements (img): use intrinsic width
    ///
    /// STEP 2: Add horizontal margin, border, padding
    ///   // inline_box_width = margin_left + border_left + padding_left
    ///   //                  + content_width
    ///   //                  + padding_right + border_right + margin_right
    ///
    /// STEP 3: Check if box fits on current line
    ///   // if current_x + inline_box_width > line_box_width {
    ///   //     self.wrap_line();
    ///   // }
    ///
    /// STEP 4: Position box horizontally
    ///   // box.x = self.current_x;
    ///   // self.current_x += inline_box_width;
    ///
    /// STEP 5: Calculate vertical alignment within line box
    ///   // Based on vertical-align property (baseline, top, middle, etc.)
    pub fn layout_inline_box(&mut self, _box_dims: &mut BoxDimensions) {
        todo!("Layout an inline-level box in inline formatting context")
    }

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
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
    ///   // self.line_boxes.push(current_line);
    ///
    /// STEP 2: Start new line
    ///   // self.current_x = 0;
    ///   // self.current_y += line_box_height;
    ///   // self.current_line = Vec::new();
    ///
    /// STEP 3: Handle word breaking
    ///   // [§ 5.5.1 Line Breaking](https://www.w3.org/TR/css-text-3/#line-breaking)
    ///   // Check for soft wrap opportunities (whitespace, hyphens)
    ///   // Handle overflow-wrap: break-word
    pub fn wrap_line(&mut self) {
        todo!("Wrap to next line when inline content exceeds max_width")
    }
}
