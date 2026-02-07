//! Layout box types and layout algorithms.
//!
//! [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)

use std::collections::HashMap;

#[cfg(feature = "layout-trace")]
use std::cell::Cell;

use koala_dom::{DomTree, NodeId, NodeType};

use crate::style::{
    AutoLength, ColorValue, ComputedStyle, DisplayValue, InnerDisplayType, OuterDisplayType,
};

use super::box_model::{BoxDimensions, Rect};
use super::default_display_for_element;
use super::inline::{FontMetrics, FontStyle, InlineLayout, LineBox, TextAlign};
use super::positioned::{BoxOffsets, PositionType, PositionedLayout};
use super::values::{AutoOr, UnresolvedAutoEdgeSizes, UnresolvedEdgeSizes};

#[cfg(feature = "layout-trace")]
thread_local! {
    static LAYOUT_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
///
/// "When two or more margins collapse, the resulting margin width is the
/// maximum of the collapsing margins' widths. In the case of negative
/// margins, the maximum of the absolute values of the negative adjoining
/// margins is deducted from the maximum of the positive adjoining margins.
/// If there are no positive margins, the maximum of the absolute values
/// of the adjoining margins is deducted from zero."
fn collapse_two_margins(a: f32, b: f32) -> f32 {
    if a >= 0.0 && b >= 0.0 {
        a.max(b)
    } else if a < 0.0 && b < 0.0 {
        a.min(b)
    } else {
        a + b
    }
}

/// Recursively walk inline-level children, feeding their content into an
/// `InlineLayout`.
///
/// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
///
/// "In an inline formatting context, boxes are laid out horizontally,
/// one after the other, beginning at the top of a containing block.
/// Horizontal margins, borders, and padding are respected between
/// these boxes."
///
/// For non-replaced inline boxes (`<span>`, `<a>`, etc.), the content
/// participates in the parent's inline formatting context:
///
/// [§ 9.2.2](https://www.w3.org/TR/CSS2/visuren.html#inline-boxes)
///
/// "An inline box is one that is both inline-level and whose contents
/// participate in its containing inline formatting context."
#[allow(clippy::too_many_arguments)]
fn layout_inline_content(
    children: &mut [LayoutBox],
    inline_layout: &mut InlineLayout,
    inherited_font_size: f32,
    inherited_color: &ColorValue,
    inherited_font_weight: u16,
    inherited_font_style: FontStyle,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
    content_rect: Rect,
) {
    for child in children.iter_mut() {
        // [§ 9.3](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
        //
        // Absolute/fixed children are out of flow and do not participate
        // in inline formatting.
        if matches!(
            child.position_type,
            PositionType::Absolute | PositionType::Fixed
        ) {
            continue;
        }

        match &child.box_type {
            BoxType::AnonymousInline(text) => {
                // [§ 9.2.1.1 Anonymous inline boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-inline)
                //
                // "Any text that is directly contained inside a block
                // container element... must be treated as an anonymous
                // inline element."
                //
                // [§ 4 Inheritance](https://www.w3.org/TR/css-cascade-4/#inheriting)
                //
                // Anonymous inline boxes inherit font-size and color from
                // their parent element.
                inline_layout.add_text(
                    text,
                    inherited_font_size,
                    inherited_color,
                    inherited_font_weight,
                    inherited_font_style,
                    font_metrics,
                );
            }
            BoxType::Principal(_) if child.display.outer == OuterDisplayType::Inline => {
                // [§ 9.2.2 Inline-level elements and inline boxes](https://www.w3.org/TR/CSS2/visuren.html#inline-boxes)
                //
                // "An inline box is one that is both inline-level and whose
                // contents participate in its containing inline formatting
                // context."
                //
                // Non-replaced inline boxes do not form opaque fragments.
                // Their left margin+border+padding is applied, then their
                // children are recursively laid out in the same inline
                // formatting context, then their right margin+border+padding
                // is applied.

                // STEP 1: Resolve the inline box's edge sizes.
                let resolved_padding = child.padding.resolve(viewport);
                let resolved_border = child.border_width.resolve(viewport);
                let resolved_margin = child.margin.resolve(viewport);

                // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
                //
                // "If 'margin-left' or 'margin-right' are computed as 'auto',
                // their used value is '0'."
                let margin_left = resolved_margin.left.to_px_or(0.0);
                let margin_right = resolved_margin.right.to_px_or(0.0);

                let left_mbp = margin_left + resolved_border.left + resolved_padding.left;
                let right_mbp = resolved_padding.right + resolved_border.right + margin_right;

                // STEP 2: Open the inline box (apply left edge).
                inline_layout.begin_inline_box(left_mbp);

                // STEP 3: Recursively lay out the inline box's children.
                //
                // [§ 4 Inheritance](https://www.w3.org/TR/css-cascade-4/#inheriting)
                //
                // The child element's own font-size and color are used for
                // its descendants. These were resolved from ComputedStyle
                // during build_layout_tree().
                layout_inline_content(
                    &mut child.children,
                    inline_layout,
                    child.font_size,
                    &child.color,
                    child.font_weight,
                    child.font_style,
                    viewport,
                    font_metrics,
                    content_rect,
                );

                // STEP 4: Close the inline box (apply right edge).
                inline_layout.end_inline_box(right_mbp);
            }
            BoxType::Principal(_) | BoxType::AnonymousBlock => {
                // [§ 9.2.1.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
                //
                // "When an inline box contains an in-flow block-level box,
                // the inline box (and its inline ancestors within the same
                // line box) are broken around the block-level box... The line
                // boxes before the break and after the break are enclosed in
                // anonymous block boxes, and the block-level box becomes a
                // sibling of those anonymous boxes."
                //
                // Break the inline context: flush the current line, lay out
                // the block child, and resume inline layout below it.

                // STEP 1: Flush any accumulated inline content into a line box.
                inline_layout.finish_line();

                // STEP 2: Create a containing block for the block child.
                // The block child is positioned at the full width of the
                // parent block container, not narrowed by any inline box
                // margin/border/padding.
                let block_cb = Rect {
                    x: content_rect.x,
                    y: inline_layout.current_y,
                    width: content_rect.width,
                    height: f32::MAX,
                };

                // STEP 3: Layout the block child.
                child.layout(block_cb, viewport, font_metrics);

                // STEP 4: Advance past the block child's margin box.
                inline_layout.current_y += child.dimensions.margin_box().height;
                inline_layout.current_x = 0.0;
            }
        }
    }
}

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
    /// Contains the `NodeId` to reference back to the DOM element.
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
    /// "In a document like this: `<div>`Some text`<p>`More text`</p></div>`
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

    /// [§ 3.5 'font-size'](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
    ///
    /// "This property indicates the desired height of glyphs from the font."
    ///
    /// Resolved font size in pixels. Used during inline layout to size text.
    /// Defaults to 16.0 (the CSS 'medium' value per UA stylesheet conventions).
    pub font_size: f32,

    /// [§ 3.1 'color'](https://www.w3.org/TR/css-color-4/#the-color-property)
    ///
    /// "This property describes the foreground color of an element's text content."
    ///
    /// Inherited text color for this box. Used during inline layout painting.
    pub color: ColorValue,

    /// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
    ///
    /// "This property describes how inline-level content of a block
    /// container is aligned."
    ///
    /// Inherited from `ComputedStyle`. Passed to `InlineLayout` when this box
    /// establishes an inline formatting context.
    pub text_align: TextAlign,

    /// [§ 3.2 'font-weight'](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
    ///
    /// "This property specifies the weight of glyphs in the font."
    ///
    /// Numeric weight: 400 = normal, 700 = bold. Inherited from `ComputedStyle`.
    pub font_weight: u16,

    /// [§ 3.3 'font-style'](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
    ///
    /// "The 'font-style' property allows italic or oblique faces to be selected."
    ///
    /// Inherited from `ComputedStyle`.
    pub font_style: FontStyle,

    /// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// Completed line boxes from inline layout. Populated when this box
    /// establishes an inline formatting context (i.e., all children are
    /// inline-level). The painter reads from these to render text at the
    /// correct positions.
    pub line_boxes: Vec<LineBox>,

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// The effective top margin after collapsing with the first child's top
    /// margin (parent-child collapsing). When set, the parent's layout
    /// context should use this instead of `dimensions.margin.top` for
    /// sibling collapsing at the grandparent level.
    pub collapsed_margin_top: Option<f32>,

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// The effective bottom margin after collapsing with the last child's
    /// bottom margin (parent-child collapsing). When set, the parent's
    /// layout context should use this instead of `dimensions.margin.bottom`
    /// for sibling collapsing at the grandparent level.
    pub collapsed_margin_bottom: Option<f32>,

    // ===== Replaced element fields =====
    /// [§ 10.3.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
    ///
    /// "A replaced element is an element whose content is outside the scope
    /// of the CSS formatting model, such as an image, embedded document,
    /// or applet."
    ///
    /// True if this box represents a replaced element (e.g., `<img>`).
    pub is_replaced: bool,

    /// The `src` attribute value for replaced elements.
    ///
    /// Used as a key to look up image data at paint/render time.
    pub replaced_src: Option<String>,

    /// Intrinsic width of the replaced content in pixels.
    ///
    /// [§ 10.3.2](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
    ///
    /// "If 'width' has a computed value of 'auto', and the element has an
    /// intrinsic width, then that intrinsic width is the used value of 'width'."
    pub intrinsic_width: Option<f32>,

    /// Intrinsic height of the replaced content in pixels.
    ///
    /// [§ 10.6.2](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-height)
    ///
    /// "If 'height' has a computed value of 'auto', and the element has an
    /// intrinsic height, then that intrinsic height is the used value of 'height'."
    pub intrinsic_height: Option<f32>,

    // ===== Flexbox fields =====
    /// [§ 5.1 'flex-direction'](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
    ///
    /// "The flex-direction property specifies how flex items are placed in
    /// the flex container."
    /// Initial: "row"
    pub flex_direction: String,

    /// [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
    ///
    /// "The justify-content property aligns flex items along the main axis."
    /// Initial: "flex-start"
    pub justify_content: String,

    /// [§ 7.2 'flex-grow'](https://www.w3.org/TR/css-flexbox-1/#flex-grow-property)
    ///
    /// "The flex-grow property sets the flex grow factor."
    /// Initial: 0
    pub flex_grow: f32,

    /// [§ 7.3 'flex-shrink'](https://www.w3.org/TR/css-flexbox-1/#flex-shrink-property)
    ///
    /// "The flex-shrink property sets the flex shrink factor."
    /// Initial: 1
    pub flex_shrink: f32,

    /// [§ 7.1 'flex-basis'](https://www.w3.org/TR/css-flexbox-1/#flex-basis-property)
    ///
    /// "The flex-basis property sets the flex basis."
    /// None = auto
    pub flex_basis: Option<AutoLength>,

    // ===== Positioning fields =====
    /// [§ 9.3.1 'position'](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
    ///
    /// "The 'position' and 'float' properties determine which of the CSS 2
    /// positioning algorithms is used to calculate the position of a box."
    pub position_type: PositionType,

    /// [§ 9.3.2 Box offsets](https://www.w3.org/TR/CSS2/visuren.html#position-props)
    ///
    /// "An element is said to be positioned if its 'position' property has
    /// a value other than 'static'. Positioned elements generate positioned
    /// boxes, laid out according to four properties: top, right, bottom, left."
    pub offsets: BoxOffsets,
}

