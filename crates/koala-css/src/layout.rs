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

use std::collections::HashMap;

use koala_dom::{DomTree, NodeId, NodeType};

use crate::style::{ComputedStyle, DisplayValue, OuterDisplayType};

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
#[derive(Debug, Clone, Copy, Default)]
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
#[derive(Debug, Clone, Copy, Default)]
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
/// [§ 4.4 Automatic values](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
///
/// "Some properties can take the keyword 'auto' as a value. This keyword
/// allows the user agent to compute the value based on other properties."
///
/// This enum represents a value that can either be 'auto' or a specific length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutoOr {
    /// The value is 'auto' and must be resolved during layout.
    Auto,
    /// The value is a specific length in pixels.
    Length(f32),
}

impl Default for AutoOr {
    fn default() -> Self {
        AutoOr::Auto
    }
}
impl AutoOr {
    /// Check if the value is 'auto'.
    pub fn is_auto(&self) -> bool {
        matches!(self, AutoOr::Auto)
    }

    /// Get the length value, or a default if 'auto'.
    pub fn to_px_or(&self, default: f32) -> f32 {
        match self {
            AutoOr::Length(v) => *v,
            AutoOr::Auto => default,
        }
    }
}

/// [§ 8 Box model](https://www.w3.org/TR/CSS2/box.html)
///
/// Edge values where each side can be 'auto' or a specific length.
/// Used for margins where 'auto' has special meaning (centering).
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoEdgeSizes {
    /// Top edge value.
    pub top: AutoOr,
    /// Right edge value.
    pub right: AutoOr,
    /// Bottom edge value.
    pub bottom: AutoOr,
    /// Left edge value.
    pub left: AutoOr,
}

/// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
///
/// "The following sections describe the types of boxes that may be generated
/// in CSS 2.1. A box's type affects, in part, its behavior in the visual
/// formatting model."
#[derive(Debug)]
pub enum BoxType {
    /// [§ 9.2 Principal box](https://www.w3.org/TR/css-display-3/#principal-box)
    ///
    /// "Most elements generate a single principal box."
    /// Contains the NodeId to reference back to the DOM element.
    Principal(NodeId),

    /// [§ 9.2.1.1 Anonymous inline boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-inline)
    ///
    /// "Any text that is directly contained inside a block container element
    /// (not inside an inline element) must be treated as an anonymous inline
    /// element."
    ///
    /// [§ 2.5 Text Runs](https://www.w3.org/TR/css-display-3/#text-nodes)
    ///
    /// "A text run is the most basic box generated."
    AnonymousInline(String),

    /// [§ 9.2.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
    ///
    /// "In a document like this: <div>Some text<p>More text</p></div>
    /// ...the 'Some text' part generates an anonymous block box."
    AnonymousBlock,
}
/// A node in the layout tree (render tree with computed layout).
///
/// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
///
/// "Each box is associated with its generating element."
///
/// The layout box stores both the computed style values (from the cascade)
/// and the used values (resolved during layout).
#[derive(Debug)]
pub struct LayoutBox {
    /// The type of box (principal, anonymous inline, anonymous block)
    pub box_type: BoxType,

    /// The computed dimensions of this box (used values after layout).
    pub dimensions: BoxDimensions,

    /// The display type of this box.
    pub display: DisplayValue,

    /// Child boxes in the layout tree.
    pub children: Vec<LayoutBox>,

    // ===== Style values from the cascade =====
    // These are the "specified" or "computed" values that may include 'auto'.
    // During layout, these are resolved to concrete "used" values.
    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    ///
    /// "Margins can be negative, but there may be implementation-specific limits."
    /// "The value 'auto' is discussed in the section on calculating widths and margins."
    ///
    /// Margins can be 'auto' (for centering) or a specific length.
    pub margin: AutoEdgeSizes,

    /// [§ 8.4 Padding properties](https://www.w3.org/TR/CSS2/box.html#padding-properties)
    ///
    /// "Unlike margin properties, values for padding values cannot be negative."
    /// "The padding properties do not allow 'auto' as a value."
    ///
    /// Padding is always a specific length (no auto).
    pub padding: EdgeSizes,

    /// [§ 8.5 Border properties](https://www.w3.org/TR/CSS2/box.html#border-properties)
    ///
    /// "The border properties specify the width, color, and style of the border."
    ///
    /// Border widths are always specific lengths (no auto).
    pub border_width: EdgeSizes,

