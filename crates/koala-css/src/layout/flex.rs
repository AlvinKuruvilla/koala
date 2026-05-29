//! CSS Flexbox Layout Algorithm.
//!
//! [§ 9 Flex Layout Algorithm](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
//!
//! This module implements CSS Flexbox:
//! - `flex-direction: row` (horizontal main axis)
//! - `flex-grow` / `flex-shrink` distribution (§ 9.7)
//! - `flex-basis` (definite length or auto)
//! - `flex` shorthand (§ 7)
//! - `flex-wrap` (§ 5.2) — single-line and multi-line
//! - `justify-content` (5 keywords)
//! - `align-items` / `align-self` cross-axis alignment (§ 8.3)
//! - No margin collapsing between flex items
//!
//! Not yet implemented: column direction, align-content, order, inline-flex.

use crate::style::{AutoLength, LengthValue};
use crate::style::computed::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};

use crate::style::values::PositionType;

use super::box_model::Rect;
use super::inline::FontMetrics;
use super::layout_box::LayoutBox;
use super::values::UnresolvedAutoEdgeSizes;

/// Direction of the flex container's main axis.
///
/// [§ 3 Flex Layout Box Model and Terminology](https://www.w3.org/TR/css-flexbox-1/#box-model)
///
/// "Each flex layout box has a main axis, the primary axis along which
/// flex items are laid out. The cross axis is perpendicular to the
/// main axis."
///
/// Derived from `FlexDirection` via [`MainAxis::from_flex_direction`].
/// The spec describes the entire flex algorithm in terms of "main" and
/// "cross" sizes / positions; this enum is the bridge that lets the
/// algorithm read like the spec while still operating on a physical
/// `Rect` underneath.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MainAxis {
    /// Main axis is horizontal (x, width); cross axis is vertical.
    Row,
    /// Main axis is vertical (y, height); cross axis is horizontal.
    Column,
}

impl MainAxis {
    /// Collapse `FlexDirection` to a `MainAxis`. The `*-reverse`
    /// variants flip item *order* along the main axis but the axis
    /// itself is unchanged, so they map to the same `MainAxis` as
    /// their non-reverse counterparts. The reversal itself is applied
    /// separately at the STEP 5 main-loop by iterating items in
    /// reverse when `main_reverse` is true.
    fn from_flex_direction(d: FlexDirection) -> Self {
        match d {
            FlexDirection::Row | FlexDirection::RowReverse => Self::Row,
            FlexDirection::Column | FlexDirection::ColumnReverse => Self::Column,
        }
    }

    /// Extent of `r` along the main axis.
    fn main_size(self, r: &Rect) -> f32 {
        match self {
            Self::Row => r.width,
            Self::Column => r.height,
        }
    }

    /// Extent of `r` along the cross axis.
    fn cross_size(self, r: &Rect) -> f32 {
        match self {
            Self::Row => r.height,
            Self::Column => r.width,
        }
    }

    /// Coordinate of `r`'s main-start edge (i.e. the smaller end of the
    /// main axis).
    fn main_start(self, r: &Rect) -> f32 {
        match self {
            Self::Row => r.x,
            Self::Column => r.y,
        }
    }

    /// Coordinate of `r`'s cross-start edge.
    fn cross_start(self, r: &Rect) -> f32 {
        match self {
            Self::Row => r.y,
            Self::Column => r.x,
        }
    }

    /// Sum of the two edges that contribute to the main-axis outer
    /// extent. Callers pass the four edges (top / right / bottom /
    /// left) of whichever resolved edge struct they have in hand —
    /// margin, border, or padding — because the three types don't
    /// share a single trait today.
    fn main_edge_sum(self, top: f32, right: f32, bottom: f32, left: f32) -> f32 {
        match self {
            Self::Row => left + right,
            Self::Column => top + bottom,
        }
    }

    /// Build a physical `Rect` from logical-axis coordinates. Lets a
    /// caller compute everything in (main, cross) terms and project
    /// to (x, y, width, height) at the boundary where a child's
    /// containing block is constructed.
    fn make_rect(
        self,
        main_start: f32,
        cross_start: f32,
        main_size: f32,
        cross_size: f32,
    ) -> Rect {
        match self {
            Self::Row => Rect {
                x: main_start,
                y: cross_start,
                width: main_size,
                height: cross_size,
            },
            Self::Column => Rect {
                x: cross_start,
                y: main_start,
                width: cross_size,
                height: main_size,
            },
        }
    }

    /// Set the child's main-axis size CSS property to `value` so the
    /// downstream block layout uses the flex-resolved size. For row
    /// direction this overrides `child.width`; for column it
    /// overrides `child.height`. `value` is interpreted as a content
    /// box length; the caller is responsible for adding back
    /// padding+border when `box-sizing: border-box`.
    fn set_main_size(self, child: &mut LayoutBox, value: f32) {
        let length = AutoLength::Length(LengthValue::Px(f64::from(value)));
        match self {
            Self::Row => child.width = Some(length),
            Self::Column => child.height = Some(length),
        }
    }
}