impl LayoutBox {
    // ── Margin collapsing helpers ──────────────────────────────────────

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// Return the effective top margin for this box, accounting for
    /// parent-child collapsing. If no collapsing occurred, falls back
    /// to the resolved `dimensions.margin.top`.
    #[must_use]
    pub fn effective_margin_top(&self) -> f32 {
        self.collapsed_margin_top
            .unwrap_or(self.dimensions.margin.top)
    }

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// Return the effective bottom margin for this box, accounting for
    /// parent-child collapsing. If no collapsing occurred, falls back
    /// to the resolved `dimensions.margin.bottom`.
    #[must_use]
    pub fn effective_margin_bottom(&self) -> f32 {
        self.collapsed_margin_bottom
            .unwrap_or(self.dimensions.margin.bottom)
    }

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// "Two margins are adjoining if and only if:
    ///   ...no... padding [or] border... separate them"
    ///
    /// Returns true if this box has a non-zero top border or top padding,
    /// which prevents parent-child top margin collapsing.
    fn has_top_border_or_padding(&self) -> bool {
        self.dimensions.border.top > 0.0 || self.dimensions.padding.top > 0.0
    }

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// Returns true if this box has a non-zero bottom border or bottom
    /// padding, which prevents parent-child bottom margin collapsing.
    fn has_bottom_border_or_padding(&self) -> bool {
        self.dimensions.border.bottom > 0.0 || self.dimensions.padding.bottom > 0.0
    }

    /// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
    ///
    /// "A box's own margins collapse if the 'min-height' property is
    /// computed as zero, the 'height' property is computed as zero or
    /// 'auto', it does not establish a new block formatting context, and
    /// it contains no in-flow content (i.e., has no in-flow line boxes
    /// and no in-flow block-level children)."
    fn is_empty_collapsible_box(&self) -> bool {
        // min-height is not yet implemented — effectively zero for all boxes.

        // height must be zero or auto.
        let height_zero_or_auto = match &self.height {
            None | Some(AutoLength::Auto) => true,
            Some(AutoLength::Length(l)) => l.to_px() == 0.0,
        };
        if !height_zero_or_auto {
            return false;
        }

        // Must have no in-flow content and no border/padding separating
        // the top and bottom margins.
        self.children.is_empty()
            && self.line_boxes.is_empty()
            && !self.has_top_border_or_padding()
            && !self.has_bottom_border_or_padding()
    }

    /// Maximum recursion depth for `measure_content_size()`.
    ///
    /// This prevents stack overflow on deeply nested DOM trees. When the
    /// flex layout path calls `measure_content_size()`, it adds recursive
    /// depth on top of the existing `layout()` recursion. Real pages like
    /// Google's homepage can have DOM trees hundreds of levels deep, so
    /// capping measurement depth keeps the total stack usage bounded.
    const MAX_MEASURE_DEPTH: usize = 64;

    /// Compute intrinsic max-content width without performing full layout.
    ///
    /// [§ 9.9.1 Flex Item Intrinsic Size Contributions](https://www.w3.org/TR/css-flexbox-1/#intrinsic-item-contributions)
    ///
    /// This is a READ-ONLY measurement — it does NOT modify positions or
    /// store layout results. It only computes the natural content width.
    ///
    /// Recursion safety: depth-limited to [`Self::MAX_MEASURE_DEPTH`].
    /// Never calls `layout()`; `layout()` never calls this.
    #[must_use]
    pub fn measure_content_size(&self, viewport: Rect, font_metrics: &dyn FontMetrics) -> f32 {
        self.measure_content_size_inner(viewport, font_metrics, 0)
    }

