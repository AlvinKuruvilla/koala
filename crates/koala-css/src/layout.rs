//! CSS Layout Engine
//!
//! This module will implement the CSS Visual Formatting Model for laying out
//! the render tree. The current SwiftUI renderer uses VStack for everything,
//! which breaks inline element flow.
//!
//! # Relevant Specifications
//!
//! - [CSS Display Module Level 3](https://www.w3.org/TR/css-display-3/)
//! - [CSS Box Model Module Level 3](https://www.w3.org/TR/css-box-3/)
//! - [CSS 2.1 Visual Formatting Model](https://www.w3.org/TR/CSS2/visuren.html)
//!
//! # Current Problems
//!
//! 1. All elements rendered as vertical stacks (VStack)
//! 2. Inline elements like <a>, <span>, <b>, <nobr> don't flow horizontally
//! 3. No display property handling (block, inline, none, etc.)
//! 4. Tables rendered without table layout algorithm

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
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
}

// [HTML Living Standard § 15 Rendering](https://html.spec.whatwg.org/multipage/rendering.html)
// defines the default CSS styles for HTML elements.

/// [§ 15.3.1 Hidden elements](https://html.spec.whatwg.org/multipage/rendering.html#hidden-elements)
/// [§ 15.3.2 The page](https://html.spec.whatwg.org/multipage/rendering.html#the-page)
/// [§ 15.3.3 Flow content](https://html.spec.whatwg.org/multipage/rendering.html#flow-content-3)
///
/// Returns the default display value for an HTML element.
pub fn default_display_for_element(tag_name: &str) -> Option<DisplayValue> {
    // [§ 15.3.1 Hidden elements]
    // "The following elements must have their display set to none:"
    // area, base, basefont, datalist, head, link, meta, noembed,
    // noframes, param, rp, script, style, template, title
    let hidden = [
        "area", "base", "basefont", "datalist", "head", "link", "meta", "noembed", "noframes",
        "param", "rp", "script", "style", "template", "title",
    ];
    if hidden.contains(&tag_name) {
        return None; // display: none
    }

    // [§ 15.3.3 Flow content]
    // Block-level elements by default
    let block_elements = [
        "address",
        "article",
        "aside",
        "blockquote",
        "body",
        "center",
        "dd",
        "details",
        "dialog",
        "dir",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hgroup",
        "hr",
        "html",
        "legend",
        "li",
        "listing",
        "main",
        "menu",
        "nav",
        "ol",
        "p",
        "plaintext",
        "pre",
        "search",
        "section",
        "summary",
        "ul",
        "xmp",
    ];
    if block_elements.contains(&tag_name) {
        return Some(DisplayValue::block());
    }

    // Inline elements (default)
    // a, abbr, acronym, b, bdi, bdo, big, br, cite, code, data, del, dfn,
    // em, font, i, img, ins, kbd, label, mark, nobr, q, ruby, s, samp,
    // small, span, strike, strong, sub, sup, time, tt, u, var, wbr
    Some(DisplayValue::inline())
}

// [CSS Box Model Module Level 3](https://www.w3.org/TR/css-box-3/)

/// [§ 3. The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
///
/// "Each box has a content area and optional surrounding padding, border,
/// and margin areas."
#[derive(Debug, Clone, Default)]
pub struct BoxDimensions {
    /// Content area dimensions
    pub content: Rect,
    /// Padding edge (content + padding)
    pub padding: EdgeSizes,
    /// Border edge (content + padding + border)
    pub border: EdgeSizes,
    /// Margin edge (content + padding + border + margin)
    pub margin: EdgeSizes,
}

/// A rectangle positioned in 2D space.
///
/// [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
#[derive(Debug, Clone, Default)]
pub struct Rect {
    /// Horizontal position of the top-left corner.
    pub x: f32,
    /// Vertical position of the top-left corner.
    pub y: f32,
    /// Width of the rectangle.
    pub width: f32,
    /// Height of the rectangle.
    pub height: f32,
}