/// Per-item data collected during flex layout.
///
/// [§ 9.2 Line Length Determination](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
struct FlexItem {
    /// Index into `container.children`.
    index: usize,
    /// [§ 9.2 step 3](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
    /// The flex base size.
    base_size: f32,
    /// [§ 9.2 step 3E](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
    /// The hypothetical main size (`base_size` clamped by min/max, but we
    /// skip min/max for MVP so this equals `base_size`).
    hypothetical_size: f32,
    /// flex-grow factor.
    grow: f32,
    /// flex-shrink factor.
    shrink: f32,
    /// The resolved target main size after § 9.7.
    target_size: f32,
    /// Whether this item is frozen during the § 9.7 loop.
    frozen: bool,
    /// Sum of horizontal margin+border+padding (the "outer" contribution
    /// beyond the content box on the main axis).
    outer_main: f32,
}

/// Main entry point for flex layout.
///
/// [§ 9 Flex Layout Algorithm](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
///
/// This function is called from `LayoutBox::layout()` when the box has
/// `display.inner == InnerDisplayType::Flex`.
pub fn layout_flex(
    container: &mut LayoutBox,
    containing_block: Rect,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
    abs_cb: Rect,
) {
    #[cfg(feature = "layout-trace")]
    {
        let flex_stack_marker: u8 = 0;
        eprintln!(
            "[FLEX] layout_flex called, {} children, display={:?}/{:?}, stack=0x{:x}",
            container.children.len(),
            container.display.outer,
            container.display.inner,
            &flex_stack_marker as *const u8 as usize
        );
    }

    // STEP 1 (§ 9.2): Resolve container's own main size.
    //
    // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    //
    // Reuse the block width algorithm — the flex container is a block-level
    // box, so its own width is determined by the same constraint equation.
    container.calculate_block_width(containing_block, viewport);

    // STEP 2 (§ 9.2): Resolve container position.
    container.calculate_block_position(containing_block, viewport);

    // [§ 10.1 Definition of containing block](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
    //
    // If the flex container is positioned, its padding box becomes the
    // containing block for absolutely positioned descendants.
    let child_abs_cb = if container.is_positioned() {
        container.dimensions.padding_box()
    } else {
        abs_cb
    };

    let content_box = container.dimensions.content_box();
    let axis = MainAxis::from_flex_direction(container.flex_direction);
    // [§ 5.1 row-reverse / column-reverse](https://drafts.csswg.org/css-flexbox-1/#flex-direction-property)
    //
    // "Same as row [or column], except the main-start and main-end
    // directions are swapped." We keep `MainAxis` as the two-axis
    // choice and carry the reverse flag alongside so the STEP 5
    // main-loop can flip the item iteration order without
    // duplicating every match arm.
    let main_reverse = matches!(
        container.flex_direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse,
    );
    // STEP 1's `calculate_block_width` sets the container's `width` but
    // not its `height`; for column direction the main-axis extent is
    // therefore still 0 here. Real available_main is computed after
    // we've collected items, below.

    // STEP 3 (spec steps 1+3): Blockify children and determine flex base sizes.
    //
    // [§ 4 Flex Items](https://www.w3.org/TR/css-flexbox-1/#flex-items)
    //
    // "Each in-flow child of a flex container becomes a flex item, and each
    // contiguous sequence of child text runs is wrapped in an anonymous
    // block container flex item."
    //
    // All children — including AnonymousInline text nodes — are treated as
    // block-level flex items. No margin collapsing, no anonymous box wrapping.
    let child_count = container.children.len();
    let mut items: Vec<FlexItem> = Vec::with_capacity(child_count);

    for i in 0..child_count {
        let child = &container.children[i];

        // [§ 4.1 Absolutely-Positioned Flex Children](https://www.w3.org/TR/css-flexbox-1/#abspos-items)
        //
        // "An absolutely-positioned child of a flex container does not
        // participate in flex layout."
        if matches!(
            child.position_type,
            PositionType::Absolute | PositionType::Fixed
        ) {
            continue;
        }

        // Resolve the child's margin/border/padding to compute its outer
        // contribution on the main axis.
        let cb_width = container.dimensions.content.width;
        let resolved_padding = child.padding.resolve(viewport, cb_width);
        let resolved_border = child.border_width.resolve(viewport, cb_width);
        let resolved_margin = child.margin.resolve(viewport, cb_width);

        let outer_main = axis.main_edge_sum(
            resolved_margin.top.to_px_or(0.0),
            resolved_margin.right.to_px_or(0.0),
            resolved_margin.bottom.to_px_or(0.0),
            resolved_margin.left.to_px_or(0.0),
        ) + axis.main_edge_sum(
            resolved_border.top,
            resolved_border.right,
            resolved_border.bottom,
            resolved_border.left,
        ) + axis.main_edge_sum(
            resolved_padding.top,
            resolved_padding.right,
            resolved_padding.bottom,
            resolved_padding.left,
        );

        // [§ 9.2 step 3](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
        //
        // Determine flex base size:
        //   A. If flex-basis is a definite length, use it.
        //   B. If flex-basis is auto and the item has a definite main size
        //      (width for row, height for column), use that.
        //   C. Otherwise, use max-content size on the main axis.
        let mut base_size = child.flex_basis.as_ref().map_or_else(
            || flex_base_from_main_or_content(child, axis, viewport, cb_width, font_metrics),
            |fb| {
                let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(fb, viewport, cb_width);
                if resolved.is_auto() {
                    // flex-basis: auto — fall through to main-size or content sizing
                    flex_base_from_main_or_content(child, axis, viewport, cb_width, font_metrics)
                } else {
                    resolved.to_px_or(0.0)
                }
            },
        );

        // [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
        //
        // When box-sizing is border-box, the flex base size (from flex-basis
        // or the main-axis size property) includes padding and border.
        // Convert to content-box so the flex algorithm operates on content
        // sizes. `outer_main` already accounts for the surrounding
        // margin+border+padding; `base_size` should be content-only.
        if child.box_sizing_border_box {
            base_size -= axis.main_edge_sum(
                resolved_padding.top,
                resolved_padding.right,
                resolved_padding.bottom,
                resolved_padding.left,
            ) + axis.main_edge_sum(
                resolved_border.top,
                resolved_border.right,
                resolved_border.bottom,
                resolved_border.left,
            );
            base_size = base_size.max(0.0);
        }

        // [§ 9.2 step 3E](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
        //
        // "The hypothetical main size is the item's flex base size clamped
        // according to its used min and max main sizes."
        //
        // MVP: no min/max, so hypothetical_size == base_size.
        let hypothetical_size = base_size;

        items.push(FlexItem {
            index: i,
            base_size,
            hypothetical_size,
            grow: child.flex_grow,
            shrink: child.flex_shrink,
            target_size: 0.0,
            frozen: false,
            outer_main,
        });
    }

    // [§ 9.3 Determine the main size of the flex container](https://drafts.csswg.org/css-flexbox-1/#algo-main-container)
    //
    // "Determine the main size of the flex container using the rules
    // of the formatting context in which it participates. The
    // automatic block size of a block-level flex container is its
    // max-content size."
    //
    // [§ 9.9.1 Flex Container Intrinsic Main Sizes](https://drafts.csswg.org/css-flexbox-1/#intrinsic-main-sizes)
    //
    // "The max-content main size of a flex container is, theoretically,
    // the smallest size the flex container can take such that when
    // flex layout is run with that container size, each flex item ends
    // up at least as large as its max-content contribution, to the
    // extent allowed by the items' flexibility."
    //
    // STEP 1's `calculate_block_width` populated `content_box.width`
    // but not the height. For row direction the container's main size
    // *is* the width and `axis.main_size(&content_box)` returns the
    // correct value. For column with indefinite height we use the sum
    // of outer hypothetical main sizes — for our MVP (no min/max
    // clamping, items contribute their max-content directly) this
    // coincides with the spec's "max-content main size of a flex
    // container" so the freeze loop sees a non-zero `available_main`
    // and items aren't shrunk to zero.
    //
    // TODO: the `raw > 0.0` heuristic conflates "indefinite height"
    // with "explicitly height: 0" (which is definite per § 9.8).
    // Resolving `container.height` upfront would be the right fix
    // but it duplicates work currently done in STEP 7.
    let available_main = match axis {
        MainAxis::Row => axis.main_size(&content_box),
        MainAxis::Column => {
            let raw = axis.main_size(&content_box);
            if raw > 0.0 {
                raw
            } else {
                items
                    .iter()
                    .map(|item| item.hypothetical_size + item.outer_main)
                    .sum()
            }
        }
    };

    // STEP 4 (§ 9.3): Collect flex items into flex lines.
    //
    // [§ 9.3 Main Size Determination](https://www.w3.org/TR/css-flexbox-1/#algo-line-break)
    //
    // "If the flex container is single-line, collect all the flex items
    // into a single flex line."
    //
    // "Otherwise, starting from the first uncollected item, collect
    // consecutive items one by one until the first time that the next
    // collected item would not fit into the flex container's inner main
    // size (or until a forced break is encountered, see § 10 Fragmenting
    // Flex Layout). If the very first uncollected item wouldn't fit,
    // collect just it into the line."
    //
    // "For this step, the size of a flex item is its outer hypothetical
    // main size."
    //
    // "Repeat until all flex items have been collected into flex lines."
    #[cfg(feature = "layout-trace")]
    {
        let m: u8 = 0;
        eprintln!(
            "[FLEX] after step 3 (measures done), flex_wrap={:?}, stack=0x{:x}",
            container.flex_wrap,
            &m as *const u8 as usize
        );
    }

    let lines: Vec<Vec<usize>> = if container.flex_wrap == FlexWrap::Nowrap {
        // "If the flex container is single-line, collect all the flex items
        // into a single flex line."
        vec![(0..items.len()).collect()]
    } else {
        // "Starting from the first uncollected item, collect consecutive
        // items one by one until the first time that the next collected
        // item would not fit into the flex container's inner main size."
        let mut lines = Vec::new();
        let mut line: Vec<usize> = Vec::new();
        let mut line_main = 0.0_f32;

        for (i, item) in items.iter().enumerate() {
            // "For this step, the size of a flex item is its outer
            // hypothetical main size."
            let item_main = item.hypothetical_size + item.outer_main;

            // "If the very first uncollected item wouldn't fit, collect
            // just it into the line."
            // (The `!line.is_empty()` guard ensures we always accept at
            // least one item per line.)
            if !line.is_empty() && line_main + item_main > available_main {
                lines.push(line);
                line = Vec::new();
                line_main = 0.0;
            }
            line.push(i);
            line_main += item_main;
        }
        if !line.is_empty() {
            lines.push(line);
        }
        // "Repeat until all flex items have been collected into flex lines."
        lines
    };

    // STEP 5 (§ 9.7): Resolve flexible lengths for each line.
    // STEP 6 (§ 9.5): Determine cross sizes and lay out children.
    //
    // [§ 9.7 Resolving Flexible Lengths](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
    //
    // Each flex line is processed independently: resolve flexible lengths,
    // compute justify-content offsets, then layout each child with its
    // resolved target main size.
    //
    // `current_cross` walks the cross-axis line-by-line; `current_main`
    // walks the main-axis item-by-item within each line. For row
    // direction main = x, cross = y; for column it's the reverse.
    let available_cross = axis.cross_size(&content_box);
    let mut current_cross = axis.cross_start(&content_box);
    let mut line_cross_sizes: Vec<f32> = Vec::new();
    // The total main extent used by each line, including gaps. Needed
    // for column-direction containers where the container's used
    // main size (= height) is the sum of item main sizes plus gaps,
    // not the sum of line cross sizes the way it is for row.
    let mut line_main_extents: Vec<f32> = Vec::new();
    let mut item_to_line: Vec<usize> = vec![0; items.len()];

    for (line_idx, line_item_indices) in lines.iter().enumerate() {
        // Build a temporary sub-slice for resolve_flexible_lengths
        let mut line_items: Vec<FlexItem> = line_item_indices
            .iter()
            .map(|&i| FlexItem {
                index: items[i].index,
                base_size: items[i].base_size,
                hypothetical_size: items[i].hypothetical_size,
                grow: items[i].grow,
                shrink: items[i].shrink,
                target_size: 0.0,
                frozen: false,
                outer_main: items[i].outer_main,
            })
            .collect();

        for &i in line_item_indices {
            item_to_line[i] = line_idx;
        }

        // § 9.7: Resolve flexible lengths for this line.
        resolve_flexible_lengths(&mut line_items, available_main);

        // Compute justify-content offsets for this line.
        let total_target: f32 = line_items
            .iter()
            .map(|item| item.target_size + item.outer_main)
            .sum();
        let free_space = (available_main - total_target).max(0.0);
        let (initial_offset, gap) =
            compute_justify_offsets(container.justify_content, free_space, line_items.len());

        // Layout each child on this line. For `*-reverse` directions
        // (§ 5.1) we walk the items in reverse so the first item in
        // source order ends up at main-end and the last at main-start.
        let line_main_start = axis.main_start(&content_box) + initial_offset;
        let mut current_main = line_main_start;
        let mut line_cross_size = 0.0_f32;

        let line_item_refs: Vec<&FlexItem> = if main_reverse {
            line_items.iter().rev().collect()
        } else {
            line_items.iter().collect()
        };

        for (item_idx, &line_item) in line_item_refs.iter().enumerate() {
            let child = &mut container.children[line_item.index];

            // [§ 9.7](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
            //
            // "Set each item's used main size to its target main size."
            let main_size_for_layout = if child.box_sizing_border_box {
                let rp = child.padding.resolve(viewport, content_box.width);
                let rb = child.border_width.resolve(viewport, content_box.width);
                line_item.target_size
                    + axis.main_edge_sum(rp.top, rp.right, rp.bottom, rp.left)
                    + axis.main_edge_sum(rb.top, rb.right, rb.bottom, rb.left)
            } else {
                line_item.target_size
            };
            axis.set_main_size(child, main_size_for_layout);

            // Build the child's containing block in logical-axis terms.
            // Main size matches the flex-resolved target. Cross size
            // needs more care: for *column* direction the cross axis is
            // `width`, and CSS 2.1 § 10.3.3 RULE D (over-constrained
            // block) will silently absorb any mismatch between the
            // child's declared width and the CB width into
            // `margin-right`. If the child has an explicit cross size,
            // pass that value through as the CB cross size so RULE D
            // doesn't fire; if the child has `auto` cross size, fall
            // back to the container's cross so the child fills it
            // (subject to the later `align-items: stretch` pass).
            // Row direction sidesteps the symmetry because block height
            // has no equivalent auto-absorb rule.
            let child_cross_cb = match axis {
                MainAxis::Row => available_cross,
                MainAxis::Column => child
                    .width
                    .as_ref()
                    .and_then(|w| {
                        let resolved =
                            UnresolvedAutoEdgeSizes::resolve_auto_length(w, viewport, content_box.width);
                        (!resolved.is_auto()).then(|| resolved.to_px_or(0.0))
                    })
                    .unwrap_or(available_cross),
            };
            let child_containing = axis.make_rect(
                current_main,
                current_cross,
                line_item.target_size,
                child_cross_cb,
            );

            #[cfg(feature = "layout-trace")]
            eprintln!(
                "[FLEX] line {line_idx} item {item_idx}: laying out child, display={:?}/{:?}",
                child.display.outer, child.display.inner
            );
            child.layout(child_containing, viewport, font_metrics, child_abs_cb);

            current_main += axis.main_size(&child.dimensions.margin_box());
            if item_idx < line_items.len() - 1 {
                current_main += gap;
            }

            // Track the largest item on this line for cross-axis sizing
            let child_cross = axis.cross_size(&child.dimensions.margin_box());
            line_cross_size = line_cross_size.max(child_cross);
        }

        line_cross_sizes.push(line_cross_size);
        line_main_extents.push(current_main - line_main_start);
        current_cross += line_cross_size;
    }

    // STEP 7 (§ 9.9): Determine the flex container's used cross size.
    //
    // [§ 9.9 Cross Size Determination](https://www.w3.org/TR/css-flexbox-1/#algo-cross-container)
    //
    // "If the cross size property is a definite size, use that, clamped by
    // the used min and max cross sizes of the flex container."
    //
    // "Otherwise, use the sum of the flex lines' cross sizes, clamped by
    // the used min and max cross sizes of the flex container."
    //
    // For row direction the container's cross axis is `height`; for
    // column it's the container's main axis (`height`) that the flex
    // algorithm sizes from item content — the cross axis (`width`)
    // already came from the block-level width algorithm at STEP 1.
    //
    // [§ 10.5](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    //
    // "If the height of the containing block is not specified explicitly,
    // the percentage value is treated as 'auto'."
    let cb_height_is_auto = containing_block.height >= f32::MAX / 2.0;
    let height_is_pct_with_auto_cb = matches!(container.height, Some(AutoLength::Length(LengthValue::Percent(_))))
        && cb_height_is_auto;
    let auto_height_from_flex = match axis {
        // Row: container's height = sum of line cross sizes.
        MainAxis::Row => line_cross_sizes.iter().sum::<f32>(),
        // Column: container's height = the longest line's main extent.
        // For single-line containers (the common case) this is simply
        // the one line's total content height plus item gaps.
        MainAxis::Column => line_main_extents
            .iter()
            .copied()
            .fold(0.0_f32, f32::max),
    };
    if let Some(AutoLength::Length(ref l)) = container.height {
        if height_is_pct_with_auto_cb {
            // Percentage height with auto CB height → treat as auto.
            container.dimensions.content.height = auto_height_from_flex;
        } else {
            #[allow(clippy::cast_possible_truncation)]
            {
                container.dimensions.content.height = l.to_px_with_containing_block(
                    f64::from(containing_block.height),
                    f64::from(viewport.width),
                    f64::from(viewport.height),
                ) as f32;
            }
        }
    } else {
        container.dimensions.content.height = auto_height_from_flex;
    }

    // [§ 9.4 Cross Size Determination](https://drafts.csswg.org/css-flexbox-1/#algo-cross-line)
    //
    // "If the flex container is single-line and has a definite cross
    // size, the cross size of the flex line is the flex container's
    // inner cross size."
    //
    // For row this means a definite container `height` feeds back
    // into the single line; for column it's a definite container
    // `width`. Without this override, `align-items: center` (and
    // friends) on a column flex with `align-items: center; width:
    // 200px` would center items against the max-item-width rather
    // than the container width — wrong by spec.
    let container_cross_definite = match axis {
        MainAxis::Row => container.height.is_some(),
        MainAxis::Column => container.width.is_some(),
    };
    if lines.len() == 1 && container_cross_definite {
        line_cross_sizes[0] = axis.cross_size(&container.dimensions.content);
    }

    // STEP 8 (§ 9.6): Cross-axis alignment per line.
    //
    // [§ 8.3 'align-items'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
    //
    // "The align-items property sets the default alignment for all of the
    // flex container's items, including anonymous flex items."
    let container_align_items = container.align_items;

    for (i, item) in items.iter().enumerate() {
        let line_idx = item_to_line[i];
        let line_cross_size = line_cross_sizes[line_idx];
        let child = &mut container.children[item.index];

        let alignment = match child.align_self {
            AlignSelf::Auto => container_align_items,
            AlignSelf::FlexStart => AlignItems::FlexStart,
            AlignSelf::FlexEnd => AlignItems::FlexEnd,
            AlignSelf::Center => AlignItems::Center,
            AlignSelf::Baseline => AlignItems::Baseline,
            AlignSelf::Stretch => AlignItems::Stretch,
        };

        let child_margin_box_cross = axis.cross_size(&child.dimensions.margin_box());

        // Shift the child's content origin along the cross axis by
        // `delta`. For row the cross axis is y; for column it's x.
        let shift_cross = |child: &mut LayoutBox, delta: f32| match axis {
            MainAxis::Row => child.dimensions.content.y += delta,
            MainAxis::Column => child.dimensions.content.x += delta,
        };

        match alignment {
            AlignItems::FlexStart | AlignItems::Baseline => {}
            AlignItems::FlexEnd => {
                let offset = line_cross_size - child_margin_box_cross;
                shift_cross(child, offset);
            }
            AlignItems::Center => {
                let offset = (line_cross_size - child_margin_box_cross) / 2.0;
                shift_cross(child, offset);
            }
            AlignItems::Stretch => {
                // Stretch the child's content size along the cross
                // axis to fill the line minus its own cross-direction
                // margin+border+padding. For row this targets
                // `content.height` (subtracting top/bottom edges);
                // for column it targets `content.width` (subtracting
                // left/right edges).
                match axis {
                    MainAxis::Row => {
                        if child.height.is_none() {
                            let stretched_height = line_cross_size
                                - child.dimensions.margin.top
                                - child.dimensions.margin.bottom
                                - child.dimensions.border.top
                                - child.dimensions.border.bottom
                                - child.dimensions.padding.top
                                - child.dimensions.padding.bottom;
                            if stretched_height > child.dimensions.content.height {
                                child.dimensions.content.height = stretched_height;
                            }
                        }
                    }
                    MainAxis::Column => {
                        if child.width.is_none() {
                            let stretched_width = line_cross_size
                                - child.dimensions.margin.left
                                - child.dimensions.margin.right
                                - child.dimensions.border.left
                                - child.dimensions.border.right
                                - child.dimensions.padding.left
                                - child.dimensions.padding.right;
                            if stretched_width > child.dimensions.content.width {
                                child.dimensions.content.width = stretched_width;
                            }
                        }
                    }
                }
            }
        }
    }

    // STEP 9: Layout absolutely positioned children.
    container.layout_absolute_children(viewport, font_metrics, child_abs_cb);
}

