//! CSS Stacking Contexts and Painting Order.
//!
//! [§ 9.9 Layered presentation](https://www.w3.org/TR/CSS2/visuren.html#layers)
//!
//! "An element in CSS 2 may have a stack level, which describes its position
//! within a set of elements sharing the same stacking context."
//!
//! [CSS 2.1 Appendix E: Elaborate description of Stacking Contexts](https://www.w3.org/TR/CSS2/zindex.html)

use super::box_model::Rect;
use super::layout_box::LayoutBox;

/// [§ 9.9.1 Specifying the stack level: the 'z-index' property](https://www.w3.org/TR/CSS2/visuren.html#z-index)
///
/// "For a positioned box, the 'z-index' property specifies:
///
/// 1. The stack level of the box in the current stacking context.
/// 2. Whether the box establishes a stacking context."
///
/// "Values have the following meanings:
///
/// <integer>
///   This integer is the stack level of the generated box in the current
///   stacking context. The box also establishes a new stacking context.
///
/// auto
///   The stack level of the generated box in the current stacking context
///   is 0. The box does not establish a new stacking context unless it is
///   the root element."
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZIndex {
    /// "The stack level is 0. Does not establish a new stacking context."
    Auto,
    /// "This integer is the stack level. Establishes a new stacking context."
    Integer(i32),
}

impl Default for ZIndex {
    fn default() -> Self {
        ZIndex::Auto
    }
}

/// A stacking context in the CSS painting order.
///
/// [§ Appendix E Elaborate description of Stacking Contexts](https://www.w3.org/TR/CSS2/zindex.html)
///
/// "Each box belongs to one stacking context. Each positioned box in a given
/// stacking context has an integer stack level, which is its position on the
/// z-axis relative to other stack levels within the same stacking context."
pub struct StackingContext {
    /// The stack level of this context (from z-index).
    pub stack_level: i32,
    /// The bounding box of this stacking context.
    pub bounds: Rect,
    /// Child stacking contexts (sorted by stack level).
    pub children: Vec<StackingContext>,
}

impl StackingContext {
    /// [§ Appendix E](https://www.w3.org/TR/CSS2/zindex.html)
    ///
    /// Build stacking contexts from the layout tree.
    ///
    /// "Stacking contexts can be formed by several CSS properties:
    ///
    /// - Root element of the document
    /// - Positioned elements with z-index other than 'auto'
    /// - Elements with opacity less than 1
    /// - Elements with transform, filter, perspective, clip-path,
    ///   mask, or mix-blend-mode"
    ///
    /// TODO: Implement stacking context collection:
    ///
    /// STEP 1: Check if the element establishes a stacking context
    ///   // Root element always establishes a stacking context.
    ///   // Positioned elements with z-index != auto establish one.
    ///   // Elements with opacity < 1 establish one (CSS3).
    ///   // Elements with certain CSS3 properties establish one.
    ///
    /// STEP 2: Collect child stacking contexts recursively
    ///   // Walk the layout tree and collect all children that
    ///   // establish their own stacking contexts.
    ///
    /// STEP 3: Sort children by stack level
    ///   // "Boxes with the same stack level in a stacking context are
    ///   //  stacked back-to-front according to document tree order."
    pub fn collect_stacking_contexts(_layout_tree: &LayoutBox) -> StackingContext {
        todo!("Build stacking context tree from layout tree per CSS 2.1 Appendix E")
    }

    /// [§ Appendix E Painting order](https://www.w3.org/TR/CSS2/zindex.html)
    ///
    /// "Within each stacking context, the following layers are painted in
    /// back-to-front order:
    ///
    /// 1. the background and borders of the element forming the stacking context.
    /// 2. the child stacking contexts with negative stack levels (most negative first).
    /// 3. the in-flow, non-inline-level, non-positioned descendants.
    /// 4. the non-positioned floats.
    /// 5. the in-flow, inline-level, non-positioned descendants, including
    ///    inline tables and inline blocks.
    /// 6. the child stacking contexts with stack level 0 and the positioned
    ///    descendants with stack level 0.
    /// 7. the child stacking contexts with positive stack levels (least positive first)."
    ///
    /// TODO: Implement painting order:
    ///
    /// STEP 1: Paint background and borders of this stacking context
    ///   // paint_background(self.root_box);
    ///   // paint_borders(self.root_box);
    ///
    /// STEP 2: Paint child stacking contexts with negative z-index
    ///   // for child in self.children.iter().filter(|c| c.stack_level < 0) {
    ///   //     child.paint(display_list);
    ///   // }
    ///
    /// STEP 3: Paint in-flow, non-inline, non-positioned descendants
    ///   // Block-level children in normal flow
    ///
    /// STEP 4: Paint non-positioned floats
    ///   // Float boxes that don't have position != static
    ///
    /// STEP 5: Paint in-flow, inline-level, non-positioned descendants
    ///   // Inline content, inline-blocks, inline-tables
    ///
    /// STEP 6: Paint positioned descendants with z-index: auto or 0
    ///   // for child in self.children.iter().filter(|c| c.stack_level == 0) {
    ///   //     child.paint(display_list);
    ///   // }
    ///
    /// STEP 7: Paint child stacking contexts with positive z-index
    ///   // for child in self.children.iter().filter(|c| c.stack_level > 0) {
    ///   //     child.paint(display_list);
    ///   // }
    pub fn paint_stacking_context(&self) -> Vec<PaintCommand> {
        todo!("Paint stacking context in correct order per CSS 2.1 Appendix E")
    }
}

/// A paint command produced by the stacking context painter.
///
/// NOTE: This is a placeholder. The actual paint commands will integrate
/// with the existing `DisplayCommand` type in `paint/display_list.rs`.
#[derive(Debug)]
pub enum PaintCommand {
    /// Paint a layout box's background and borders.
    PaintBox(Rect),
    /// Paint text content.
    PaintText {
        /// X position of the text.
        x: f32,
        /// Y position of the text.
        y: f32,
        /// The text content to paint.
        text: String,
    },
}
