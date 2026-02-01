//! Layout box types and layout algorithms.
//!
//! [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)

use std::collections::HashMap;

use koala_dom::{DomTree, NodeId, NodeType};

use crate::style::{AutoLength, ComputedStyle, DisplayValue, OuterDisplayType};

use super::box_model::{BoxDimensions, Rect};
use super::inline::FontMetrics;
use super::values::{AutoOr, UnresolvedAutoEdgeSizes, UnresolvedEdgeSizes};
use super::default_display_for_element;

/// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
///
/// "The following sections describe the types of boxes that may be generated
/// in CSS 2.1. A box's type affects, in part, its behavior in the visual
/// formatting model."
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// The type of box (principal, anonymous inline, anonymous block)
    pub box_type: BoxType,

    /// The computed dimensions of this box (used values after layout).
    pub dimensions: BoxDimensions,

    /// The display type of this box.
    pub display: DisplayValue,

    /// Child boxes in the layout tree.
    pub children: Vec<LayoutBox>,

    // ===== Computed style values (unresolved) =====
    // [§ 6 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
    //
    // These are the "computed" values from the cascade. Viewport-relative units
    // (vw, vh) are stored unresolved here and resolved to "used" values during
    // layout when the viewport dimensions are available.
    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    ///
    /// "Margins can be negative, but there may be implementation-specific limits."
    /// "The value 'auto' is discussed in the section on calculating widths and margins."
    ///
    /// Computed margin values (unresolved). Resolved during layout.
    pub margin: UnresolvedAutoEdgeSizes,

    /// [§ 8.4 Padding properties](https://www.w3.org/TR/CSS2/box.html#padding-properties)
    ///
    /// "Unlike margin properties, values for padding values cannot be negative."
    /// "The padding properties do not allow 'auto' as a value."
    ///
    /// Computed padding values (unresolved). Resolved during layout.
    pub padding: UnresolvedEdgeSizes,

    /// [§ 8.5 Border properties](https://www.w3.org/TR/CSS2/box.html#border-properties)
    ///
    /// "The border properties specify the width, color, and style of the border."
    ///
    /// Computed border-width values (unresolved). Resolved during layout.
    pub border_width: UnresolvedEdgeSizes,

    /// [§ 10.2 Content width: the 'width' property](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
    ///
    /// "This property specifies the content width of boxes."
    /// "The value 'auto' means that the width depends on the values of other properties."
    ///
    /// Computed width value (unresolved). None means 'auto'.
    pub width: Option<AutoLength>,

    /// [§ 10.5 Content height: the 'height' property](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    ///
    /// "This property specifies the content height of boxes."
    /// "The value 'auto' means that the height depends on the values of other properties."
    ///
    /// Computed height value (unresolved). None means 'auto'.
    pub height: Option<AutoLength>,
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
                    // Document has no margin/padding/border (all None = 0 when resolved)
                    margin: UnresolvedAutoEdgeSizes::default(),
                    padding: UnresolvedEdgeSizes::default(),
                    border_width: UnresolvedEdgeSizes::default(),
                    width: None,
                    height: None,
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
                    // Anonymous inline boxes have no margin/padding/border (all None = 0 when resolved)
                    margin: UnresolvedAutoEdgeSizes::default(),
                    padding: UnresolvedEdgeSizes::default(),
                    border_width: UnresolvedEdgeSizes::default(),
                    width: None,
                    height: None,
                })
            }
            // Comments do not generate boxes and are not part of the render tree.
            NodeType::Comment(_) => None,
        }
    }

    /// [§ 6 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
    ///
    /// "The computed value is the result of resolving the specified value...
    /// as far as possible without laying out the document."
    ///
    /// [§ 8 Box model](https://www.w3.org/TR/CSS2/box.html)
    ///
    /// Extract box model computed values from the style.
    /// These are unresolved values - viewport units (vw, vh) are preserved
    /// and resolved during layout when viewport dimensions are available.
    ///
    /// Returns (margin, padding, border_width, width, height) as unresolved values.
    fn extract_box_style_values(
        style: Option<&ComputedStyle>,
    ) -> (
        UnresolvedAutoEdgeSizes,
        UnresolvedEdgeSizes,
        UnresolvedEdgeSizes,
        Option<AutoLength>,
        Option<AutoLength>,
    ) {
        let Some(s) = style else {
            // [§ 6 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
            //
            // No computed style - use defaults (None for all, resolved to 0 during layout).
            return (
                UnresolvedAutoEdgeSizes::default(),
                UnresolvedEdgeSizes::default(),
                UnresolvedEdgeSizes::default(),
                None,
                None,
            );
        };

        // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
        //
        // "If the margin property is not set, the margin is 0."
        // "The value 'auto' is discussed in the section on calculating widths and margins."
        //
        // Store unresolved AutoLength values. Resolution happens during layout.
        let margin = UnresolvedAutoEdgeSizes {
            top: s.margin_top.clone(),
            right: s.margin_right.clone(),
            bottom: s.margin_bottom.clone(),
            left: s.margin_left.clone(),
        };

        // [§ 8.4 Padding properties](https://www.w3.org/TR/CSS2/box.html#padding-properties)
        //
        // "If the padding property is not set, the padding is 0."
        //
        // Store unresolved LengthValue values. Resolution happens during layout.
        let padding = UnresolvedEdgeSizes {
            top: s.padding_top.clone(),
            right: s.padding_right.clone(),
            bottom: s.padding_bottom.clone(),
            left: s.padding_left.clone(),
        };

        // [§ 8.5 Border properties](https://www.w3.org/TR/CSS2/box.html#border-properties)
        //
        // "The initial value of border width is 'medium' (implementation-defined)."
        //
        // Extract the width LengthValue from BorderValue. Resolution happens during layout.
        let border_width = UnresolvedEdgeSizes {
            top: s.border_top.as_ref().map(|b| b.width.clone()),
            right: s.border_right.as_ref().map(|b| b.width.clone()),
            bottom: s.border_bottom.as_ref().map(|b| b.width.clone()),
            left: s.border_left.as_ref().map(|b| b.width.clone()),
        };

        // [§ 10.2 Content width](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
        //
        // "This property specifies the content width of boxes."
        // None means 'auto' - width is calculated during layout.
        let width = s.width.clone();

        // [§ 10.5 Content height](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
        //
        // "This property specifies the content height of boxes."
        // None means 'auto' - height depends on content.
        let height = s.height.clone();

        (margin, padding, border_width, width, height)
    }

    /// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// "In a block formatting context, boxes are laid out one after the other,
    /// vertically, beginning at the top of a containing block."
    ///
    /// [§ 6.1 Used Values](https://www.w3.org/TR/css-cascade-4/#used)
    ///
    /// "The used value is the result of taking the computed value and
    /// completing any remaining calculations to make it the absolute
    /// theoretical value used in the layout of the document."
    ///
    /// This method lays out this box and all its descendants.
    /// The viewport is needed to resolve viewport-relative units (vw, vh).
    pub fn layout(&mut self, containing_block: Rect, viewport: Rect, font_metrics: &dyn FontMetrics) {
        match self.display.outer {
            OuterDisplayType::Block => self.layout_block(containing_block, viewport, font_metrics),
            OuterDisplayType::Inline => {
                // TODO: Implement proper inline layout with line box construction
                // [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
                //
                // Proper inline layout requires:
                //
                // STEP 1: Create or get parent's InlineFormattingContext
                //   // let ifc = parent.get_or_create_ifc();
                //
                // STEP 2: Add this inline box to the line
                //   // ifc.add_inline_box(self);
                //   // This may trigger line wrapping if box doesn't fit
                //
                // STEP 3: For inline boxes with children, recursively add children
                //   // for child in self.children {
                //   //     match child.display.outer {
                //   //         Inline => ifc.add_inline_box(child),
                //   //         Block => {
                //   //             // Breaks the line, starts block formatting
                //   //             ifc.break_line();
                //   //             child.layout_block(...);
                //   //             ifc.new_line_after_block();
                //   //         }
                //   //     }
                //   // }
                //
                // STEP 4: Calculate inline box dimensions from font metrics
                //   // self.dimensions.content.width = text_width;
                //   // self.dimensions.content.height = line_height;
                //
                // TEMPORARY: Fall back to block layout until inline is implemented.
                // This causes inline elements to stack vertically instead of horizontally.
                self.layout_block(containing_block, viewport, font_metrics)
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
    fn layout_block(&mut self, containing_block: Rect, viewport: Rect, font_metrics: &dyn FontMetrics) {
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
        self.calculate_block_width(containing_block, viewport);

        // STEP 2: Calculate horizontal position
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "Each box's left outer edge touches the left edge of the
        // containing block (for right-to-left formatting, right edges touch)."
        self.calculate_block_position(containing_block, viewport);

        // STEP 3: Layout children
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "In a block formatting context, boxes are laid out one after the
        // other, vertically, beginning at the top of a containing block."
        self.layout_block_children(viewport, font_metrics);

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
        self.calculate_block_height(viewport, font_metrics);
    }

    /// [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///
    /// Calculate the width of a block-level box.
    fn calculate_block_width(&mut self, containing_block: Rect, viewport: Rect) {
        // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
        //
        // "The following constraints must hold among the used values of the
        // other properties:
        //
        //   'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
        //   'padding-right' + 'border-right-width' + 'margin-right'
        //   = width of containing block"

        // STEP 1: Resolve computed values to used values.
        // [§ 6.1 Used Values](https://www.w3.org/TR/css-cascade-4/#used)
        //
        // "The used value is the result of taking the computed value and
        // completing any remaining calculations to make it the absolute
        // theoretical value used in the layout of the document."
        //
        // Viewport units (vw, vh) are resolved here using the viewport dimensions.
        let resolved_padding = self.padding.resolve(viewport);
        let resolved_border = self.border_width.resolve(viewport);
        let resolved_margin = self.margin.resolve(viewport);

        // STEP 2: Read the resolved values.
        // Border and padding cannot be 'auto', only margins and width can.
        let padding_left = resolved_padding.left;
        let padding_right = resolved_padding.right;
        let border_left = resolved_border.left;
        let border_right = resolved_border.right;
        let mut margin_left = resolved_margin.left;
        let mut margin_right = resolved_margin.right;

        // Resolve width: None means 'auto'
        let width = self.width.as_ref().map_or(AutoOr::Auto, |al| {
            UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport)
        });

        // STEP 3: Handle over-constrained case
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

        // STEP 4: Apply the constraint rules to calculate used values.
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

        // STEP 5: Store the used values in self.dimensions
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
    fn calculate_block_position(&mut self, containing_block: Rect, viewport: Rect) {
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

        // STEP 2: Resolve and store the vertical box model values.
        // [§ 6.1 Used Values](https://www.w3.org/TR/css-cascade-4/#used)
        //
        // (We only stored horizontal values in calculate_block_width)
        // Must be done before calculating y position.
        let resolved_padding = self.padding.resolve(viewport);
        let resolved_border = self.border_width.resolve(viewport);
        let resolved_margin = self.margin.resolve(viewport);

        self.dimensions.margin.top = resolved_margin.top.to_px_or(0.0);
        self.dimensions.margin.bottom = resolved_margin.bottom.to_px_or(0.0);
        self.dimensions.border.top = resolved_border.top;
        self.dimensions.border.bottom = resolved_border.bottom;
        self.dimensions.padding.top = resolved_padding.top;
        self.dimensions.padding.bottom = resolved_padding.bottom;

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

    /// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// Layout children in a block formatting context.
    ///
    /// "In a block formatting context, boxes are laid out one after the other,
    /// vertically, beginning at the top of a containing block."
    fn layout_block_children(&mut self, viewport: Rect, font_metrics: &dyn FontMetrics) {
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "In a block formatting context, boxes are laid out one after the other,
        // vertically, beginning at the top of a containing block. The vertical
        // distance between two sibling boxes is determined by the 'margin'
        // properties. Vertical margins between adjacent block-level boxes in a
        // block formatting context collapse."

        // STEP 1: Determine the containing block for children.
        // [§ 10.1 Definition of containing block](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
        //
        // "For other elements, if the element's position is 'relative' or 'static',
        // the containing block is formed by the content edge of the nearest
        // block container ancestor box."
        //
        // Our content box becomes the containing block for our children.
        // Children will be positioned relative to our content area.
        let content_box = self.dimensions.content_box();

        // STEP 2: Initialize the current Y position.
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // "...boxes are laid out one after the other, vertically, beginning at
        // the top of a containing block."
        //
        // Start at the top of our content box (y = 0 relative to content area,
        // but we pass absolute coordinates to children).
        let mut current_y = content_box.y;
        // STEP 3: Layout each child.
        // For each child box:
        //   a. Create a containing block rect with current_y as the y position
        //   b. Call child.layout(containing_block, viewport) to layout the child
        //   c. The child will calculate its own width, position, and height
        //
        // Note: We iterate over `self.children` but need mutable access to each child.
        for child in &mut self.children {
            // a. Create containing block for child
            let child_containing_block = Rect {
                x: content_box.x,
                y: current_y,
                width: content_box.width,
                height: f32::MAX, // Height is unconstrained for normal flow
            };

            // b. Layout the child (viewport is passed through for resolving vw/vh)
            child.layout(child_containing_block, viewport, font_metrics);

            // STEP 4: Advance the Y position.
            // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
            //
            // "The vertical distance between two sibling boxes is determined by the
            // 'margin' properties."
            //
            // After laying out each child, advance current_y by the child's margin
            // box height:
            //   current_y += child.dimensions.margin_box().height
            //
            // This positions the next sibling below the current one.
            current_y += child.dimensions.margin_box().height;
        }

        // STEP 5: Handle margin collapsing (NOT YET IMPLEMENTED).
        // [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
        //
        // "In CSS, the adjoining margins of two or more boxes can combine to
        // form a single margin. Margins that combine this way are said to collapse."
        //
        // TODO: Implement margin collapsing:
        //
        // fn collapse_margins(&mut self) {
        //     // STEP 1: Identify adjoining margins
        //     // "Two margins are adjoining if and only if:"
        //     //   - Both belong to in-flow block-level boxes in same BFC
        //     //   - No line boxes, clearance, padding, or border separate them
        //     //   - Both belong to vertically-adjacent box edges
        //
        //     // STEP 2: Calculate collapsed margin value
        //     // "When two or more margins collapse, the resulting margin width is
        //     //  the maximum of the collapsing margins' widths."
        //     // let collapsed = margins.iter().map(|m| m.abs()).max();
        //     //
        //     // "If there are no positive margins, the maximum of the absolute
        //     //  values of the adjoining margins is deducted from zero."
        //     // For negative margins: collapsed = max_positive + min_negative
        //
        //     // STEP 3: Handle parent-child collapsing
        //     // "If a box has margin-top that collapses with its first child's
        //     //  margin-top..."
        //
        //     // STEP 4: Handle empty boxes
        //     // "A box with zero min-height, zero or auto computed height, no
        //     //  inline content, and no border or padding... collapses its
        //     //  margin-top with its margin-bottom."
        // }
        //
        // NOTE: For now, we simply stack boxes without collapsing.
    }

    /// [§ 10.6.3 Block-level, non-replaced elements in normal flow when 'overflow' computes to 'visible'](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
    ///
    /// Calculate the height of a block-level box.
    ///
    /// "If 'height' is 'auto', the height depends on whether the element has
    /// any block-level children and whether it has padding or borders."
    fn calculate_block_height(&mut self, viewport: Rect, font_metrics: &dyn FontMetrics) {
        // STEP 1: Check if height is explicitly specified.
        // [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
        //
        // "If 'height' is not 'auto', then the used value is the specified
        // value."
        //
        // If height is a length (not auto), resolve it and use that value directly.
        if let Some(AutoLength::Length(l)) = &self.height {
            // [§ 6.1.1 Specified, computed, and actual values](https://www.w3.org/TR/CSS2/cascade.html#value-stages)
            //
            // Resolve the computed value to a used value using the viewport.
            self.dimensions.content.height =
                l.to_px_with_viewport(viewport.width as f64, viewport.height as f64) as f32;
            return;
        }

        // STEP 2: Handle anonymous inline boxes (text content).
        // [§ 9.2.1.1 Anonymous inline boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-inline)
        //
        // "Any text that is directly contained inside a block container element
        // (not inside an inline element) must be treated as an anonymous inline
        // element."
        //
        // [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
        //
        // "For inline boxes, this [contribution to line box height] is their
        // 'line-height'."
        //
        // "The height of the inline box encloses all glyphs and their half-leading
        // on each side and is thus exactly 'line-height'."
        if let BoxType::AnonymousInline(ref text) = self.box_type {
            if !text.trim().is_empty() {
                // [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
                //
                // "The 'line-height' property specifies the minimal height of line boxes
                // within the element."
                //
                // The default value for 'line-height' is 'normal', which the spec says:
                // "Tells user agents to set the used value to a 'reasonable' value based
                // on the font of the element. The value has the same meaning as <number>.
                // We recommend a used value for 'normal' between 1.0 to 1.2."
                //
                // Use FontMetrics to get the line height for the default font size (16px).
                let default_font_size: f32 = 16.0;
                let line_height = font_metrics.line_height(default_font_size);

                // Count lines in text content.
                // NOTE: This is a simplification. Proper implementation would wrap
                // text based on available width and font metrics.
                let line_count = text.lines().count().max(1);

                self.dimensions.content.height = (line_count as f32) * line_height;
                return;
            }
        }

        // STEP 3: Calculate 'auto' height from children.
        // [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
        //
        // "If 'height' is 'auto', the height depends on whether the element
        // has any block-level children..."
        //
        // For a block formatting context:
        // "the bottom edge of the bottom (possibly collapsed) margin of its
        // last in-flow child"
        //
        // NOTE: Margin collapsing is not yet implemented so we follow the
        // simplified approach below:
        //
        // Simplified (no margin collapsing): Sum the margin box heights of
        // all children:
        //   auto_height = self.children.iter()
        //       .map(|c| c.dimensions.margin_box().height)
        //       .sum()
        //
        // Then: self.dimensions.content.height = auto_height
        self.dimensions.content.height = self
            .children
            .iter()
            .map(|c| c.dimensions.margin_box().height)
            .sum();
    }

    /// [§ 10.4 Minimum and maximum widths: 'min-width' and 'max-width'](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
    ///
    /// "The following algorithm describes how the two properties influence
    /// the used value of the 'width' property:
    ///
    /// 1. The tentative used width is calculated (without 'min-width' and
    ///    'max-width') following the rules under 'Calculating widths and margins'.
    ///
    /// 2. If the tentative used width is greater than 'max-width', the rules
    ///    above are applied again, but this time using the computed value of
    ///    'max-width' as the computed value for 'width'.
    ///
    /// 3. If the resulting width is smaller than 'min-width', the rules above
    ///    are applied again, but this time using the value of 'min-width' as
    ///    the computed value for 'width'."
    ///
    /// NOTE: Requires `min-width` and `max-width` properties to be parsed
    /// into `ComputedStyle` before this can be implemented.
    ///
    /// TODO: Implement min/max width constraints:
    ///
    /// STEP 1: Get the tentative used width (already computed by calculate_block_width)
    ///   // let tentative_width = self.dimensions.content.width;
    ///
    /// STEP 2: Apply max-width constraint
    ///   // if let Some(max_width) = self.max_width {
    ///   //     let max_px = max_width.resolve(viewport);
    ///   //     if tentative_width > max_px {
    ///   //         // Re-run width calculation with max_width as the width
    ///   //         self.dimensions.content.width = max_px;
    ///   //         // Re-solve margin equation with new width
    ///   //     }
    ///   // }
    ///
    /// STEP 3: Apply min-width constraint
    ///   // if let Some(min_width) = self.min_width {
    ///   //     let min_px = min_width.resolve(viewport);
    ///   //     if self.dimensions.content.width < min_px {
    ///   //         self.dimensions.content.width = min_px;
    ///   //         // Re-solve margin equation with new width
    ///   //     }
    ///   // }
    fn apply_min_max_width(&mut self, _containing_block: Rect, _viewport: Rect) {
        todo!("Apply min-width/max-width constraints per CSS 2.1 § 10.4")
    }

    /// [§ 10.7 Minimum and maximum heights: 'min-height' and 'max-height'](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
    ///
    /// "The following algorithm describes how the two properties influence
    /// the used value of the 'height' property:
    ///
    /// 1. The tentative used height is calculated (without 'min-height' and
    ///    'max-height') following the rules under 'Calculating heights and margins'.
    ///
    /// 2. If this tentative height is greater than 'max-height', the rules
    ///    above are applied again, but this time using the value of
    ///    'max-height' as the computed value for 'height'.
    ///
    /// 3. If the resulting height is smaller than 'min-height', the rules
    ///    above are applied again, but this time using the value of
    ///    'min-height' as the computed value for 'height'."
    ///
    /// NOTE: Requires `min-height` and `max-height` properties to be parsed
    /// into `ComputedStyle` before this can be implemented.
    ///
    /// TODO: Implement min/max height constraints:
    ///
    /// STEP 1: Get the tentative used height (already computed by calculate_block_height)
    ///   // let tentative_height = self.dimensions.content.height;
    ///
    /// STEP 2: Apply max-height constraint
    ///   // if let Some(max_height) = self.max_height {
    ///   //     let max_px = max_height.resolve(viewport);
    ///   //     if tentative_height > max_px {
    ///   //         self.dimensions.content.height = max_px;
    ///   //     }
    ///   // }
    ///
    /// STEP 3: Apply min-height constraint
    ///   // if let Some(min_height) = self.min_height {
    ///   //     let min_px = min_height.resolve(viewport);
    ///   //     if self.dimensions.content.height < min_px {
    ///   //         self.dimensions.content.height = min_px;
    ///   //     }
    ///   // }
    fn apply_min_max_height(&mut self, _viewport: Rect) {
        todo!("Apply min-height/max-height constraints per CSS 2.1 § 10.7")
    }

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// "In CSS, the adjoining margins of two or more boxes (which might or
    /// might not be siblings) can combine to form a single margin. Margins
    /// that combine this way are said to collapse, and the resulting combined
    /// margin is called a collapsed margin."
    ///
    /// "When two or more margins collapse, the resulting margin width is the
    /// maximum of the collapsing margins' widths. In the case of negative
    /// margins, the maximum of the absolute values of the negative adjoining
    /// margins is deducted from the maximum of the positive adjoining margins.
    /// If there are no positive margins, the maximum of the absolute values
    /// of the adjoining margins is deducted from zero."
    ///
    /// TODO: Implement margin collapsing:
    ///
    /// STEP 1: Walk through children pairwise
    ///   // For each pair of adjacent siblings (child_a, child_b):
    ///   //   margin_bottom_a = child_a.dimensions.margin.bottom
    ///   //   margin_top_b = child_b.dimensions.margin.top
    ///
    /// STEP 2: Determine if margins are adjoining
    ///   // [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///   // "Two margins are adjoining if and only if:
    ///   //   - both belong to in-flow block-level boxes in the same BFC
    ///   //   - no line boxes, no clearance, no padding and no border
    ///   //     separate them"
    ///   //
    ///   // Check: child_a has no bottom border/padding, child_b has no top border/padding
    ///   // Check: neither is a float or absolutely positioned
    ///
    /// STEP 3: Calculate collapsed margin
    ///   // if both positive: collapsed = max(margin_a, margin_b)
    ///   // if both negative: collapsed = min(margin_a, margin_b)
    ///   // if mixed: collapsed = margin_a + margin_b (positive + negative)
    ///
    /// STEP 4: Adjust child positions
    ///   // collapsed_gap = collapsed_margin - (margin_bottom_a + margin_top_b)
    ///   // Shift child_b and all subsequent children up by the difference
    ///
    /// STEP 5: Handle parent-child margin collapsing
    ///   // "The top margin of an in-flow block element collapses with its
    ///   //  first in-flow block-level child's top margin if the element
    ///   //  has no top border, no top padding, and the child has no clearance."
    ///   //
    ///   // If self has no top border/padding:
    ///   //   collapsed_top = max(self.margin.top, first_child.margin.top)
    ///   //   self.margin.top = collapsed_top
    ///   //   first_child.margin.top = 0
    fn collapse_margins(&mut self) {
        todo!("Collapse vertical margins between children per CSS 2.1 § 8.3.1")
    }

    /// [§ 9.2.1.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
    ///
    /// "When an inline box contains an in-flow block-level box, the inline box
    /// (and its inline ancestors within the same line box) are broken around
    /// the block-level box (and any block-level siblings that are consecutive
    /// or separated only by collapsible whitespace and/or out-of-flow elements),
    /// splitting the inline box into two boxes (even if either side is empty),
    /// one on each side of the block-level box(es). The line boxes before the
    /// break and after the break are enclosed in anonymous block boxes, and
    /// the block-level box becomes a sibling of those anonymous boxes."
    ///
    /// Example:
    /// ```html
    /// <div>Some text <p>block paragraph</p> more text</div>
    /// ```
    /// Generates:
    /// ```text
    /// Anonymous block box: "Some text"
    /// <p> block box: "block paragraph"
    /// Anonymous block box: "more text"
    /// ```
    ///
    /// TODO: Implement anonymous box generation:
    ///
    /// STEP 1: Check if children are mixed (both block and inline)
    ///   // let has_block_children = self.children.iter()
    ///   //     .any(|c| c.display.outer == OuterDisplayType::Block);
    ///   // let has_inline_children = self.children.iter()
    ///   //     .any(|c| c.display.outer == OuterDisplayType::Inline);
    ///   //
    ///   // if !(has_block_children && has_inline_children) {
    ///   //     return; // No mixed content, no anonymous boxes needed
    ///   // }
    ///
    /// STEP 2: Group consecutive inline children into anonymous block boxes
    ///   // Walk children, accumulating runs of inline boxes.
    ///   // When a block child is encountered:
    ///   //   - Wrap the accumulated inline run in an AnonymousBlock
    ///   //   - Add the block child as-is
    ///   //   - Start a new inline run
    ///   // After the loop, wrap any remaining inline run.
    ///
    /// STEP 3: Replace self.children with the new list
    ///   // self.children = new_children;
    pub fn generate_anonymous_boxes(&mut self) {
        todo!("Generate anonymous block boxes for mixed inline/block content per CSS 2.1 § 9.2.1.1")
    }

    /// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// Layout children in an inline formatting context.
    ///
    /// "In an inline formatting context, boxes are laid out horizontally,
    /// one after the other, beginning at the top of a containing block."
    ///
    /// This is the counterpart to `layout_block_children` — called when
    /// all children are inline-level.
    ///
    /// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
    ///
    /// "The height of the line box is determined by the rules given in the
    /// section on line height calculations."
    ///
    /// TODO: Implement inline children layout:
    ///
    /// STEP 1: Create an InlineLayout context
    ///   // let mut inline_layout = InlineLayout::new(
    ///   //     self.dimensions.content.width,
    ///   //     self.dimensions.content.y,
    ///   // );
    ///
    /// STEP 2: Add each child to the inline layout
    ///   // for child in &self.children {
    ///   //     match &child.box_type {
    ///   //         BoxType::AnonymousInline(text) => {
    ///   //             inline_layout.add_text(text, font_size);
    ///   //         }
    ///   //         BoxType::Principal(_) if child.display.outer == Inline => {
    ///   //             // Recursively process inline box children
    ///   //             inline_layout.add_inline_box(width, height);
    ///   //         }
    ///   //         BoxType::Principal(_) => {
    ///   //             // Block-level child interrupts inline flow
    ///   //             // [§ 9.2.1.1] This shouldn't happen if
    ///   //             // generate_anonymous_boxes was called first
    ///   //         }
    ///   //         _ => {}
    ///   //     }
    ///   // }
    ///
    /// STEP 3: Finalize the last line
    ///   // inline_layout.finish_line();
    ///
    /// STEP 4: Set content height from line boxes
    ///   // self.dimensions.content.height = inline_layout.total_height();
    fn layout_inline_children(&mut self, _viewport: Rect, _font_metrics: &dyn FontMetrics) {
        todo!("Layout inline children using inline formatting context per CSS 2.1 § 9.4.2")
    }

    /// [§ 10.3.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
    ///
    /// "A replaced element is an element whose content is outside the scope of
    /// the CSS formatting model, such as an image, embedded document, or applet."
    ///
    /// [§ 10.3.2](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
    ///
    /// "If 'width' has a computed value of 'auto', and the element has an
    /// intrinsic width, then that intrinsic width is the used value of 'width'."
    ///
    /// [§ 10.6.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-height)
    ///
    /// "If 'height' has a computed value of 'auto', and the element has an
    /// intrinsic height, then that intrinsic height is the used value of 'height'."
    ///
    /// "If 'height' and 'width' both have computed values of 'auto' and the
    /// element has an intrinsic ratio but no intrinsic height or width, then
    /// the used value of 'width' is undefined in CSS 2.1. However, it is
    /// suggested that, if the containing block's width does not itself depend
    /// on the replaced element's width, then the used value of 'width' is
    /// calculated from the constraint equation used for block-level,
    /// non-replaced elements in normal flow."
    ///
    /// TODO: Implement replaced element sizing:
    ///
    /// STEP 1: Get intrinsic dimensions
    ///   // (intrinsic_width, intrinsic_height, intrinsic_ratio)
    ///   // For <img>: from the image's natural dimensions
    ///   // For <video>: from the video's natural dimensions
    ///   // For <canvas>: 300x150 default
    ///   // For <iframe>: 300x150 default
    ///
    /// STEP 2: Resolve width
    ///   // If width is auto:
    ///   //   If has intrinsic width → use intrinsic width
    ///   //   Else if has intrinsic ratio and resolved height → width = height * ratio
    ///   //   Else → use 300px (CSS2 default for replaced elements)
    ///   // Else: use specified width
    ///
    /// STEP 3: Resolve height
    ///   // If height is auto:
    ///   //   If has intrinsic height → use intrinsic height
    ///   //   Else if has intrinsic ratio and resolved width → height = width / ratio
    ///   //   Else → use 150px (CSS2 default)
    ///   // Else: use specified height
    fn layout_replaced_element(
        &mut self,
        _intrinsic_width: Option<f32>,
        _intrinsic_height: Option<f32>,
        _intrinsic_ratio: Option<f32>,
        _viewport: Rect,
    ) {
        todo!("Layout replaced element (img, video, etc.) per CSS 2.1 § 10.3.2 / § 10.6.2")
    }

    /// [§ 11.1 Overflow and clipping](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    ///
    /// "This property specifies whether content of a block container element
    /// is clipped when it overflows the element's box."
    ///
    /// "Values have the following meanings:
    ///
    /// visible
    ///   This value indicates that content is not clipped, i.e., it may be
    ///   rendered outside the block box.
    ///
    /// hidden
    ///   This value indicates that the content is clipped and that no
    ///   scrolling user interface should be provided to view the content
    ///   outside the clipping region.
    ///
    /// scroll
    ///   This value indicates that the content is clipped and that if the
    ///   user agent uses a scrolling mechanism that is visible on the screen
    ///   (such as a scroll bar or a panner), that mechanism should be
    ///   displayed for a box whether or not any of its content is clipped.
    ///
    /// auto
    ///   The behavior of the 'auto' value is user agent-dependent, but
    ///   should cause a scrolling mechanism to be provided for overflowing boxes."
    ///
    /// NOTE: Requires `overflow` property to be parsed into `ComputedStyle`.
    ///
    /// TODO: Implement overflow handling:
    ///
    /// STEP 1: Determine if content overflows
    ///   // let content_height = self.dimensions.content.height;
    ///   // let box_height = specified_height or auto;
    ///   // overflows = content_height > box_height
    ///
    /// STEP 2: Apply clipping if overflow is hidden/scroll/auto
    ///   // Create a clip rect matching the padding box
    ///   // clip_rect = self.dimensions.padding_box();
    ///
    /// STEP 3: Handle scrollable overflow
    ///   // [CSS Overflow Module Level 3 § 2](https://www.w3.org/TR/css-overflow-3/#overflow-properties)
    ///   // Calculate scrollable overflow region:
    ///   // "The scrollable overflow region is the union of the border boxes
    ///   //  of all descendants that extend beyond the padding edge."
    ///   // scroll_width = max(child.margin_box().x + child.margin_box().width) - content.x
    ///   // scroll_height = max(child.margin_box().y + child.margin_box().height) - content.y
    fn apply_overflow_clipping(&self) -> Option<Rect> {
        todo!("Apply overflow clipping per CSS 2.1 § 11.1")
    }

    /// [§ 10.3.5 Floating, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#float-width)
    ///
    /// "If 'width' is computed as 'auto', the used value is the 'shrink-to-fit'
    /// width."
    ///
    /// [§ 10.3.5 Shrink-to-fit width](https://www.w3.org/TR/CSS2/visudet.html#float-width)
    ///
    /// "Calculation of the shrink-to-fit width is similar to calculating the
    /// width of a table cell using the automatic table layout algorithm. Roughly:
    /// calculate the preferred width by formatting the content without breaking
    /// lines other than where explicit line breaks occur, and also calculate
    /// the preferred minimum width, e.g., by trying all possible line breaks.
    /// CSS 2.1 does not define the exact algorithm.
    ///
    /// Thirdly, find the available width: this is found by solving for 'width'
    /// after setting 'left' (in case 2) or 'right' (in case 4) to 0.
    ///
    /// Then the shrink-to-fit width is:
    ///   min(max(preferred minimum width, available width), preferred width)"
    ///
    /// TODO: Implement shrink-to-fit width:
    ///
    /// STEP 1: Calculate preferred width
    ///   // Format content with no line breaks except explicit ones.
    ///   // preferred_width = max line width across all lines
    ///
    /// STEP 2: Calculate preferred minimum width
    ///   // Try all possible line breaks.
    ///   // preferred_min_width = max word width (or widest unbreakable unit)
    ///
    /// STEP 3: Calculate available width
    ///   // available_width = containing_block.width - margins - borders - padding
    ///
    /// STEP 4: Compute shrink-to-fit width
    ///   // shrink_to_fit = min(max(preferred_min_width, available_width), preferred_width)
    fn shrink_to_fit_width(&self, _containing_block: Rect, _viewport: Rect) -> f32 {
        todo!("Calculate shrink-to-fit width per CSS 2.1 § 10.3.5")
    }

    /// [§ 9.2.1.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
    ///
    /// Determine whether this box's children need anonymous box wrapping.
    ///
    /// "When an inline-level box contains a block-level box, the inline-level
    /// box (and its inline ancestors within the same line box) are broken
    /// around the block-level box."
    ///
    /// Returns true if children contain both block-level and inline-level boxes.
    pub fn has_mixed_children(&self) -> bool {
        let mut has_block = false;
        let mut has_inline = false;
        for child in &self.children {
            match child.display.outer {
                OuterDisplayType::Block => has_block = true,
                OuterDisplayType::Inline => has_inline = true,
                _ => {}
            }
            if has_block && has_inline {
                return true;
            }
        }
        false
    }

    /// [§ 9.4.1 Block formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
    ///
    /// Determine whether all children of this box are inline-level.
    ///
    /// "In a block formatting context, boxes are laid out one after the
    /// other, vertically..."
    ///
    /// If all children are inline-level, the parent establishes an
    /// inline formatting context for its contents instead.
    pub fn all_children_inline(&self) -> bool {
        self.children.iter().all(|c| c.display.outer == OuterDisplayType::Inline)
    }
}