    /// [§ 10.2 Content width: the 'width' property](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
    ///
    /// "This property specifies the content width of boxes."
    /// "The value 'auto' means that the width depends on the values of other properties."
    ///
    /// Width can be 'auto' or a specific length.
    pub width: AutoOr,

    /// [§ 10.5 Content height: the 'height' property](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    ///
    /// "This property specifies the content height of boxes."
    /// "The value 'auto' means that the height depends on the values of other properties."
    ///
    /// Height can be 'auto' or a specific length.
    pub height: AutoOr,
}

impl LayoutBox {
    /// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
    ///
    /// "The display property, determines the type of box or boxes that
    /// are generated for an element."
    pub fn build_layout_tree(
        tree: &DomTree,
        styles: &HashMap<NodeId, ComputedStyle>,
        node_id: NodeId,
    ) -> Option<LayoutBox> {
        let Some(node) = tree.get(node_id) else {
            return None;
        };

        match &node.node_type {
            // [§ 9.1.1 The viewport](https://www.w3.org/TR/CSS2/visuren.html#viewport)
            //
            // "User agents for continuous media generally offer users a viewport
            // (a window or other viewing area on the screen) through which users
            // consult a document."
            //
            // The Document node serves as the initial containing block and
            // establishes the root of the layout tree.
            NodeType::Document => {
                let mut children = Vec::new();
                for &child_id in tree.children(node_id) {
                    if let Some(child_box) = LayoutBox::build_layout_tree(tree, styles, child_id) {
                        children.push(child_box);
                    }
                }
                Some(LayoutBox {
                    box_type: BoxType::Principal(node_id),
                    dimensions: BoxDimensions::default(),
                    display: DisplayValue::block(),
                    children,
                    // Document has no margin/padding/border
                    margin: AutoEdgeSizes::default(),
                    padding: EdgeSizes::default(),
                    border_width: EdgeSizes::default(),
                    width: AutoOr::Auto,
                    height: AutoOr::Auto,
                })
            }
            // [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
            //
            // "An element's display type determines the type of principal box
            // it generates."
            NodeType::Element(data) => {
                let tag = data.tag_name.to_lowercase();
                let style = styles.get(&node_id);

                // [§ 2.6 display: none](https://www.w3.org/TR/css-display-3/#valdef-display-none)
                //
                // "The element and its descendants generate no boxes or text runs."
                //
                // Check if CSS explicitly sets display: none
                if let Some(s) = style {
                    if s.display_none {
                        return None;
                    }
                }

                // [§ 2 The display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
                //
                // "The display property defines an element's display type..."
                //
                // Priority:
                // 1. CSS-specified display value (from computed style)
                // 2. User-agent default for the element
                let display = style
                    .and_then(|s| s.display)
                    .or_else(|| default_display_for_element(&tag))?;

                // Build children recursively
                let mut children = Vec::new();
                for &child_id in tree.children(node_id) {
                    if let Some(child_box) = LayoutBox::build_layout_tree(tree, styles, child_id) {
                        children.push(child_box);
                    }
                }

                // Extract style values from computed style
                // [§ 8 Box model](https://www.w3.org/TR/CSS2/box.html)
                let (margin, padding, border_width, width, height) =
                    Self::extract_box_style_values(style);

                Some(LayoutBox {
                    box_type: BoxType::Principal(node_id),
                    dimensions: BoxDimensions::default(),
                    display,
                    children,
                    margin,
                    padding,
                    border_width,
                    width,
                    height,
                })
            }
            // [§ 9.2.1.1 Anonymous inline boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-inline)
            //
            // "Any text that is directly contained inside a block container element
            // (not inside an inline element) must be treated as an anonymous inline
            // element."
            //
            // [§ 2.5 Text Runs](https://www.w3.org/TR/css-display-3/#text-nodes)
            //
            // "A text run is the most basic inline-level content, consisting of a
            // contiguous sequence of text."
            NodeType::Text(text) => {
                // [§ 4.3.1 White Space Phase I](https://www.w3.org/TR/css-text-3/#white-space-phase-1)
                //
                // Skip whitespace-only text nodes as they don't generate visible boxes.
                // NOTE: Full implementation would handle white-space property.
                if text.trim().is_empty() {
                    return None;
                }
                Some(LayoutBox {
                    box_type: BoxType::AnonymousInline(text.clone()),
                    dimensions: BoxDimensions::default(),
                    display: DisplayValue::inline(),
                    children: Vec::new(),
                    // Anonymous inline boxes have no margin/padding/border
                    margin: AutoEdgeSizes::default(),
                    padding: EdgeSizes::default(),
                    border_width: EdgeSizes::default(),
                    width: AutoOr::Auto,
                    height: AutoOr::Auto,
                })
            }
            // Comments do not generate boxes and are not part of the render tree.
            NodeType::Comment(_) => None,
        }
    }