/// Determine flex base size from the main-axis size property or
/// content measurement.
///
/// [§ 9.2 step 3](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
///
/// When flex-basis is auto:
///   - If the item has a definite main-axis size (width for row,
///     height for column), use that.
///   - Otherwise, use max-content size on the main axis.
///
/// TODO(content-main-size): `measure_content_size` today returns
/// max-content *width*. For column-direction flex items without a
/// definite height, the spec wants their max-content *block* size
/// (intrinsic height) here. We don't have that helper yet; for
/// column-direction items with no `height` declared we return 0.0
/// and document the gap. In practice this collapses
/// `display: flex; flex-direction: column` items without explicit
/// heights to zero main size — visible in
/// `koala-ui/res/landing.html`'s capability / binding rows.
fn flex_base_from_main_or_content(
    child: &LayoutBox,
    axis: MainAxis,
    viewport: Rect,
    cb_width: f32,
    font_metrics: &dyn FontMetrics,
) -> f32 {
    match axis {
        MainAxis::Row => {
            if let Some(ref w) = child.width {
                let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(w, viewport, cb_width);
                if !resolved.is_auto() {
                    // Note: The caller handles border-box conversion after this
                    // returns, so we return the raw CSS value here.
                    return resolved.to_px_or(0.0);
                }
            }
            // No definite width — use max-content size.
            child.measure_content_size(viewport, font_metrics)
        }
        MainAxis::Column => {
            if let Some(ref h) = child.height {
                let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(h, viewport, cb_width);
                if !resolved.is_auto() {
                    return resolved.to_px_or(0.0);
                }
            }
            // TODO(content-main-size): need intrinsic block size for
            // column-direction items without a declared height.
            0.0
        }
    }
}

