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
use crate::style::computed::{AlignItems, AlignSelf, FlexWrap, JustifyContent};

use super::box_model::Rect;
use super::inline::FontMetrics;
use super::layout_box::LayoutBox;
use super::positioned::PositionType;
use super::values::UnresolvedAutoEdgeSizes;

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
    let available_main = content_box.width;

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

        let outer_main = resolved_margin.left.to_px_or(0.0)
            + resolved_border.left
            + resolved_padding.left
            + resolved_padding.right
            + resolved_border.right
            + resolved_margin.right.to_px_or(0.0);

        // [§ 9.2 step 3](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
        //
        // Determine flex base size:
        //   A. If flex-basis is a definite length, use it.
        //   B. If flex-basis is auto and the item has a definite width, use that.
        //   C. Otherwise, use max-content size via measure_content_size().
        let mut base_size = child.flex_basis.as_ref().map_or_else(
            || flex_base_from_width_or_content(child, viewport, cb_width, font_metrics),
            |fb| {
                let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(fb, viewport, cb_width);
                if resolved.is_auto() {
                    // flex-basis: auto — fall through to width or content sizing
                    flex_base_from_width_or_content(child, viewport, cb_width, font_metrics)
                } else {
                    resolved.to_px_or(0.0)
                }
            },
        );

        // [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
        //
        // When box-sizing is border-box, the flex base size (from flex-basis
        // or width) includes padding and border. Convert to content-box so
        // the flex algorithm operates on content sizes.
        //
        // Note: The outer_main already accounts for margin+border+padding
        // around the content box. The base_size should be content-only.
        if child.box_sizing_border_box {
            base_size -= resolved_padding.left
                + resolved_padding.right
                + resolved_border.left
                + resolved_border.right;
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
    let mut current_y = content_box.y;
    let mut line_cross_sizes: Vec<f32> = Vec::new();
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

        // Layout each child on this line.
        let mut current_x = content_box.x + initial_offset;
        let mut line_cross_size = 0.0_f32;

        for (item_idx, line_item) in line_items.iter().enumerate() {
            let child = &mut container.children[line_item.index];

            // [§ 9.7](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
            //
            // "Set each item's used main size to its target main size."
            let width_for_layout = if child.box_sizing_border_box {
                let rp = child.padding.resolve(viewport, content_box.width);
                let rb = child.border_width.resolve(viewport, content_box.width);
                line_item.target_size + rp.left + rp.right + rb.left + rb.right
            } else {
                line_item.target_size
            };
            child.width = Some(AutoLength::Length(LengthValue::Px(
                f64::from(width_for_layout),
            )));

            let child_containing = Rect {
                x: current_x,
                y: current_y,
                width: line_item.target_size,
                height: f32::MAX,
            };

            #[cfg(feature = "layout-trace")]
            eprintln!(
                "[FLEX] line {line_idx} item {item_idx}: laying out child, display={:?}/{:?}",
                child.display.outer, child.display.inner
            );
            child.layout(child_containing, viewport, font_metrics, child_abs_cb);

            current_x += child.dimensions.margin_box().width;
            if item_idx < line_items.len() - 1 {
                current_x += gap;
            }

            // Track the tallest item on this line for cross-axis sizing
            let child_cross = child.dimensions.margin_box().height;
            line_cross_size = line_cross_size.max(child_cross);
        }

        line_cross_sizes.push(line_cross_size);
        current_y += line_cross_size;
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
    // [§ 10.5](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
    //
    // "If the height of the containing block is not specified explicitly,
    // the percentage value is treated as 'auto'."
    let cb_height_is_auto = containing_block.height >= f32::MAX / 2.0;
    let height_is_pct_with_auto_cb = matches!(container.height, Some(AutoLength::Length(LengthValue::Percent(_))))
        && cb_height_is_auto;
    if let Some(AutoLength::Length(ref l)) = container.height {
        if height_is_pct_with_auto_cb {
            // Percentage height with auto CB height → treat as auto.
            container.dimensions.content.height = line_cross_sizes.iter().sum();
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
        // Auto cross size: sum of all flex lines' cross sizes.
        container.dimensions.content.height = line_cross_sizes.iter().sum();
    }

    // [§ 9.4 Cross Size Determination](https://www.w3.org/TR/css-flexbox-1/#algo-cross-line)
    //
    // "If the flex container is single-line and has a definite cross size,
    // the cross size of the flex line is the flex container's inner cross
    // size."
    //
    // For single-line containers with a definite height, the line cross
    // size used for alignment must be the container's content height,
    // not the max child height.
    if lines.len() == 1 && container.height.is_some() {
        line_cross_sizes[0] = container.dimensions.content.height;
    }

    // STEP 8 (§ 9.6): Cross-axis alignment per line.
    //
    // [§ 8.3 'align-items'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
    //
    // "The align-items property sets the default alignment for all of the
    // flex container's items, including anonymous flex items."
    let container_align_items = container.align_items;

    // Compute the cumulative y-offset for each line start
    let mut line_y_offsets: Vec<f32> = Vec::with_capacity(lines.len());
    let mut y_accum = 0.0_f32;
    for &cross in &line_cross_sizes {
        line_y_offsets.push(y_accum);
        y_accum += cross;
    }

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

        let child_margin_box_height = child.dimensions.margin_box().height;

        match alignment {
            AlignItems::FlexStart | AlignItems::Baseline => {}
            AlignItems::FlexEnd => {
                let offset = line_cross_size - child_margin_box_height;
                child.dimensions.content.y += offset;
            }
            AlignItems::Center => {
                let offset = (line_cross_size - child_margin_box_height) / 2.0;
                child.dimensions.content.y += offset;
            }
            AlignItems::Stretch => {
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
        }
    }

    // STEP 9: Layout absolutely positioned children.
    container.layout_absolute_children(viewport, font_metrics, child_abs_cb);
}

/// Determine flex base size from width property or content measurement.
///
/// [§ 9.2 step 3](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
///
/// When flex-basis is auto:
///   - If the item has a definite width, use that.
///   - Otherwise, use max-content size.
fn flex_base_from_width_or_content(
    child: &LayoutBox,
    viewport: Rect,
    cb_width: f32,
    font_metrics: &dyn FontMetrics,
) -> f32 {
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