    /// [§ 8 Box model](https://www.w3.org/TR/CSS2/box.html)
    ///
    /// Extract box model style values from the computed style.
    /// Returns (margin, padding, border_width, width, height).
    fn extract_box_style_values(
        style: Option<&ComputedStyle>,
    ) -> (AutoEdgeSizes, EdgeSizes, EdgeSizes, AutoOr, AutoOr) {
        let Some(s) = style else {
            // No computed style - use defaults (all auto/zero)
            return (
                AutoEdgeSizes::default(),
                EdgeSizes::default(),
                EdgeSizes::default(),
                AutoOr::Auto,
                AutoOr::Auto,
            );
        };

        // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
        //
        // "If the margin property is not set, the margin is 0."
        // "The value 'auto' is discussed in the section on calculating widths and margins."
        //
        // NOTE: For now, we treat explicit lengths as Length and missing values as Auto.
        // A full implementation would also parse 'auto' as an explicit value.
        let margin = AutoEdgeSizes {
            top: s
                .margin_top
                .as_ref()
                .map(|l| AutoOr::Length(l.to_px() as f32))
                .unwrap_or(AutoOr::Length(0.0)),
            right: s
                .margin_right
                .as_ref()
                .map(|l| AutoOr::Length(l.to_px() as f32))
                .unwrap_or(AutoOr::Length(0.0)),
            bottom: s
                .margin_bottom
                .as_ref()
                .map(|l| AutoOr::Length(l.to_px() as f32))
                .unwrap_or(AutoOr::Length(0.0)),
            left: s
                .margin_left
                .as_ref()
                .map(|l| AutoOr::Length(l.to_px() as f32))
                .unwrap_or(AutoOr::Length(0.0)),
        };

        // [§ 8.4 Padding properties](https://www.w3.org/TR/CSS2/box.html#padding-properties)
        //
        // "If the padding property is not set, the padding is 0."
        let padding = EdgeSizes {
            top: s
                .padding_top
                .as_ref()
                .map(|l| l.to_px() as f32)
                .unwrap_or(0.0),
            right: s
                .padding_right
                .as_ref()
                .map(|l| l.to_px() as f32)
                .unwrap_or(0.0),
            bottom: s
                .padding_bottom
                .as_ref()
                .map(|l| l.to_px() as f32)
                .unwrap_or(0.0),
            left: s
                .padding_left
                .as_ref()
                .map(|l| l.to_px() as f32)
                .unwrap_or(0.0),
        };

        // [§ 8.5 Border properties](https://www.w3.org/TR/CSS2/box.html#border-properties)
        //
        // "The initial value of border width is 'medium' (implementation-defined)."
        // For simplicity, we use the border width from BorderValue if present.
        let border_width = EdgeSizes {
            top: s
                .border_top
                .as_ref()
                .map(|b| b.width.to_px() as f32)
                .unwrap_or(0.0),
            right: s
                .border_right
                .as_ref()
                .map(|b| b.width.to_px() as f32)
                .unwrap_or(0.0),
            bottom: s
                .border_bottom
                .as_ref()
                .map(|b| b.width.to_px() as f32)
                .unwrap_or(0.0),
            left: s
                .border_left
                .as_ref()
                .map(|b| b.width.to_px() as f32)
                .unwrap_or(0.0),
        };

        // TODO: Extract explicit width/height from computed style when implemented
        let width = AutoOr::Auto;
        let height = AutoOr::Auto;

        (margin, padding, border_width, width, height)
    }

    /// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// "In a block formatting context, boxes are laid out one after the other,
    /// vertically, beginning at the top of a containing block."
    ///
    /// This method lays out this box and all its descendants.
    pub fn layout(&mut self, containing_block: Rect) {
        match self.display.outer {
            OuterDisplayType::Block => self.layout_block(containing_block),
            OuterDisplayType::Inline => {
                // TODO: Inline layout requires line box construction
                // [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
                todo!("Inline layout not yet implemented")
            }
            OuterDisplayType::RunIn => {
                // [§ 9.2.3 Run-in boxes](https://www.w3.org/TR/CSS2/visuren.html#run-in)
                todo!("Run-in layout not yet implemented")
            }
        }
    }

