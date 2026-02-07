//! CSS Flexbox Layout Algorithm (MVP).
//!
//! [§ 9 Flex Layout Algorithm](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
//!
//! This module implements a minimal viable subset of CSS Flexbox:
//! - `flex-direction: row` (horizontal main axis)
//! - `flex-grow` / `flex-shrink` distribution (§ 9.7)
//! - `flex-basis` (definite length or auto)
//! - `justify-content` (5 keywords)
//! - No margin collapsing between flex items
//!
//! Not yet implemented: column direction, flex-wrap, align-items/self/content,
//! order, inline-flex, stretch re-layout, flex shorthand.

use crate::style::AutoLength;

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
        let resolved_padding = child.padding.resolve(viewport);
        let resolved_border = child.border_width.resolve(viewport);
        let resolved_margin = child.margin.resolve(viewport);

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
            || flex_base_from_width_or_content(child, viewport, font_metrics),
            |fb| {
                let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(fb, viewport);
                if resolved.is_auto() {
                    // flex-basis: auto — fall through to width or content sizing
                    flex_base_from_width_or_content(child, viewport, font_metrics)
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

    // STEP 4 (spec step 6 / § 9.7): Resolve flexible lengths.
    #[cfg(feature = "layout-trace")]
    {
        let m: u8 = 0;
        eprintln!(
            "[FLEX] after step 3 (measures done), stack=0x{:x}",
            &m as *const u8 as usize
        );
    }
    resolve_flexible_lengths(&mut items, available_main);

    // STEP 5 (spec step 12): Compute justify-content offsets.
    let justify_keyword = container.justify_content.as_str();
    let total_target: f32 = items
        .iter()
        .map(|item| item.target_size + item.outer_main)
        .sum();
    let free_space = (available_main - total_target).max(0.0);
    let (initial_offset, gap) = compute_justify_offsets(justify_keyword, free_space, items.len());

    // STEP 6 (spec step 7): Layout each child ONCE with resolved main size.
    //
    // [§ 9.5 Cross Sizing](https://www.w3.org/TR/css-flexbox-1/#algo-cross-item)
    //
    // Each child is laid out with its target main size as the containing
    // block width, and unconstrained cross size.
    //
    // [§ 9.3 Main Size Determination](https://www.w3.org/TR/css-flexbox-1/#algo-main-container)
    //
    // The resolved flex lengths override the item's intrinsic/specified
    // width. We set the child's width to the target size so that
    // calculate_block_width() uses it as a definite length.
    let mut current_x = content_box.x + initial_offset;

    #[cfg(feature = "layout-trace")]
    {
        let m: u8 = 0;
        eprintln!(
            "[FLEX] before step 6 (child layout), stack=0x{:x}",
            &m as *const u8 as usize
        );
    }

    for (item_idx, item) in items.iter().enumerate() {
        let child = &mut container.children[item.index];

        // Override the child's width with the resolved flex main size.
        // [§ 9.7](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
        //
        // "Set each item's used main size to its target main size."
        //
        // [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
        //
        // If the child uses border-box, add padding+border back to the
        // content-box target_size so that calculate_block_width() correctly
        // converts it back. For content-box children, use as-is.
        let width_for_layout = if child.box_sizing_border_box {
            let rp = child.padding.resolve(viewport);
            let rb = child.border_width.resolve(viewport);
            item.target_size + rp.left + rp.right + rb.left + rb.right
        } else {
            item.target_size
        };
        child.width = Some(AutoLength::Length(crate::style::LengthValue::Px(
            f64::from(width_for_layout),
        )));

        let child_containing = Rect {
            x: current_x,
            y: content_box.y,
            width: item.target_size,
            height: f32::MAX, // unconstrained cross size
        };

        #[cfg(feature = "layout-trace")]
        eprintln!(
            "[FLEX] STEP 6: laying out child {item_idx}, display={:?}/{:?}, {} grandchildren",
            child.display.outer,
            child.display.inner,
            child.children.len()
        );
        child.layout(child_containing, viewport, font_metrics, child_abs_cb);
        #[cfg(feature = "layout-trace")]
        eprintln!("[FLEX] STEP 6: child {item_idx} layout complete");

        current_x += child.dimensions.margin_box().width;
        if item_idx < items.len() - 1 {
            current_x += gap;
        }
    }

    // STEP 7 (spec step 14): Container height.
    //
    // [§ 9.9 Cross Size Determination](https://www.w3.org/TR/css-flexbox-1/#algo-cross-container)
    //
    // "If the cross size property is a definite size, use that; otherwise,
    // use the largest of the flex lines' cross sizes."
    if let Some(AutoLength::Length(ref l)) = container.height {
        #[allow(clippy::cast_possible_truncation)]
        {
            container.dimensions.content.height =
                l.to_px_with_viewport(f64::from(viewport.width), f64::from(viewport.height)) as f32;
        }
    } else {
        // Auto height: max of in-flow children's margin-box heights.
        // Absolute/fixed children do not contribute to the container's
        // auto height.
        let max_height = container
            .children
            .iter()
            .filter(|c| {
                !matches!(
                    c.position_type,
                    PositionType::Absolute | PositionType::Fixed
                )
            })
            .map(|c| c.dimensions.margin_box().height)
            .fold(0.0_f32, f32::max);
        container.dimensions.content.height = max_height;
    }

    // STEP 8: Layout absolutely positioned children.
    // [§ 4.1 Absolutely-Positioned Flex Children](https://www.w3.org/TR/css-flexbox-1/#abspos-items)
    //
    // "An absolutely-positioned child of a flex container does not
    // participate in flex layout." They are positioned after flex
    // layout completes.
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
    font_metrics: &dyn FontMetrics,
) -> f32 {
    if let Some(ref w) = child.width {
        let resolved = UnresolvedAutoEdgeSizes::resolve_auto_length(w, viewport);
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
fn compute_justify_offsets(keyword: &str, free_space: f32, item_count: usize) -> (f32, f32) {
    if item_count == 0 {
        return (0.0, 0.0);
    }

    match keyword {
        // "Flex items are packed toward the end of the line."
        "flex-end" => (free_space, 0.0),

        // "Flex items are packed toward the center of the line."
        "center" => (free_space / 2.0, 0.0),

        // "Flex items are evenly distributed in the line. If the leftover
        // free-space is negative or there is only a single flex item on the
        // line, this value is identical to flex-start."
        "space-between" => {
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
        "space-around" => {
            if free_space <= 0.0 {
                (0.0, 0.0)
            } else {
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                let gap = free_space / item_count as f32;
                (gap / 2.0, gap)
            }
        }

        // "Flex items are packed toward the start of the line."
        // Default: flex-start
        _ => (0.0, 0.0),
    }
}