/// [§ 9.7 Resolving Flexible Lengths](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
///
/// Full iterative freeze-loop algorithm.
fn resolve_flexible_lengths(items: &mut [FlexItem], available_main: f32) {
    if items.is_empty() {
        return;
    }

    // STEP 1: "Determine the used flex factor."
    //
    // "If the sum of the outer hypothetical main sizes of all items on the
    // line is less than the flex container's inner main size, use the flex
    // grow factor for the rest of this algorithm; otherwise, use the flex
    // shrink factor."
    let sum_outer_hypo: f32 = items
        .iter()
        .map(|item| item.hypothetical_size + item.outer_main)
        .sum();
    let growing = sum_outer_hypo < available_main;

    // STEP 2: "Size inflexible items."
    //
    // "Freeze, setting its target main size to its hypothetical main size…
    //   - any item that has a flex factor of zero
    //   - if using the flex grow factor: any item that has a flex base size
    //     greater than its hypothetical main size
    //   - if using the flex shrink factor: any item that has a flex base size
    //     less than its hypothetical main size"
    for item in items.iter_mut() {
        let factor = if growing { item.grow } else { item.shrink };
        let freeze = factor == 0.0
            || (growing && item.base_size > item.hypothetical_size)
            || (!growing && item.base_size < item.hypothetical_size);
        if freeze {
            item.frozen = true;
            item.target_size = item.hypothetical_size;
        }
    }

    // STEP 3: "Calculate initial free space."
    let initial_free_space = available_main
        - items
            .iter()
            .map(|item| {
                if item.frozen {
                    item.target_size + item.outer_main
                } else {
                    item.base_size + item.outer_main
                }
            })
            .sum::<f32>();

    // STEP 4: Loop until all items are frozen.
    loop {
        // 4a. Check for all frozen.
        if items.iter().all(|item| item.frozen) {
            break;
        }

        // 4b. Calculate remaining free space.
        let remaining_free = available_main
            - items
                .iter()
                .map(|item| {
                    if item.frozen {
                        item.target_size + item.outer_main
                    } else {
                        item.base_size + item.outer_main
                    }
                })
                .sum::<f32>();

        // 4c. "If the sum of the unfrozen flex factors is less than one,
        //      multiply the initial free space by this sum."
        let unfrozen_factor_sum: f32 = items
            .iter()
            .filter(|item| !item.frozen)
            .map(|item| if growing { item.grow } else { item.shrink })
            .sum();

        let free_space = if unfrozen_factor_sum < 1.0 && unfrozen_factor_sum > 0.0 {
            let scaled = initial_free_space * unfrozen_factor_sum;
            // "…if the magnitude of this value is less than the magnitude
            // of the remaining free space, use this as the used free space."
            if scaled.abs() < remaining_free.abs() {
                scaled
            } else {
                remaining_free
            }
        } else {
            remaining_free
        };

        // 4d. Distribute free space.
        if growing {
            // [§ 9.7 step 6e](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
            //
            // "If using the flex grow factor: Find the ratio of the item's
            // flex grow factor to the sum of the flex grow factors of all
            // unfrozen items on the line. Set the item's target main size
            // to its flex base size plus a fraction of the remaining free
            // space proportional to the ratio."
            let grow_sum: f32 = items
                .iter()
                .filter(|item| !item.frozen)
                .map(|item| item.grow)
                .sum();
            if grow_sum > 0.0 {
                for item in items.iter_mut().filter(|item| !item.frozen) {
                    let ratio = item.grow / grow_sum;
                    item.target_size = item.base_size + free_space * ratio;
                }
            }
        } else {
            // [§ 9.7 step 6e](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
            //
            // "If using the flex shrink factor: For every unfrozen item on
            // the line, multiply its flex shrink factor by its inner flex
            // base size, and note this as its scaled flex shrink factor.
            // Find the ratio of the item's scaled flex shrink factor to the
            // sum of the scaled flex shrink factors of all unfrozen items on
            // the line. Set the item's target main size to its flex base size
            // minus a fraction of the absolute value of the remaining free
            // space proportional to the ratio."
            let scaled_shrink_sum: f32 = items
                .iter()
                .filter(|item| !item.frozen)
                .map(|item| item.shrink * item.base_size)
                .sum();
            if scaled_shrink_sum > 0.0 {
                for item in items.iter_mut().filter(|item| !item.frozen) {
                    let scaled = item.shrink * item.base_size;
                    let ratio = scaled / scaled_shrink_sum;
                    item.target_size = free_space.abs().mul_add(-ratio, item.base_size);
                }
            }
        }

        // 4e. Fix min/max violations and determine total violation.
        //
        // MVP: no min-width/max-width, so we only clamp to 0 (content
        // boxes cannot have negative widths).
        let mut total_violation = 0.0_f32;
        for item in items.iter_mut().filter(|item| !item.frozen) {
            let clamped = item.target_size.max(0.0);
            total_violation += clamped - item.target_size;
            item.target_size = clamped;
        }

        // 4f. Freeze based on violation.
        //
        // "If the total violation is:
        //   - Zero: freeze all items.
        //   - Positive: freeze all items with min violations.
        //   - Negative: freeze all items with max violations."
        if total_violation.abs() < 0.01 {
            // Zero violation — freeze all unfrozen items.
            for item in items.iter_mut() {
                item.frozen = true;
            }
        } else if total_violation > 0.0 {
            // Positive — freeze items that were clamped up (min violation).
            // In our MVP (only clamp to 0), this means items whose
            // unclamped target was < 0.
            for item in items.iter_mut().filter(|item| !item.frozen) {
                if item.target_size <= 0.01 {
                    item.frozen = true;
                }
            }
        } else {
            // Negative — freeze items that were clamped down (max violation).
            // MVP: no max-width, so this branch is a no-op. Freeze all to
            // prevent infinite loops.
            for item in items.iter_mut() {
                item.frozen = true;
            }
        }
    }
}