    /// [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///
    /// Layout algorithm for block-level boxes in normal flow.
    fn layout_block(&mut self, containing_block: Rect) {
        // STEP 1: Calculate width
        // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
        //
        // "The following constraints must hold among the used values of the
        // other properties:
        //
        // 'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
        // 'padding-right' + 'border-right-width' + 'margin-right'
        // = width of containing block"
        //
        // For now, we use the full containing block width (auto width behavior).
        self.calculate_block_width(containing_block);

        // STEP 2: Calculate horizontal position
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "Each box's left outer edge touches the left edge of the
        // containing block (for right-to-left formatting, right edges touch)."
        self.calculate_block_position(containing_block);

        // STEP 3: Layout children
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "In a block formatting context, boxes are laid out one after the
        // other, vertically, beginning at the top of a containing block."
        self.layout_block_children();

        // STEP 4: Calculate height
        // [§ 10.6.3 Block-level non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
        //
        // "If 'height' is 'auto', the height depends on whether the element
        // has any block-level children and whether it has padding or borders."
        //
        // "...the height is the distance between the top content edge and the
        // bottom edge of the last line box, if the box establishes an inline
        // formatting context... or the bottom edge of the bottom margin of
        // its last in-flow child, if the child's bottom margin does not
        // collapse with the element's bottom margin"
        self.calculate_block_height();
    }

    /// [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///
    /// Calculate the width of a block-level box.
    fn calculate_block_width(&mut self, containing_block: Rect) {
        // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
        //
        // "The following constraints must hold among the used values of the
        // other properties:
        //
        //   'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
        //   'padding-right' + 'border-right-width' + 'margin-right'
        //   = width of containing block"

        // STEP 1-4: Read the style values.
        // Border and padding cannot be 'auto', only margins and width can.
        let padding_left = self.padding.left;
        let padding_right = self.padding.right;
        let border_left = self.border_width.left;
        let border_right = self.border_width.right;
        let mut margin_left = self.margin.left;
        let mut margin_right = self.margin.right;
        let width = self.width;

        // STEP 5: Handle over-constrained case
        // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
        //
        // "If 'width' is not 'auto' and 'border-left-width' + 'padding-left' +
        // 'width' + 'padding-right' + 'border-right-width' (plus any of
        // 'margin-left' or 'margin-right' that are not 'auto') is larger than
        // the width of the containing block, then any 'auto' values for
        // 'margin-left' or 'margin-right' are, for the following rules,
        // treated as zero."
        if !width.is_auto() {
            let total = border_left
                + padding_left
                + width.to_px_or(0.0)
                + padding_right
                + border_right
                + margin_left.to_px_or(0.0)
                + margin_right.to_px_or(0.0);

            if total > containing_block.width {
                if margin_left.is_auto() {
                    margin_left = AutoOr::Length(0.0);
                }
                if margin_right.is_auto() {
                    margin_right = AutoOr::Length(0.0);
                }
            }
        }

        // STEP 6: Apply the constraint rules to calculate used values.
        let used_width: f32;
        let used_margin_left: f32;
        let used_margin_right: f32;

        // RULE A: "If 'width' is set to 'auto', any other 'auto' values become
        //         '0' and 'width' follows from the resulting equality."
        if width.is_auto() {
            used_margin_left = margin_left.to_px_or(0.0);
            used_margin_right = margin_right.to_px_or(0.0);
            used_width = containing_block.width
                - used_margin_left
                - used_margin_right
                - border_left
                - border_right
                - padding_left
                - padding_right;
        }
        // RULE B: "If both 'margin-left' and 'margin-right' are 'auto', their
        //         used values are equal. This horizontally centers the element
        //         with respect to the edges of the containing block."
        else if margin_left.is_auto() && margin_right.is_auto() {
            used_width = width.to_px_or(0.0);
            let remaining = containing_block.width
                - used_width
                - border_left
                - border_right
                - padding_left
                - padding_right;
            used_margin_left = remaining / 2.0;
            used_margin_right = remaining / 2.0;
        }
        // RULE C: "If there is exactly one value specified as 'auto', its used
        //         value follows from the equality."
        else if margin_left.is_auto() {
            used_width = width.to_px_or(0.0);
            used_margin_right = margin_right.to_px_or(0.0);
            used_margin_left = containing_block.width
                - used_width
                - used_margin_right
                - border_left
                - border_right
                - padding_left
                - padding_right;
        } else if margin_right.is_auto() {
            used_width = width.to_px_or(0.0);
            used_margin_left = margin_left.to_px_or(0.0);
            used_margin_right = containing_block.width
                - used_width
                - used_margin_left
                - border_left
                - border_right
                - padding_left
                - padding_right;
        }
        // RULE D: "If all of the above have a computed value other than 'auto',
        //         the values are said to be 'over-constrained' and one of the
        //         used values will have to be different from its computed value.
        //         If the 'direction' property of the containing block has the
        //         value 'ltr', the specified value of 'margin-right' is ignored
        //         and the value is calculated so as to make the equality true."
        else {
            used_width = width.to_px_or(0.0);
            used_margin_left = margin_left.to_px_or(0.0);
            // Over-constrained: adjust margin-right to satisfy the equation (assuming LTR)
            used_margin_right = containing_block.width
                - used_width
                - used_margin_left
                - border_left
                - border_right
                - padding_left
                - padding_right;
        }

        // STEP 7: Store the used values in self.dimensions
        self.dimensions.content.width = used_width;
        self.dimensions.margin.left = used_margin_left;
        self.dimensions.margin.right = used_margin_right;
        self.dimensions.padding.left = padding_left;
        self.dimensions.padding.right = padding_right;
        self.dimensions.border.left = border_left;
        self.dimensions.border.right = border_right;
    }