    fn measure_content_size_inner(
        &self,
        viewport: Rect,
        font_metrics: &dyn FontMetrics,
        depth: usize,
    ) -> f32 {
        // Case 1: Text nodes — measure text width on a single line (max-content).
        if let BoxType::AnonymousInline(ref text) = self.box_type {
            return font_metrics.text_width(text, self.font_size);
        }

        // Case 2: Replaced elements — use intrinsic width or fallback.
        if self.is_replaced {
            return self.intrinsic_width.unwrap_or(300.0);
        }

        // Case 3: Explicit width — resolve and return.
        if let Some(ref w) = self.width {
            let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(w, viewport);
            if !resolved.is_auto() {
                return resolved.to_px_or(0.0);
            }
        }

        // Depth guard: stop recursing into children beyond the limit.
        // Items at excessive depth are treated as zero-width; the flex
        // algorithm will distribute remaining space via flex-grow.
        if depth >= Self::MAX_MEASURE_DEPTH {
            return 0.0;
        }

        // Case 4: Auto width — sum up children's content sizes.
        //
        // Resolve padding and border on the main axis so we account for
        // them in the intrinsic size.
        let resolved_padding = self.padding.resolve(viewport);
        let resolved_border = self.border_width.resolve(viewport);
        let extra = resolved_padding.left
            + resolved_padding.right
            + resolved_border.left
            + resolved_border.right;

        if self.children.is_empty() {
            return extra;
        }

        // If all children are inline, max-content = sum of text widths
        // (no line breaking).
        if self.all_children_inline() {
            let inline_sum: f32 = self
                .children
                .iter()
                .map(|c| c.measure_content_size_inner(viewport, font_metrics, depth + 1))
                .sum();
            return inline_sum + extra;
        }

        // If children are block-level, max-content = max of children's
        // content sizes.
        let block_max = self
            .children
            .iter()
            .map(|c| c.measure_content_size_inner(viewport, font_metrics, depth + 1))
            .fold(0.0_f32, f32::max);
        block_max + extra
    }