/// Edge sizes for padding, border, or margin.
///
/// [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
#[derive(Debug, Clone, Default)]
pub struct EdgeSizes {
    /// Top edge size.
    pub top: f32,
    /// Right edge size.
    pub right: f32,
    /// Bottom edge size.
    pub bottom: f32,
    /// Left edge size.
    pub left: f32,
}

impl BoxDimensions {
    // [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
    //
    // "Each box has a content area and optional surrounding padding, border,
    // and margin areas... These areas are determined by their respective edges."
    //
    // ┌─────────────────────────────────────────┐
    // │              margin-top                 │
    // │   ┌─────────────────────────────────┐   │
    // │   │          border-top             │   │
    // │   │   ┌─────────────────────────┐   │   │
    // │   │   │      padding-top        │   │   │
    // │   │   │   ┌─────────────────┐   │   │   │
    // │ m │ b │ p │                 │ p │ b │ m │
    // │ a │ o │ a │     CONTENT     │ a │ o │ a │
    // │ r │ r │ d │                 │ d │ r │ r │
    // │ g │ d │ d │                 │ d │ d │ g │
    // │ i │ e │ i │                 │ i │ e │ i │
    // │ n │ r │ n │                 │ n │ r │ n │
    // │   │   │ g │                 │ g │   │   │
    // │   │   │   └─────────────────┘   │   │   │
    // │   │   │      padding-bottom     │   │   │
    // │   │   └─────────────────────────┘   │   │
    // │   │          border-bottom          │   │
    // │   └─────────────────────────────────┘   │
    // │              margin-bottom              │
    // └─────────────────────────────────────────┘
    //
    // The boxes from innermost to outermost:
    //   1. Content box  - the actual content (text, images, etc.)
    //   2. Padding box  - content + padding
    //   3. Border box   - content + padding + border
    //   4. Margin box   - content + padding + border + margin (outermost)

    /// [§ 3.1 Margins](https://www.w3.org/TR/css-box-3/#margins)
    ///
    /// "The margin box is the outermost box, and contains all four areas."
    ///
    /// # Formulas
    ///
    /// To find the margin box from the content box, we expand outward through
    /// all three layers (padding, border, margin):
    ///
    /// ```text
    /// x = content.x - padding.left - border.left - margin.left
    /// y = content.y - padding.top - border.top - margin.top
    ///
    /// width = content.width
    ///       + padding.left + padding.right
    ///       + border.left + border.right
    ///       + margin.left + margin.right
    ///
    /// height = content.height
    ///        + padding.top + padding.bottom
    ///        + border.top + border.bottom
    ///        + margin.top + margin.bottom
    /// ```
    pub fn margin_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left - self.border.left - self.margin.left,
            y: self.content.y - self.padding.top - self.border.top - self.margin.top,
            width: self.content.width
                + self.padding.left
                + self.padding.right
                + self.border.left
                + self.border.right
                + self.margin.left
                + self.margin.right,
            height: self.content.height
                + self.padding.top
                + self.padding.bottom
                + self.border.top
                + self.border.bottom
                + self.margin.top
                + self.margin.bottom,
        }
    }

    /// [§ 3.2 Padding](https://www.w3.org/TR/css-box-3/#paddings)
    ///
    /// "The padding box contains both the content and padding areas."
    ///
    /// # Formulas
    ///
    /// To find the padding box from the content box, we expand outward through
    /// only the padding layer:
    ///
    /// ```text
    /// x = content.x - padding.left
    /// y = content.y - padding.top
    ///
    /// width = content.width + padding.left + padding.right
    /// height = content.height + padding.top + padding.bottom
    /// ```
    pub fn padding_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left,
            y: self.content.y - self.padding.top,
            width: self.content.width + self.padding.left + self.padding.right,
            height: self.content.height + self.padding.top + self.padding.bottom,
        }
    }

    /// [§ 3.3 Borders](https://www.w3.org/TR/css-box-3/#borders)
    ///
    /// "The border box contains content, padding, and border areas."
    ///
    /// # Formulas
    ///
    /// To find the border box from the content box, we expand outward through
    /// two layers (padding, border):
    ///
    /// ```text
    /// x = content.x - padding.left - border.left
    /// y = content.y - padding.top - border.top
    ///
    /// width = content.width
    ///       + padding.left + padding.right
    ///       + border.left + border.right
    ///
    /// height = content.height
    ///        + padding.top + padding.bottom
    ///        + border.top + border.bottom
    /// ```
    pub fn border_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left - self.border.left,
            y: self.content.y - self.padding.top - self.border.top,
            width: self.content.width
                + self.padding.left
                + self.padding.right
                + self.border.left
                + self.border.right,
            height: self.content.height
                + self.padding.top
                + self.padding.bottom
                + self.border.top
                + self.border.bottom,
        }
    }
}

// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)

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
    pub fn layout_block_box(&mut self, _box_dims: &mut BoxDimensions) {
        todo!("Layout a block-level box in block formatting context")
    }
}

// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)

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
    pub fn layout_inline_box(&mut self, _box_dims: &mut BoxDimensions) {
        todo!("Layout an inline-level box in inline formatting context")
    }

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    /// "When the total width of the inline-level boxes on a line is less
    /// than the width of the line box containing them, their horizontal
    /// distribution within the line box is determined by the 'text-align'
    /// property."
    pub fn wrap_line(&mut self) {
        todo!("Wrap to next line when inline content exceeds max_width")
    }
}

/// A node in the layout tree (render tree with computed layout).
///
/// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
#[derive(Debug)]
pub struct LayoutBox {
    /// The computed dimensions of this box.
    pub dimensions: BoxDimensions,
    /// The display type of this box.
    pub display: DisplayValue,
    /// Child boxes in the layout tree.
    pub children: Vec<LayoutBox>,
}

impl LayoutBox {
    /// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
    ///
    /// "The display property, determines the type of box or boxes that
    /// are generated for an element."
    pub fn build_layout_tree(_dom_node: &koala_dom::Node) -> Option<LayoutBox> {
        todo!("Build layout tree from DOM tree with computed styles")
    }

    /// Perform layout on this box and its descendants
    pub fn layout(&mut self, _containing_width: f32) {
        todo!("Recursively layout this box based on display type")
    }
}

// [§ 17 Tables](https://www.w3.org/TR/CSS2/tables.html)

/// [§ 17.5 Visual layout of table contents](https://www.w3.org/TR/CSS2/tables.html#table-layout)
///
/// "The table-layout property controls the algorithm used to lay out the
/// table cells, rows, and columns."
pub struct TableLayout {
    /// Column widths
    pub column_widths: Vec<f32>,
    /// Row heights
    pub row_heights: Vec<f32>,
}

impl TableLayout {
    /// [§ 17.5.2 Automatic table layout](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
    ///
    /// "In this algorithm, the table's width is given by the width of its
    /// columns (and intervening borders)."
    pub fn compute_automatic_layout(&mut self) {
        todo!("Implement automatic table layout algorithm")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_display_block() {
        assert_eq!(
            default_display_for_element("div"),
            Some(DisplayValue::block())
        );
        assert_eq!(
            default_display_for_element("p"),
            Some(DisplayValue::block())
        );
    }

    #[test]
    fn test_default_display_inline() {
        assert_eq!(
            default_display_for_element("span"),
            Some(DisplayValue::inline())
        );
        assert_eq!(
            default_display_for_element("a"),
            Some(DisplayValue::inline())
        );
    }

    #[test]
    fn test_default_display_none() {
        assert_eq!(default_display_for_element("script"), None);
        assert_eq!(default_display_for_element("style"), None);
        assert_eq!(default_display_for_element("head"), None);
    }
}