    /// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// Calculate the position of a block-level box.
    ///
    /// "Each box's left outer edge touches the left edge of the containing block
    /// (for right-to-left formatting, right edges touch)."
    fn calculate_block_position(&mut self, containing_block: Rect) {
        // [§ 8.1 Box dimensions](https://www.w3.org/TR/CSS2/box.html#box-dimensions)
        //
        // The position we store is the content box position. The content box
        // is nested inside padding, border, and margin boxes:
        //
        //   +-------------------------------------------+
        //   |                 margin                    |
        //   |   +-----------------------------------+   |
        //   |   |             border                |   |
        //   |   |   +---------------------------+   |   |
        //   |   |   |         padding           |   |   |
        //   |   |   |   +-------------------+   |   |   |
        //   |   |   |   |     content       |   |   |   |
        //   |   |   |   +-------------------+   |   |   |
        //   |   |   +---------------------------+   |   |
        //   |   +-----------------------------------+   |
        //   +-------------------------------------------+
        //
        // The containing_block represents the content area of our parent.
        // Our margin box is positioned within that area.

        // STEP 1: Calculate the x position of the content box.
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "Each box's left outer edge touches the left edge of the containing block."
        //
        // The left outer edge is the margin edge. So:
        //   margin_edge.x = containing_block.x
        //   content.x = margin_edge.x + margin.left + border.left + padding.left
        //
        // Note: margin.left was already computed in calculate_block_width and
        // stored in self.dimensions.margin.left
        self.dimensions.content.x = containing_block.x
            + self.dimensions.margin.left
            + self.dimensions.border.left
            + self.dimensions.padding.left;

        // STEP 2: Store the vertical box model values.
        // (We only stored horizontal values in calculate_block_width)
        // Must be done before calculating y position.
        self.dimensions.margin.top = self.margin.top.to_px_or(0.0);
        self.dimensions.margin.bottom = self.margin.bottom.to_px_or(0.0);
        self.dimensions.border.top = self.border_width.top;
        self.dimensions.border.bottom = self.border_width.bottom;
        self.dimensions.padding.top = self.padding.top;
        self.dimensions.padding.bottom = self.padding.bottom;

        // STEP 3: Calculate the y position of the content box.
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "In a block formatting context, boxes are laid out one after the other,
        // vertically, beginning at the top of a containing block."
        //
        // The containing_block.y is passed in by the parent and already accounts
        // for any siblings above us. So:
        //   margin_edge.y = containing_block.y
        //   content.y = margin_edge.y + margin.top + border.top + padding.top
        self.dimensions.content.y = containing_block.y
            + self.dimensions.margin.top
            + self.dimensions.border.top
            + self.dimensions.padding.top;
    }

    /// [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// Layout children in a block formatting context.
    ///
    /// "The vertical distance between two sibling boxes is determined by the
    /// 'margin' properties. Vertical margins between adjacent block-level
    /// boxes in a block formatting context collapse."
    fn layout_block_children(&mut self) {
        // TODO: Iterate over children, laying out each one
        // Track current_y position, advancing by each child's margin box height
        // Pass our content box as the containing block for children
        todo!("Layout block children")
    }

    /// [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
    ///
    /// Calculate the height of a block-level box.
    ///
    /// For 'height: auto', the height is determined by the children.
    fn calculate_block_height(&mut self) {
        // TODO: Calculate height
        // For auto height: sum of children's margin boxes
        // (margin collapsing not implemented yet)
        todo!("Calculate block height")
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