    /// [§ 9.2 Controlling box generation](https://www.w3.org/TR/CSS2/visuren.html#box-gen)
    ///
    /// "The display property, determines the type of box or boxes that
    /// are generated for an element."
    ///
    /// `image_dimensions` maps `NodeId` to (width, height) for replaced
    /// elements like `<img>` whose intrinsic size was resolved externally.
    #[must_use]
    #[allow(clippy::implicit_hasher)]
    pub fn build_layout_tree(
        tree: &DomTree,
        styles: &HashMap<NodeId, ComputedStyle>,
        node_id: NodeId,
        image_dimensions: &HashMap<NodeId, (f32, f32)>,
    ) -> Option<Self> {
        let node = tree.get(node_id)?;

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
                    if let Some(child_box) =
                        Self::build_layout_tree(tree, styles, child_id, image_dimensions)
                    {
                        children.push(child_box);
                    }
                }
                Some(Self {
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
                    font_size: 16.0,
                    color: ColorValue::BLACK,
                    text_align: TextAlign::default(),
                    font_weight: 400,
                    font_style: FontStyle::Normal,
                    line_boxes: Vec::new(),
                    collapsed_margin_top: None,
                    collapsed_margin_bottom: None,
                    is_replaced: false,
                    replaced_src: None,
                    intrinsic_width: None,
                    intrinsic_height: None,
                    flex_direction: "row".to_string(),
                    justify_content: "flex-start".to_string(),
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    flex_basis: None,
                    position_type: PositionType::Static,
                    offsets: BoxOffsets::default(),
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
                if let Some(s) = style
                    && s.display_none
                {
                    return None;
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
                    if let Some(child_box) =
                        Self::build_layout_tree(tree, styles, child_id, image_dimensions)
                    {
                        children.push(child_box);
                    }
                }

                // Extract style values from computed style
                // [§ 8 Box model](https://www.w3.org/TR/CSS2/box.html)
                let (margin, padding, border_width, width, height) =
                    Self::extract_box_style_values(style);

                // [§ 3.5 'font-size'](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
                //
                // Resolve font-size to pixels. Defaults to 16px ('medium').
                #[allow(clippy::cast_possible_truncation)]
                let font_size = style
                    .and_then(|s| s.font_size.as_ref())
                    .map_or(16.0, |fs| fs.to_px() as f32);

                // [§ 3.1 'color'](https://www.w3.org/TR/css-color-4/#the-color-property)
                //
                // "The initial value is implementation-dependent."
                // Most browsers default to black.
                let color = style
                    .and_then(|s| s.color.as_ref())
                    .cloned()
                    .unwrap_or(ColorValue::BLACK);

                // [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
                //
                // "This property describes how inline-level content of a block
                // container is aligned."
                // "Initial value: a nameless value that acts as 'left' if
                // 'direction' is 'ltr', 'right' if 'direction' is 'rtl'."
                let text_align = style
                    .and_then(|s| s.text_align.as_deref())
                    .map(|ta| match ta {
                        "right" => TextAlign::Right,
                        "center" => TextAlign::Center,
                        "justify" => TextAlign::Justify,
                        _ => TextAlign::Left,
                    })
                    .unwrap_or_default();

                // [§ 3.2 'font-weight'](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
                //
                // "This property specifies the weight of glyphs in the font."
                // 400 = normal, 700 = bold.
                let font_weight = style.and_then(|s| s.font_weight).unwrap_or(400);

                // [§ 3.3 'font-style'](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
                //
                // "The 'font-style' property allows italic or oblique faces to
                // be selected."
                let font_style = style
                    .and_then(|s| s.font_style.as_deref())
                    .map(FontStyle::from_css)
                    .unwrap_or_default();

                // [§ 5.1 'flex-direction'](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
                let flex_direction = style
                    .and_then(|s| s.flex_direction.clone())
                    .unwrap_or_else(|| "row".to_string());
                // [§ 8.2 'justify-content'](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
                let justify_content = style
                    .and_then(|s| s.justify_content.clone())
                    .unwrap_or_else(|| "flex-start".to_string());
                // [§ 7.2 'flex-grow'](https://www.w3.org/TR/css-flexbox-1/#flex-grow-property)
                let flex_grow = style.and_then(|s| s.flex_grow).unwrap_or(0.0);
                // [§ 7.3 'flex-shrink'](https://www.w3.org/TR/css-flexbox-1/#flex-shrink-property)
                let flex_shrink = style.and_then(|s| s.flex_shrink).unwrap_or(1.0);
                // [§ 7.1 'flex-basis'](https://www.w3.org/TR/css-flexbox-1/#flex-basis-property)
                let flex_basis = style.and_then(|s| s.flex_basis);

                // [§ 9.3.1 'position'](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
                //
                // "Values: static | relative | absolute | fixed | sticky"
                // Initial: static
                let position_type = style
                    .and_then(|s| s.position.as_deref())
                    .map_or(PositionType::Static, |p| match p {
                        "relative" => PositionType::Relative,
                        "absolute" => PositionType::Absolute,
                        "fixed" => PositionType::Fixed,
                        "sticky" => PositionType::Sticky,
                        _ => PositionType::Static,
                    });

                // [§ 9.7 Relationships between 'display', 'position', and 'float'](https://www.w3.org/TR/CSS2/visuren.html#dis-pos-flo)
                //
                // "If 'position' has the value 'absolute' or 'fixed', the box
                // is absolutely positioned, 'float' is set to 'none', and display
                // is set according to the table below. The position of the box
                // will be determined by the 'top', 'right', 'bottom' and 'left'
                // properties and the box's containing block."
                //
                // The table maps inline → block (and inline-* → block-*).
                let display =
                    if matches!(position_type, PositionType::Absolute | PositionType::Fixed)
                        && display.outer == OuterDisplayType::Inline
                    {
                        DisplayValue::block()
                    } else {
                        display
                    };

                // [§ 9.3.2 Box offsets](https://www.w3.org/TR/CSS2/visuren.html#position-props)
                //
                // "These properties specify offsets with respect to the box's
                // containing block."
                //
                // None in ComputedStyle means property not set (treated as 'auto').
                // AutoLength::Auto also means 'auto'. Both map to None in BoxOffsets.
                // Length values are resolved to px during cascade.
                #[allow(clippy::cast_possible_truncation)]
                let offsets = BoxOffsets {
                    top: style.and_then(|s| s.top.as_ref()).and_then(|al| match al {
                        AutoLength::Auto => None,
                        AutoLength::Length(l) => Some(l.to_px() as f32),
                    }),
                    right: style.and_then(|s| s.right.as_ref()).and_then(|al| match al {
                        AutoLength::Auto => None,
                        AutoLength::Length(l) => Some(l.to_px() as f32),
                    }),
                    bottom: style.and_then(|s| s.bottom.as_ref()).and_then(|al| match al {
                        AutoLength::Auto => None,
                        AutoLength::Length(l) => Some(l.to_px() as f32),
                    }),
                    left: style.and_then(|s| s.left.as_ref()).and_then(|al| match al {
                        AutoLength::Auto => None,
                        AutoLength::Length(l) => Some(l.to_px() as f32),
                    }),
                };

                // [§ 10.3.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
                //
                // Detect replaced elements (e.g., <img>) and record their
                // intrinsic dimensions and src attribute for layout and paint.
                let (is_replaced, replaced_src, intrinsic_width, intrinsic_height) = if tag == "img"
                {
                    let src = data.attrs.get("src").cloned();
                    let dims = image_dimensions.get(&node_id);
                    (
                        src.is_some(),
                        src,
                        dims.map(|(w, _)| *w),
                        dims.map(|(_, h)| *h),
                    )
                } else {
                    (false, None, None, None)
                };

                Some(Self {
                    box_type: BoxType::Principal(node_id),
                    dimensions: BoxDimensions::default(),
                    display,
                    children,
                    margin,
                    padding,
                    border_width,
                    width,
                    height,
                    font_size,
                    color,
                    text_align,
                    font_weight,
                    font_style,
                    line_boxes: Vec::new(),
                    collapsed_margin_top: None,
                    collapsed_margin_bottom: None,
                    is_replaced,
                    replaced_src,
                    intrinsic_width,
                    intrinsic_height,
                    flex_direction,
                    justify_content,
                    flex_grow,
                    flex_shrink,
                    flex_basis,
                    position_type,
                    offsets,
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
                Some(Self {
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
                    // [§ 4 Inheritance](https://www.w3.org/TR/css-cascade-4/#inheriting)
                    //
                    // Text nodes inherit font-size and color from their parent.
                    // These defaults are overridden during inline layout by the
                    // parent's resolved values.
                    font_size: 16.0,
                    color: ColorValue::BLACK,
                    text_align: TextAlign::default(),
                    font_weight: 400,
                    font_style: FontStyle::Normal,
                    line_boxes: Vec::new(),
                    collapsed_margin_top: None,
                    collapsed_margin_bottom: None,
                    is_replaced: false,
                    replaced_src: None,
                    intrinsic_width: None,
                    intrinsic_height: None,
                    flex_direction: "row".to_string(),
                    justify_content: "flex-start".to_string(),
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    flex_basis: None,
                    position_type: PositionType::Static,
                    offsets: BoxOffsets::default(),
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
    /// Returns (margin, padding, `border_width`, width, height) as unresolved values.
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
            top: s.margin_top,
            right: s.margin_right,
            bottom: s.margin_bottom,
            left: s.margin_left,
        };

        // [§ 8.4 Padding properties](https://www.w3.org/TR/CSS2/box.html#padding-properties)
        //
        // "If the padding property is not set, the padding is 0."
        //
        // Store unresolved LengthValue values. Resolution happens during layout.
        let padding = UnresolvedEdgeSizes {
            top: s.padding_top,
            right: s.padding_right,
            bottom: s.padding_bottom,
            left: s.padding_left,
        };

        // [§ 8.5 Border properties](https://www.w3.org/TR/CSS2/box.html#border-properties)
        //
        // "The initial value of border width is 'medium' (implementation-defined)."
        //
        // Extract the width LengthValue from BorderValue. Resolution happens during layout.
        let border_width = UnresolvedEdgeSizes {
            top: s.border_top.as_ref().map(|b| b.width),
            right: s.border_right.as_ref().map(|b| b.width),
            bottom: s.border_bottom.as_ref().map(|b| b.width),
            left: s.border_left.as_ref().map(|b| b.width),
        };

        // [§ 10.2 Content width](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
        //
        // "This property specifies the content width of boxes."
        // None means 'auto' - width is calculated during layout.
        let width = s.width;

        // [§ 10.5 Content height](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
        //
        // "This property specifies the content height of boxes."
        // None means 'auto' - height depends on content.
        let height = s.height;

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
    pub fn layout(
        &mut self,
        containing_block: Rect,
        viewport: Rect,
        font_metrics: &dyn FontMetrics,
    ) {
        #[cfg(feature = "layout-trace")]
        let _depth = {
            let depth = LAYOUT_DEPTH.with(|d| {
                let current = d.get();
                d.set(current + 1);
                current
            });
            let stack_marker: u8 = 0;
            let stack_addr = &stack_marker as *const u8 as usize;
            eprintln!(
                "[LAYOUT DEPTH] depth={depth} box={:?} display={:?}/{:?} children={} stack_addr=0x{stack_addr:x}",
                self.box_type,
                self.display.outer,
                self.display.inner,
                self.children.len()
            );
            // Guard struct decrements depth counter on all return paths.
            struct DepthGuard;
            impl Drop for DepthGuard {
                fn drop(&mut self) {
                    LAYOUT_DEPTH.with(|d| d.set(d.get() - 1));
                }
            }
            DepthGuard
        };

        // [§ 10.3.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
        //
        // "A replaced element is an element whose content is outside the scope
        // of the CSS formatting model."
        //
        // Replaced elements use their own sizing algorithm instead of the
        // normal block/inline layout dispatch.
        if self.is_replaced {
            self.layout_replaced(containing_block, viewport);
        } else if self.display.inner == InnerDisplayType::Flex {
            // [§ 9 Flex Layout Algorithm](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
            //
            // Check inner display type — flex containers use their own algorithm.
            super::flex::layout_flex(self, containing_block, viewport, font_metrics);
        } else {
            match self.display.outer {
                OuterDisplayType::Block => {
                    self.layout_block(containing_block, viewport, font_metrics);
                }
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
                    self.layout_block(containing_block, viewport, font_metrics);
                }
                OuterDisplayType::RunIn => {
                    // [§ 9.2.3 Run-in boxes](https://www.w3.org/TR/CSS2/visuren.html#run-in)
                    todo!("Run-in layout not yet implemented")
                }
            }
        }

        // [§ 9.4.3 Relative positioning](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
        //
        // "Once a box has been laid out according to the normal flow, it may be
        // shifted relative to its normal position."
        //
        // Applied after all normal-flow layout is complete, so the offset
        // does not affect sibling or child positioning.
        if self.position_type == PositionType::Relative {
            PositionedLayout::layout_relative(&mut self.dimensions, &self.offsets);
        }
    }

    /// [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///
    /// Layout algorithm for block-level boxes in normal flow.
    fn layout_block(
        &mut self,
        containing_block: Rect,
        viewport: Rect,
        font_metrics: &dyn FontMetrics,
    ) {
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

        // STEP 3: Generate anonymous block boxes for mixed content.
        // [§ 9.2.1.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
        //
        // "When an inline box contains an in-flow block-level box, the inline
        // box... is broken around the block-level box... The line boxes before
        // the break and after the break are enclosed in anonymous block boxes."
        #[cfg(feature = "layout-trace")]
        eprintln!(
            "[BLOCK STEP3] generating anon boxes for {:?}, {} children before",
            self.box_type,
            self.children.len()
        );
        self.generate_anonymous_boxes();
        #[cfg(feature = "layout-trace")]
        eprintln!(
            "[BLOCK STEP3] after anon boxes: {} children, all_inline={}",
            self.children.len(),
            self.all_children_inline()
        );

        // STEP 4: Layout children.
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // If all children are inline-level, establish an inline formatting
        // context. Otherwise, use block formatting context.
        if self.all_children_inline() && !self.children.is_empty() {
            #[cfg(feature = "layout-trace")]
            eprintln!(
                "[BLOCK STEP4] layout_inline_children for {:?}",
                self.box_type
            );
            self.layout_inline_children(viewport, font_metrics);
        } else {
            #[cfg(feature = "layout-trace")]
            eprintln!(
                "[BLOCK STEP4] layout_block_children for {:?}, {} children",
                self.box_type,
                self.children.len()
            );
            self.layout_block_children(viewport, font_metrics);
        }

        // STEP 5: Calculate height
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

        // STEP 6: Layout absolutely positioned children.
        // [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
        //
        // "In the absolute positioning model, a box is removed from the
        // normal flow entirely and assigned a position with respect to a
        // containing block."
        //
        // Absolute children are positioned relative to this box's padding
        // box (if this box is positioned), or passed through to the next
        // positioned ancestor.
        self.layout_absolute_children(viewport, font_metrics);
    }

    /// [§ 10.3.3 Block-level, non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    ///
    /// Calculate the width of a block-level box.
    pub(crate) fn calculate_block_width(&mut self, containing_block: Rect, viewport: Rect) {
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
    pub(crate) fn calculate_block_position(&mut self, containing_block: Rect, viewport: Rect) {
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
    pub(crate) fn layout_block_children(&mut self, viewport: Rect, font_metrics: &dyn FontMetrics) {
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

        // STEP 3: Layout each child with margin collapsing.
        // [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
        //
        // "Vertical margins between adjacent block-level boxes in a block
        // formatting context collapse."
        //
        // "When two or more margins collapse, the resulting margin width is the
        // maximum of the collapsing margins' widths."
        //
        // Track the previous sibling's bottom margin so we can collapse it
        // with the current sibling's top margin.
        let mut prev_margin_bottom: Option<f32> = None;

        // [§ 8.3.1 Parent-child margin collapsing](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
        //
        // "The top margin of an in-flow block element collapses with its
        // first in-flow block-level child's top margin value if the element
        // has no top border, no top padding, and the child has no clearance."
        //
        // Pre-compute the condition; dimensions.border/padding are already
        // resolved by calculate_block_position() before this method runs.
        let no_top_separator =
            self.dimensions.border.top == 0.0 && self.dimensions.padding.top == 0.0;
        let parent_margin_top = self.dimensions.margin.top;
        let child_count = self.children.len();

        // Track whether we've seen the first in-flow child for parent-child
        // top margin collapsing purposes.
        let mut first_inflow = true;

        for i in 0..child_count {
            let child = &mut self.children[i];

            // [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
            //
            // "In the absolute positioning model, a box is removed from the
            // normal flow entirely."
            //
            // Absolute and fixed children do not participate in normal flow:
            // they are skipped during block layout and positioned later in
            // layout_absolute_children().
            if matches!(
                child.position_type,
                PositionType::Absolute | PositionType::Fixed
            ) {
                continue;
            }

            // STEP 3a: Parent-first-child top margin collapsing.
            //
            // [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
            //
            // When this is the first child and the parent has no top
            // border/padding, the child's top margin collapses with the
            // parent's top margin. The parent already occupies space for
            // its own margin-top; we pull current_y up by the child's
            // margin-top so that calculate_block_position() (which adds
            // child_mt) places the child flush at the parent's content top.
            // The parent's effective margin becomes the collapsed value.
            if first_inflow && no_top_separator && child.display.outer == OuterDisplayType::Block {
                let child_mt = child.margin.resolve(viewport).top.to_px_or(0.0);
                current_y -= child_mt;
                self.collapsed_margin_top = Some(collapse_two_margins(parent_margin_top, child_mt));
            }
            first_inflow = false;

            // STEP 3b: Collapse margins between adjacent siblings.
            //
            // [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
            //
            // "Two margins are adjoining if and only if:
            //   - both belong to in-flow block-level boxes that participate
            //     in the same block formatting context
            //   - no line boxes, no clearance, no padding and no border
            //     separate them"
            //
            // Pre-resolve the child's margin-top from the unresolved value.
            // This is safe because UnresolvedAutoEdgeSizes::resolve() is a
            // pure function of the viewport dimensions — identical to what
            // calculate_block_position() will compute internally.
            if let Some(prev_mb) = prev_margin_bottom {
                let child_mt = child.margin.resolve(viewport).top.to_px_or(0.0);
                let collapsed = collapse_two_margins(prev_mb, child_mt);
                // current_y already includes the previous child's margin-bottom
                // (from margin_box().height). The child will add its own
                // margin-top during calculate_block_position(). Without
                // collapsing the total gap would be prev_mb + child_mt.
                // We want the gap to be `collapsed`, so subtract the overlap.
                let overlap = prev_mb + child_mt - collapsed;
                current_y -= overlap;
            }

            // STEP 3c: Handle empty box self-collapsing.
            //
            // [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
            //
            // "A box's own margins collapse if the 'min-height' property is
            // computed as zero, the 'height' property is computed as zero or
            // 'auto', it does not establish a new block formatting context,
            // and it contains no in-flow content."
            //
            // An empty box takes up zero content height; its top and bottom
            // margins collapse into a single margin that participates in
            // sibling collapsing with its neighbours.
            if child.is_empty_collapsible_box() {
                let child_margin_top = child.margin.resolve(viewport).top.to_px_or(0.0);
                let child_margin_bottom = child.margin.resolve(viewport).bottom.to_px_or(0.0);
                let self_collapsed = collapse_two_margins(child_margin_top, child_margin_bottom);

                // Lay out the child so its dimensions are resolved (even
                // though it has zero content).
                let child_containing_block = Rect {
                    x: content_box.x,
                    y: current_y,
                    width: content_box.width,
                    height: f32::MAX,
                };
                child.layout(child_containing_block, viewport, font_metrics);

                // The empty box's self-collapsed margin merges with the
                // accumulated prev_margin_bottom for subsequent sibling
                // collapsing.
                prev_margin_bottom = Some(prev_margin_bottom.map_or(self_collapsed, |prev_mb| {
                    collapse_two_margins(prev_mb, self_collapsed)
                }));
                continue;
            }

            // STEP 3d: Create containing block and lay out the child.
            let child_containing_block = Rect {
                x: content_box.x,
                y: current_y,
                width: content_box.width,
                height: f32::MAX, // Height is unconstrained for normal flow
            };

            child.layout(child_containing_block, viewport, font_metrics);

            // STEP 4: Advance the Y position.
            // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
            //
            // "The vertical distance between two sibling boxes is determined by the
            // 'margin' properties."
            current_y += child.dimensions.margin_box().height;
            prev_margin_bottom = Some(child.effective_margin_bottom());
        }

        // STEP 5: Parent-last-child bottom margin collapsing.
        // [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
        //
        // "The bottom margin of an in-flow block-level element collapses
        // with the bottom margin of its last in-flow block-level child if
        // the element has no bottom padding and no bottom border and the
        // child's bottom margin does not collapse through with a top margin
        // that has clearance."
        let no_bottom_separator =
            self.dimensions.border.bottom == 0.0 && self.dimensions.padding.bottom == 0.0;
        // Find the last in-flow child (skip absolute/fixed).
        let last_inflow = self.children.iter().rev().find(|c| {
            !matches!(
                c.position_type,
                PositionType::Absolute | PositionType::Fixed
            )
        });
        if no_bottom_separator
            && self.height.is_none()
            && let Some(last) = last_inflow
            && last.display.outer == OuterDisplayType::Block
        {
            let parent_margin_bottom = self.dimensions.margin.bottom;
            let last_child_mb = last.effective_margin_bottom();
            self.collapsed_margin_bottom =
                Some(collapse_two_margins(parent_margin_bottom, last_child_mb));
        }
    }

    /// [§ 10.6.3 Block-level, non-replaced elements in normal flow when 'overflow' computes to 'visible'](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
    ///
    /// Calculate the height of a block-level box.
    ///
    /// "If 'height' is 'auto', the height depends on whether the element has
    /// any block-level children and whether it has padding or borders."
    pub(crate) fn calculate_block_height(&mut self, viewport: Rect, font_metrics: &dyn FontMetrics) {
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
            #[allow(clippy::cast_possible_truncation)]
            {
                self.dimensions.content.height = l
                    .to_px_with_viewport(f64::from(viewport.width), f64::from(viewport.height))
                    as f32;
            }
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
        if let BoxType::AnonymousInline(ref text) = self.box_type
            && !text.trim().is_empty()
        {
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

            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            {
                self.dimensions.content.height = (line_count as f32) * line_height;
            }
            return;
        }

        // STEP 3: Handle inline formatting context.
        // [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
        //
        // "...the height is the distance between the top content edge and
        // the bottom edge of the last line box, if the box establishes an
        // inline formatting context with one or more lines"
        //
        // If this box has line_boxes, the height was already set correctly
        // by layout_inline_children(). Don't overwrite it.
        if !self.line_boxes.is_empty() {
            return;
        }

        // STEP 4: Calculate 'auto' height from block-level children.
        // [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
        //
        // "If 'height' is 'auto', the height depends on whether the element
        // has any block-level children..."
        //
        // "...the height is the distance between the top content edge and...
        // the bottom edge of the bottom (possibly collapsed) margin of its
        // last in-flow child"
        //
        // Compute height from the last child's actual position rather than
        // summing margin_box heights. This correctly accounts for collapsed
        // margins between siblings (which reduce the effective spacing).
        // Use the last in-flow child (skip absolute/fixed) for auto height.
        // [§ 9.3](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
        //
        // "In the absolute positioning model, a box is removed from the
        // normal flow entirely." — absolute children do not contribute
        // to the parent's auto height.
        let last_inflow = self.children.iter().rev().find(|c| {
            !matches!(
                c.position_type,
                PositionType::Absolute | PositionType::Fixed
            )
        });
        if let Some(last) = last_inflow {
            let last_mb = last.dimensions.margin_box();
            let mut height = (last_mb.y + last_mb.height) - self.dimensions.content.y;

            // [§ 8.3.1 Parent-child bottom margin collapsing](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
            //
            // When the last child's bottom margin collapses with the
            // parent's bottom margin, it is no longer part of the parent's
            // content height — it becomes part of the parent's own margin.
            // Exclude it from the auto height calculation.
            if self.collapsed_margin_bottom.is_some() {
                height -= last.effective_margin_bottom();
            }

            self.dimensions.content.height = height;
        }
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
    /// STEP 1: Get the tentative used width (already computed by `calculate_block_width`)
    ///
    /// ```text
    /// let tentative_width = self.dimensions.content.width;
    /// ```
    ///
    /// STEP 2: Apply max-width constraint
    ///
    /// ```text
    /// if let Some(max_width) = self.max_width {
    ///     let max_px = max_width.resolve(viewport);
    ///     if tentative_width > max_px {
    ///         // Re-run width calculation with max_width as the width
    ///         self.dimensions.content.width = max_px;
    ///         // Re-solve margin equation with new width
    ///     }
    /// }
    /// ```
    ///
    /// STEP 3: Apply min-width constraint
    ///
    /// ```text
    /// if let Some(min_width) = self.min_width {
    ///     let min_px = min_width.resolve(viewport);
    ///     if self.dimensions.content.width < min_px {
    ///         self.dimensions.content.width = min_px;
    ///         // Re-solve margin equation with new width
    ///     }
    /// }
    /// ```
    fn apply_min_max_width(&self, _containing_block: Rect, _viewport: Rect) {
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
    /// STEP 1: Get the tentative used height (already computed by `calculate_block_height`)
    ///
    /// ```text
    /// let tentative_height = self.dimensions.content.height;
    /// ```
    ///
    /// STEP 2: Apply max-height constraint
    ///
    /// ```text
    /// if let Some(max_height) = self.max_height {
    ///     let max_px = max_height.resolve(viewport);
    ///     if tentative_height > max_px {
    ///         self.dimensions.content.height = max_px;
    ///     }
    /// }
    /// ```
    ///
    /// STEP 3: Apply min-height constraint
    ///
    /// ```text
    /// if let Some(min_height) = self.min_height {
    ///     let min_px = min_height.resolve(viewport);
    ///     if self.dimensions.content.height < min_px {
    ///         self.dimensions.content.height = min_px;
    ///     }
    /// }
    /// ```
    fn apply_min_max_height(&self, _viewport: Rect) {
        todo!("Apply min-height/max-height constraints per CSS 2.1 § 10.7")
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
    pub fn generate_anonymous_boxes(&mut self) {
        // STEP 0: Flatten block-in-inline.
        // [§ 9.2.1.1](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
        //
        // If any inline child contains a block-level descendant, promote those
        // descendants before the mixed-content check so the existing wrapping
        // logic handles the result correctly.
        self.flatten_block_in_inline();

        // STEP 1: Check if in-flow children are mixed (both block and inline).
        // [§ 9.3](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
        //
        // Absolute/fixed children are out of flow and do not participate
        // in the inline/block content classification.
        let is_inflow =
            |c: &Self| !matches!(c.position_type, PositionType::Absolute | PositionType::Fixed);
        let has_block_children = self
            .children
            .iter()
            .filter(|c| is_inflow(c))
            .any(|c| c.display.outer == OuterDisplayType::Block);
        let has_inline_children = self
            .children
            .iter()
            .filter(|c| is_inflow(c))
            .any(|c| c.display.outer == OuterDisplayType::Inline);

        if !(has_block_children && has_inline_children) {
            return; // No mixed content, no anonymous boxes needed
        }

        // STEP 2: Group consecutive inline children into anonymous block boxes.
        //
        // Walk children, accumulating runs of inline boxes.
        // When a block child is encountered:
        //   - Wrap the accumulated inline run in an AnonymousBlock
        //   - Add the block child as-is
        //   - Start a new inline run
        // After the loop, wrap any remaining inline run.
        let mut new_children: Vec<Self> = Vec::new();
        let mut inline_run: Vec<Self> = Vec::new();

        for child in std::mem::take(&mut self.children) {
            if child.display.outer == OuterDisplayType::Block {
                // Flush any accumulated inline run into an anonymous block.
                if !inline_run.is_empty() {
                    new_children.push(Self::wrap_in_anonymous_block(inline_run));
                    inline_run = Vec::new();
                }
                new_children.push(child);
            } else {
                // Inline-level child — accumulate into the current run.
                inline_run.push(child);
            }
        }

        // Flush any trailing inline run.
        if !inline_run.is_empty() {
            new_children.push(Self::wrap_in_anonymous_block(inline_run));
        }

        // STEP 3: Replace self.children with the new list.
        self.children = new_children;
    }

    /// Wrap a run of inline-level children in an anonymous block box.
    ///
    /// [§ 9.2.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
    ///
    /// "Anonymous block boxes are generated to wrap inline-level content
    /// that appears alongside block-level boxes inside a block container."
    fn wrap_in_anonymous_block(children: Vec<Self>) -> Self {
        Self {
            box_type: BoxType::AnonymousBlock,
            display: DisplayValue::block(),
            dimensions: BoxDimensions::default(),
            children,
            margin: UnresolvedAutoEdgeSizes::default(),
            padding: UnresolvedEdgeSizes::default(),
            border_width: UnresolvedEdgeSizes::default(),
            width: None,
            height: None,
            font_size: 16.0,
            color: ColorValue::BLACK,
            text_align: TextAlign::default(),
            font_weight: 400,
            font_style: FontStyle::Normal,
            line_boxes: Vec::new(),
            collapsed_margin_top: None,
            collapsed_margin_bottom: None,
            is_replaced: false,
            replaced_src: None,
            intrinsic_width: None,
            intrinsic_height: None,
            flex_direction: "row".to_string(),
            justify_content: "flex-start".to_string(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            position_type: PositionType::Static,
            offsets: BoxOffsets::default(),
        }
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
    pub(crate) fn layout_inline_children(&mut self, viewport: Rect, font_metrics: &dyn FontMetrics) {
        // STEP 1: Create an InlineLayout context.
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "The width of a line box is determined by a containing block."
        let content_rect = self.dimensions.content_box();
        let mut inline_layout = InlineLayout::new(
            self.dimensions.content.width,
            self.dimensions.content.y,
            self.text_align,
        );

        // STEP 2: Recursively add all inline content to the inline layout.
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "In an inline formatting context, boxes are laid out horizontally,
        // one after the other, beginning at the top of a containing block.
        // Horizontal margins, borders, and padding are respected between
        // these boxes."
        //
        // [§ 4 Inheritance](https://www.w3.org/TR/css-cascade-4/#inheriting)
        //
        // Font-size and color are inherited properties. This box's resolved
        // font_size and color are passed as the inherited values for its
        // inline children. Principal inline children will use their own
        // resolved values when recursing.
        layout_inline_content(
            &mut self.children,
            &mut inline_layout,
            self.font_size,
            &self.color,
            self.font_weight,
            self.font_style,
            viewport,
            font_metrics,
            content_rect,
        );

        // STEP 3: Finalize the last line.
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // Any remaining fragments on the current line are flushed into a
        // final line box.
        inline_layout.finish_line();

        // STEP 4: Set content height.
        // [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
        //
        // "If 'height' is 'auto'... the height is the distance between the
        // top content edge and the bottom edge of the last line box, if the
        // box establishes an inline formatting context with one or more lines."
        //
        // When block children are interspersed (per § 9.2.1.1), current_y
        // tracks the full height including both line boxes and block
        // interruptions.
        self.dimensions.content.height = inline_layout.current_y - self.dimensions.content.y;

        // STEP 5: Store the line boxes for painting.
        //
        // The painter reads from line_boxes to emit DrawText commands at
        // the correct positions, rather than reading from children's
        // dimensions (which are not set during inline layout).
        self.line_boxes = inline_layout.line_boxes;
    }

    /// [§ 10.3.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
    /// [§ 10.6.2 Inline, replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-height)
    ///
    /// "A replaced element is an element whose content is outside the scope of
    /// the CSS formatting model, such as an image, embedded document, or applet."
    ///
    /// Layout a replaced element (e.g., `<img>`) using its intrinsic dimensions.
    fn layout_replaced(&mut self, containing_block: Rect, viewport: Rect) {
        // STEP 1: Resolve padding, border, and margin.
        let resolved_padding = self.padding.resolve(viewport);
        let resolved_border = self.border_width.resolve(viewport);
        let resolved_margin = self.margin.resolve(viewport);

        self.dimensions.padding.top = resolved_padding.top;
        self.dimensions.padding.bottom = resolved_padding.bottom;
        self.dimensions.padding.left = resolved_padding.left;
        self.dimensions.padding.right = resolved_padding.right;

        self.dimensions.border.top = resolved_border.top;
        self.dimensions.border.bottom = resolved_border.bottom;
        self.dimensions.border.left = resolved_border.left;
        self.dimensions.border.right = resolved_border.right;

        self.dimensions.margin.top = resolved_margin.top.to_px_or(0.0);
        self.dimensions.margin.bottom = resolved_margin.bottom.to_px_or(0.0);
        self.dimensions.margin.left = resolved_margin.left.to_px_or(0.0);
        self.dimensions.margin.right = resolved_margin.right.to_px_or(0.0);

        // STEP 2: Compute intrinsic ratio.
        let intrinsic_ratio = match (self.intrinsic_width, self.intrinsic_height) {
            (Some(w), Some(h)) if h > 0.0 => Some(w / h),
            _ => None,
        };

        // STEP 3: Resolve width.
        // [§ 10.3.2](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width)
        //
        // "If 'width' has a computed value of 'auto', and the element has an
        // intrinsic width, then that intrinsic width is the used value of 'width'."
        let width_is_auto = matches!(self.width, None | Some(AutoLength::Auto));
        let height_is_auto = matches!(self.height, None | Some(AutoLength::Auto));

        let used_width = if width_is_auto {
            if let Some(iw) = self.intrinsic_width {
                iw
            } else if let (Some(ratio), false) = (intrinsic_ratio, height_is_auto) {
                // "If 'width' has a computed value of 'auto', but none of the
                // conditions above are met, then the used value of 'width'
                // becomes... height * ratio"
                let h = self.height.as_ref().map_or(150.0, |al| {
                    UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).to_px_or(150.0)
                });
                h * ratio
            } else {
                // [§ 10.3.2] Fallback: 300px
                300.0
            }
        } else {
            self.width.as_ref().map_or(300.0, |al| {
                UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).to_px_or(300.0)
            })
        };

        // STEP 4: Resolve height.
        // [§ 10.6.2](https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-height)
        //
        // "If 'height' has a computed value of 'auto', and the element has an
        // intrinsic height, then that intrinsic height is the used value of 'height'."
        let used_height = if height_is_auto {
            self.intrinsic_height.map_or_else(
                || {
                    // "Otherwise, if 'height' has a computed value of 'auto', and
                    // the element has an intrinsic ratio then the used value of
                    // 'height' is: used width / ratio"
                    intrinsic_ratio.map_or(
                        // [§ 10.6.2] Fallback: 150px
                        150.0,
                        |ratio| {
                            if ratio > 0.0 {
                                used_width / ratio
                            } else {
                                150.0
                            }
                        },
                    )
                },
                |ih| ih,
            )
        } else {
            self.height.as_ref().map_or(150.0, |al| {
                UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).to_px_or(150.0)
            })
        };

        self.dimensions.content.width = used_width;
        self.dimensions.content.height = used_height;

        // STEP 5: Position the content box.
        // [§ 9.4.1](https://www.w3.org/TR/CSS2/visuren.html#block-formatting)
        //
        // Same positioning as calculate_block_position.
        self.dimensions.content.x = containing_block.x
            + self.dimensions.margin.left
            + self.dimensions.border.left
            + self.dimensions.padding.left;

        self.dimensions.content.y = containing_block.y
            + self.dimensions.margin.top
            + self.dimensions.border.top
            + self.dimensions.padding.top;
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
    ///
    /// ```text
    /// let content_height = self.dimensions.content.height;
    /// let box_height = specified_height or auto;
    /// overflows = content_height > box_height
    /// ```
    ///
    /// STEP 2: Apply clipping if overflow is hidden/scroll/auto
    ///
    /// ```text
    /// // Create a clip rect matching the padding box
    /// clip_rect = self.dimensions.padding_box();
    /// ```
    ///
    /// STEP 3: Handle scrollable overflow
    ///
    /// [CSS Overflow Module Level 3 § 2](https://www.w3.org/TR/css-overflow-3/#overflow-properties)
    ///
    /// Calculate scrollable overflow region:
    ///
    /// "The scrollable overflow region is the union of the border boxes
    ///  of all descendants that extend beyond the padding edge."
    ///
    /// ```text
    /// scroll_width = max(child.margin_box().x + child.margin_box().width) - content.x
    /// scroll_height = max(child.margin_box().y + child.margin_box().height) - content.y
    /// ```
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
    ///
    /// ```text
    /// // Format content with no line breaks except explicit ones.
    /// preferred_width = max line width across all lines
    /// ```
    ///
    /// STEP 2: Calculate preferred minimum width
    ///
    /// ```text
    /// // Try all possible line breaks.
    /// preferred_min_width = max word width (or widest unbreakable unit)
    /// ```
    ///
    /// STEP 3: Calculate available width
    ///
    /// ```text
    /// available_width = containing_block.width - margins - borders - padding
    /// ```
    ///
    /// STEP 4: Compute shrink-to-fit width
    ///
    /// ```text
    /// shrink_to_fit = min(max(preferred_min_width, available_width), preferred_width)
    /// ```
    fn shrink_to_fit_width(&self, _containing_block: Rect, _viewport: Rect) -> f32 {
        todo!("Calculate shrink-to-fit width per CSS 2.1 § 10.3.5")
    }

    /// [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
    ///
    /// "In the absolute positioning model, a box is removed from the normal
    /// flow entirely and assigned a position with respect to a containing
    /// block."
    ///
    /// [§ 10.1 Definition of containing block](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
    ///
    /// "If the element has 'position: absolute', the containing block is
    /// established by the nearest ancestor with a 'position' of 'absolute',
    /// 'relative', or 'fixed', in the following way:
    ///   ... the containing block is formed by the padding edge of the
    ///   ancestor."
    ///
    /// v1 simplification: Uses the parent's padding box as the containing
    /// block. Full spec requires walking up to find the nearest positioned
    /// ancestor.
    pub(crate) fn layout_absolute_children(
        &mut self,
        viewport: Rect,
        font_metrics: &dyn FontMetrics,
    ) {
        // Collect indices of absolute/fixed children to avoid borrow issues.
        let abs_indices: Vec<usize> = self
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                matches!(
                    c.position_type,
                    PositionType::Absolute | PositionType::Fixed
                )
            })
            .map(|(i, _)| i)
            .collect();

        if abs_indices.is_empty() {
            return;
        }

        // The containing block for absolute children is this box's padding box.
        let padding_box = self.dimensions.padding_box();

        for idx in abs_indices {
            let child = &mut self.children[idx];

            // [§ 9.3.1 Fixed positioning](https://www.w3.org/TR/CSS2/visuren.html#fixed-positioning)
            //
            // "Fixed positioning is a subcategory of absolute positioning.
            // The only difference is that for a fixed positioned box, the
            // containing block is established by the viewport."
            let cb = if child.position_type == PositionType::Fixed {
                viewport
            } else {
                padding_box
            };

            PositionedLayout::layout_absolute(child, cb, viewport, font_metrics);
        }
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
    #[must_use]
    pub fn has_mixed_children(&self) -> bool {
        let mut has_block = false;
        let mut has_inline = false;
        for child in &self.children {
            match child.display.outer {
                OuterDisplayType::Block => has_block = true,
                OuterDisplayType::Inline => has_inline = true,
                OuterDisplayType::RunIn => {}
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
    #[must_use]
    pub fn all_children_inline(&self) -> bool {
        self.children
            .iter()
            // [§ 9.3](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
            //
            // Absolute/fixed children are out of flow — they do not affect
            // whether the parent establishes an inline or block formatting
            // context for its in-flow children.
            .filter(|c| {
                !matches!(
                    c.position_type,
                    PositionType::Absolute | PositionType::Fixed
                )
            })
            .all(|c| c.display.outer == OuterDisplayType::Inline)
    }

    /// Promote block-level descendants out of inline ancestors.
    ///
    /// [§ 9.2.1.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
    ///
    /// "When an inline box contains an in-flow block-level box, the inline
    /// box (and its inline ancestors within the same line box) are broken
    /// around the block-level box..."
    ///
    /// This pre-pass replaces each inline child that contains a block
    /// descendant with that child's own children (promoting them one level
    /// up). The loop repeats until no inline child has block descendants,
    /// handling nested inline wrappers (e.g. `<span><em><div>`).
    ///
    /// NOTE: The inline box's own styling (margin/border/padding) is lost
    /// during promotion. This is an acceptable trade-off — the content
    /// renders instead of crashing.
    fn flatten_block_in_inline(&mut self) {
        loop {
            let needs_flatten = self
                .children
                .iter()
                .any(|c| c.display.outer == OuterDisplayType::Inline && c.has_block_descendant());
            if !needs_flatten {
                break;
            }

            let old_children = std::mem::take(&mut self.children);
            for child in old_children {
                if child.display.outer == OuterDisplayType::Inline && child.has_block_descendant() {
                    // Replace the inline wrapper with its children.
                    self.children.extend(child.children);
                } else {
                    self.children.push(child);
                }
            }
        }
    }

    /// Returns true if any descendant (recursively) is block-level.
    ///
    /// [§ 9.2.1.1 Anonymous block boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-block-level)
    ///
    /// Used to detect the case where an inline box contains a block-level
    /// descendant, which requires splitting the inline box per the spec.
    fn has_block_descendant(&self) -> bool {
        self.children
            .iter()
            .any(|c| c.display.outer == OuterDisplayType::Block || c.has_block_descendant())
    }
}