/// Compute justify-content alignment offsets.
///
/// [§ 8.2 Axis Alignment: the justify-content property](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
///
/// Returns `(initial_offset, gap_between_items)`.
fn compute_justify_offsets(
    keyword: JustifyContent,
    free_space: f32,
    item_count: usize,
) -> (f32, f32) {
    if item_count == 0 {
        return (0.0, 0.0);
    }

    match keyword {
        // "Flex items are packed toward the end of the line."
        JustifyContent::FlexEnd => (free_space, 0.0),

        // "Flex items are packed toward the center of the line."
        JustifyContent::Center => (free_space / 2.0, 0.0),

        // "Flex items are evenly distributed in the line. If the leftover
        // free-space is negative or there is only a single flex item on the
        // line, this value is identical to flex-start."
        JustifyContent::SpaceBetween => {
            if item_count <= 1 || free_space <= 0.0 {
                (0.0, 0.0)
            } else {
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                let gap = free_space / (item_count - 1) as f32;
                (0.0, gap)
            }
        }

        // "Flex items are evenly distributed in the line, with half-size
        // spaces on either end."
        JustifyContent::SpaceAround => {
            if free_space <= 0.0 {
                (0.0, 0.0)
            } else {
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                let gap = free_space / item_count as f32;
                (gap / 2.0, gap)
            }
        }

        // "Flex items are packed toward the start of the line."
        JustifyContent::FlexStart => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the `MainAxis` abstraction.
    //!
    //! The flex algorithm reads main vs cross extents off rectangles
    //! and edge structs in hundreds of places once the refactor is
    //! complete; getting these helpers right in isolation means the
    //! sweep across the algorithm is mechanical instead of a debug
    //! safari. Two checks per method (one per direction) cover the
    //! contract exhaustively because every method is a binary swap.
    use super::*;

    fn sample_rect() -> Rect {
        // Deliberately asymmetric so we can tell which axis is which
        // from the assertion alone.
        Rect { x: 10.0, y: 20.0, width: 100.0, height: 50.0 }
    }

    #[test]
    fn from_flex_direction_collapses_reverse_to_same_axis() {
        assert_eq!(MainAxis::from_flex_direction(FlexDirection::Row), MainAxis::Row);
        assert_eq!(
            MainAxis::from_flex_direction(FlexDirection::RowReverse),
            MainAxis::Row,
        );
        assert_eq!(
            MainAxis::from_flex_direction(FlexDirection::Column),
            MainAxis::Column,
        );
        assert_eq!(
            MainAxis::from_flex_direction(FlexDirection::ColumnReverse),
            MainAxis::Column,
        );
    }

    #[test]
    fn main_size_is_width_for_row_height_for_column() {
        let r = sample_rect();
        assert_eq!(MainAxis::Row.main_size(&r), 100.0);
        assert_eq!(MainAxis::Column.main_size(&r), 50.0);
    }

    #[test]
    fn cross_size_is_height_for_row_width_for_column() {
        let r = sample_rect();
        assert_eq!(MainAxis::Row.cross_size(&r), 50.0);
        assert_eq!(MainAxis::Column.cross_size(&r), 100.0);
    }

    #[test]
    fn main_start_is_x_for_row_y_for_column() {
        let r = sample_rect();
        assert_eq!(MainAxis::Row.main_start(&r), 10.0);
        assert_eq!(MainAxis::Column.main_start(&r), 20.0);
    }

    #[test]
    fn cross_start_is_y_for_row_x_for_column() {
        let r = sample_rect();
        assert_eq!(MainAxis::Row.cross_start(&r), 20.0);
        assert_eq!(MainAxis::Column.cross_start(&r), 10.0);
    }

    #[test]
    fn main_edge_sum_is_horizontal_for_row_vertical_for_column() {
        // top, right, bottom, left
        assert_eq!(MainAxis::Row.main_edge_sum(1.0, 2.0, 4.0, 8.0), 10.0);
        assert_eq!(MainAxis::Column.main_edge_sum(1.0, 2.0, 4.0, 8.0), 5.0);
    }
}
