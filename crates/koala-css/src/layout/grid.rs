//! CSS Grid Layout Algorithm (MVP).
//!
//! [§ 12 Grid Sizing](https://www.w3.org/TR/css-grid-1/#layout-algorithm)
//!
//! This module implements a minimal viable subset of CSS Grid Layout:
//! - `grid-template-columns` / `grid-template-rows` (px, fr, auto, repeat())
//! - `gap` / `row-gap` / `column-gap`
//! - Explicit placement via `grid-column` / `grid-row` (line numbers, span)
//! - Auto-placement in row-major or column-major order (§ 8.5)
//!
//! Not yet implemented: grid-template-areas, grid-auto-rows/columns, subgrid,
//! minmax(), auto-fill/auto-fit, alignment properties.

use crate::style::computed::{GridAutoFlow, GridLine, TrackSize};
use crate::style::{AutoLength, LengthValue};

use super::box_model::Rect;
use super::inline::FontMetrics;
use super::layout_box::LayoutBox;
use super::positioned::PositionType;

/// 0-based grid position for an item.
struct GridPosition {
    col_start: usize,
    col_end: usize,
    row_start: usize,
    row_end: usize,
}

/// A grid item with its child index and resolved position.
struct GridItem {
    child_index: usize,
    position: GridPosition,
}

/// Main entry point for grid layout.
///
/// [§ 12 Grid Sizing](https://www.w3.org/TR/css-grid-1/#layout-algorithm)
///
/// This function is called from `LayoutBox::layout()` when the box has
/// `display.inner == InnerDisplayType::Grid`.
pub fn layout_grid(
    container: &mut LayoutBox,
    containing_block: Rect,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
    abs_cb: Rect,
) {
    // STEP 1 (§ 12.1): Resolve container's own width.
    //
    // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    //
    // Reuse the block width algorithm — the grid container is a block-level
    // box, so its own width is determined by the same constraint equation.
    container.calculate_block_width(containing_block, viewport);

    // STEP 2: Resolve container position.
    container.calculate_block_position(containing_block, viewport);

    // [§ 10.1 Definition of containing block](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
    //
    // If the grid container is positioned, its padding box becomes the
    // containing block for absolutely positioned descendants.
    let child_abs_cb = if container.is_positioned() {
        container.dimensions.padding_box()
    } else {
        abs_cb
    };

    let content_box = container.dimensions.content_box();
    let available_width = content_box.width;

    // STEP 3: Determine the number of explicit columns and rows.
    let num_explicit_cols = container.grid_template_columns.sizes.len();
    let num_explicit_rows = container.grid_template_rows.sizes.len();
    let col_gap = container.column_gap;
    let row_gap = container.row_gap;
    let auto_flow = container.grid_auto_flow;

    // STEP 4 (§ 8.5): Place grid items.
    //
    // [§ 8.5 Grid Item Placement Algorithm](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
    //
    // "Grid items that aren't explicitly placed are automatically placed into
    // an unoccupied space in the grid container."
    let in_flow_indices: Vec<usize> = (0..container.children.len())
        .filter(|&i| {
            !matches!(
                container.children[i].position_type,
                PositionType::Absolute | PositionType::Fixed
            )
        })
        .collect();

    let default_cols = if num_explicit_cols > 0 {
        num_explicit_cols
    } else {
        // No explicit columns: default to 1 column
        1
    };

    let items = place_grid_items(
        container,
        &in_flow_indices,
        default_cols,
        num_explicit_rows,
        auto_flow,
    );

    // Determine the actual grid dimensions from placed items.
    let mut num_cols = default_cols;
    let mut num_rows = if num_explicit_rows > 0 {
        num_explicit_rows
    } else {
        1
    };
    for item in &items {
        num_cols = num_cols.max(item.position.col_end);
        num_rows = num_rows.max(item.position.row_end);
    }

    // STEP 5 (§ 12.3): Resolve column track sizes.
    let column_sizes = resolve_track_sizes(
        &container.grid_template_columns.sizes,
        num_cols,
        available_width,
        col_gap,
        container,
        &items,
        true, // columns
        viewport,
        font_metrics,
    );

    // STEP 6: First pass — layout children with resolved column widths
    // to determine content heights for auto row sizing.
    let mut child_heights: Vec<f32> = vec![0.0; container.children.len()];

    for item in &items {
        let child = &mut container.children[item.child_index];

        // Calculate the item's available width from its column span.
        let item_width = track_span_size(&column_sizes, item.position.col_start, item.position.col_end, col_gap);

        // Override the child's width with the grid cell width.
        child.width = Some(AutoLength::Length(LengthValue::Px(
            f64::from(item_width),
        )));

        // Layout with a temporary containing block to measure height.
        let temp_cb = Rect {
            x: 0.0,
            y: 0.0,
            width: item_width,
            height: f32::MAX,
        };
        child.layout(temp_cb, viewport, font_metrics, child_abs_cb);
        child_heights[item.child_index] = child.dimensions.margin_box().height;
    }

    // STEP 7 (§ 12.3): Resolve row track sizes.
    //
    // Row auto tracks use the maximum content height of items in that row.
    let row_sizes = resolve_row_track_sizes(
        &container.grid_template_rows.sizes,
        num_rows,
        &items,
        &child_heights,
    );

    // STEP 8: Compute track offsets (x/y positions for each track).
    let col_offsets = compute_track_offsets(&column_sizes, col_gap, content_box.x);
    let row_offsets = compute_track_offsets(&row_sizes, row_gap, content_box.y);

    // STEP 9: Final pass — layout each child at its final position.
    for item in &items {
        let child = &mut container.children[item.child_index];

        let cell_x = col_offsets[item.position.col_start];
        let cell_y = row_offsets[item.position.row_start];
        let cell_width = track_span_size(&column_sizes, item.position.col_start, item.position.col_end, col_gap);
        let cell_height = track_span_size(&row_sizes, item.position.row_start, item.position.row_end, row_gap);

        // Override width for the final layout.
        child.width = Some(AutoLength::Length(LengthValue::Px(
            f64::from(cell_width),
        )));

        let child_cb = Rect {
            x: cell_x,
            y: cell_y,
            width: cell_width,
            height: cell_height,
        };
        child.layout(child_cb, viewport, font_metrics, child_abs_cb);
    }

    // STEP 10: Container height.
    //
    // [§ 12.4 Grid Container Sizing](https://www.w3.org/TR/css-grid-1/#algo-overview)
    //
    // "If the grid container's size is definite, use that. Otherwise,
    // compute from the track sizes."
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
            let total_row_gaps = if num_rows > 1 {
                row_gap * (num_rows - 1) as f32
            } else {
                0.0
            };
            let total_height: f32 = row_sizes.iter().sum::<f32>() + total_row_gaps;
            container.dimensions.content.height = total_height;
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
        // Auto height: sum of row tracks + gaps.
        let total_row_gaps = if num_rows > 1 {
            row_gap * (num_rows - 1) as f32
        } else {
            0.0
        };
        let total_height: f32 = row_sizes.iter().sum::<f32>() + total_row_gaps;
        container.dimensions.content.height = total_height;
    }

    // STEP 11: Layout absolutely positioned children.
    container.layout_absolute_children(viewport, font_metrics, child_abs_cb);
}

/// [§ 8.5 Grid Item Placement Algorithm](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
///
/// Place all grid items, handling both explicit and auto placement.
fn place_grid_items(
    container: &LayoutBox,
    in_flow_indices: &[usize],
    num_cols: usize,
    num_explicit_rows: usize,
    auto_flow: GridAutoFlow,
) -> Vec<GridItem> {
    let mut items = Vec::with_capacity(in_flow_indices.len());

    // Occupancy grid: tracks which cells are occupied.
    // Grows dynamically as items are placed.
    let initial_rows = num_explicit_rows.max(1);
    let mut occupancy: Vec<Vec<bool>> = vec![vec![false; num_cols]; initial_rows];

    // STEP 1: Place explicitly positioned items first.
    //
    // [§ 8.5 step 1](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
    //
    // "Place all items with a definite row position AND definite column
    // position. Grow the implicit grid as needed."
    for &idx in in_flow_indices {
        let child = &container.children[idx];
        let col_start = resolve_definite_line(child.grid_column_start, num_cols);
        let col_end = resolve_definite_line(child.grid_column_end, num_cols);
        let row_start = resolve_definite_line(child.grid_row_start, occupancy.len());
        let row_end = resolve_definite_line(child.grid_row_end, occupancy.len());

        if let (Some(cs), Some(rs)) = (col_start, row_start) {
            let ce = col_end.unwrap_or_else(|| resolve_span_end(child.grid_column_end, cs, num_cols));
            let re = row_end.unwrap_or_else(|| resolve_span_end(child.grid_row_end, rs, occupancy.len()));

            // Grow occupancy grid if needed.
            grow_occupancy(&mut occupancy, re, num_cols);

            // Mark cells as occupied.
            mark_occupied(&mut occupancy, cs, ce, rs, re);

            items.push(GridItem {
                child_index: idx,
                position: GridPosition {
                    col_start: cs,
                    col_end: ce,
                    row_start: rs,
                    row_end: re,
                },
            });
        }
    }

    // STEP 2: Auto-place remaining items.
    //
    // [§ 8.5 step 4](https://www.w3.org/TR/css-grid-1/#auto-placement-algo)
    //
    // "Process the items locked to a given row/column, then process the
    // remaining items."
    let placed_indices: Vec<usize> = items.iter().map(|i| i.child_index).collect();

    // Cursor for auto-placement
    let mut cursor_row: usize = 0;
    let mut cursor_col: usize = 0;

    for &idx in in_flow_indices {
        if placed_indices.contains(&idx) {
            continue;
        }

        let child = &container.children[idx];

        // Determine the span for this item.
        let col_span = resolve_item_span(child.grid_column_start, child.grid_column_end);
        let row_span = resolve_item_span(child.grid_row_start, child.grid_row_end);

        // Check if item has a definite column but not a definite row.
        let definite_col = resolve_definite_line(child.grid_column_start, num_cols);

        if let Some(cs) = definite_col {
            // Item has definite column: scan rows to find space.
            let ce = cs + col_span;
            let mut r = 0;
            loop {
                let re = r + row_span;
                grow_occupancy(&mut occupancy, re, num_cols);
                if is_area_free(&occupancy, cs, ce, r, re) {
                    mark_occupied(&mut occupancy, cs, ce, r, re);
                    items.push(GridItem {
                        child_index: idx,
                        position: GridPosition {
                            col_start: cs,
                            col_end: ce,
                            row_start: r,
                            row_end: re,
                        },
                    });
                    break;
                }
                r += 1;
            }
        } else {
            // Fully auto-placed: scan from cursor.
            match auto_flow {
                GridAutoFlow::Row => {
                    loop {
                        let ce = cursor_col + col_span;
                        let re = cursor_row + row_span;

                        if ce <= num_cols {
                            grow_occupancy(&mut occupancy, re, num_cols);
                            if is_area_free(&occupancy, cursor_col, ce, cursor_row, re) {
                                mark_occupied(&mut occupancy, cursor_col, ce, cursor_row, re);
                                items.push(GridItem {
                                    child_index: idx,
                                    position: GridPosition {
                                        col_start: cursor_col,
                                        col_end: ce,
                                        row_start: cursor_row,
                                        row_end: re,
                                    },
                                });
                                break;
                            }
                        }

                        // Advance cursor.
                        cursor_col += 1;
                        if cursor_col + col_span > num_cols {
                            cursor_col = 0;
                            cursor_row += 1;
                        }
                    }
                }
                GridAutoFlow::Column => {
                    loop {
                        let ce = cursor_col + col_span;
                        let re = cursor_row + row_span;

                        grow_occupancy(&mut occupancy, re, num_cols);
                        if ce <= num_cols && is_area_free(&occupancy, cursor_col, ce, cursor_row, re) {
                            mark_occupied(&mut occupancy, cursor_col, ce, cursor_row, re);
                            items.push(GridItem {
                                child_index: idx,
                                position: GridPosition {
                                    col_start: cursor_col,
                                    col_end: ce,
                                    row_start: cursor_row,
                                    row_end: re,
                                },
                            });
                            break;
                        }

                        // Column-major: advance row first.
                        cursor_row += 1;
                        if cursor_row + row_span > occupancy.len() + 1 {
                            cursor_row = 0;
                            cursor_col += 1;
                            if cursor_col >= num_cols {
                                cursor_col = 0;
                                // Need more rows
                                cursor_row = occupancy.len();
                            }
                        }
                    }
                }
            }
        }
    }

    items
}

/// Resolve a `GridLine` to a 0-based track index if it specifies a definite line.
///
/// [§ 8.3](https://www.w3.org/TR/css-grid-1/#line-placement)
///
/// - `GridLine::Line(n)` where n > 0 → n - 1 (convert 1-based to 0-based)
/// - `GridLine::Line(n)` where n < 0 → count from end
/// - `GridLine::Auto` / `GridLine::Span(_)` → None (not definite)
fn resolve_definite_line(line: GridLine, track_count: usize) -> Option<usize> {
    match line {
        GridLine::Line(n) if n > 0 => Some((n - 1) as usize),
        GridLine::Line(n) if n < 0 => {
            // Negative line: count from end.
            // -1 = last line = track_count, so track index = track_count + n
            let idx = track_count as i32 + n;
            if idx >= 0 {
                Some(idx as usize)
            } else {
                Some(0)
            }
        }
        _ => None,
    }
}

/// Resolve the end of a span given a start position.
fn resolve_span_end(line: GridLine, start: usize, _track_count: usize) -> usize {
    match line {
        GridLine::Span(n) => start + n as usize,
        GridLine::Auto => start + 1, // default span is 1
        GridLine::Line(_) => start + 1, // shouldn't reach here, fallback
    }
}

/// Determine the span of a grid item from its start/end line values.
///
/// Default span is 1 if neither specifies a span.
fn resolve_item_span(start: GridLine, end: GridLine) -> usize {
    // If end is Span(n), use that.
    if let GridLine::Span(n) = end {
        return n as usize;
    }
    // If start is Span(n), use that.
    if let GridLine::Span(n) = start {
        return n as usize;
    }
    // If both are Line, compute span from difference.
    if let (GridLine::Line(s), GridLine::Line(e)) = (start, end) {
        let span = (e - s).unsigned_abs() as usize;
        if span > 0 {
            return span;
        }
    }
    // Default span.
    1
}

/// Grow the occupancy grid to have at least `min_rows` rows.
fn grow_occupancy(occupancy: &mut Vec<Vec<bool>>, min_rows: usize, num_cols: usize) {
    while occupancy.len() < min_rows {
        occupancy.push(vec![false; num_cols]);
    }
}

/// Check if a rectangular area in the occupancy grid is free.
fn is_area_free(
    occupancy: &[Vec<bool>],
    col_start: usize,
    col_end: usize,
    row_start: usize,
    row_end: usize,
) -> bool {
    for row in &occupancy[row_start..row_end] {
        for &cell in &row[col_start..col_end] {
            if cell {
                return false;
            }
        }
    }
    true
}

/// Mark a rectangular area in the occupancy grid as occupied.
fn mark_occupied(
    occupancy: &mut [Vec<bool>],
    col_start: usize,
    col_end: usize,
    row_start: usize,
    row_end: usize,
) {
    for row in &mut occupancy[row_start..row_end] {
        for cell in &mut row[col_start..col_end] {
            *cell = true;
        }
    }
}

/// [§ 12.3 Track Sizing Algorithm](https://www.w3.org/TR/css-grid-1/#algo-track-sizing)
///
/// Resolve column track sizes (simplified).
///
/// 1. Fixed tracks → use specified size.
/// 2. Auto tracks → max-content of items in that column.
/// 3. Fr tracks → distribute remaining space proportionally.
#[allow(clippy::too_many_arguments)]
fn resolve_track_sizes(
    template: &[TrackSize],
    num_tracks: usize,
    available: f32,
    gap: f32,
    container: &LayoutBox,
    items: &[GridItem],
    is_columns: bool,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
) -> Vec<f32> {
    let mut sizes = vec![0.0_f32; num_tracks];
    let mut total_fixed = 0.0_f32;
    let mut total_fr = 0.0_f32;

    // Total gap space.
    let total_gaps = if num_tracks > 1 {
        gap * (num_tracks - 1) as f32
    } else {
        0.0
    };

    // STEP 1: Initialize fixed and auto tracks.
    for (i, size) in sizes.iter_mut().enumerate() {
        let track_def = template.get(i).copied().unwrap_or(TrackSize::Auto);
        match track_def {
            TrackSize::Fixed(px) => {
                *size = px;
                total_fixed += px;
            }
            TrackSize::Auto => {
                // Measure max-content of items in this track.
                if is_columns {
                    let max_content = items
                        .iter()
                        .filter(|item| item.position.col_start == i && item.position.col_end == i + 1)
                        .map(|item| {
                            container.children[item.child_index]
                                .measure_content_size(viewport, font_metrics)
                        })
                        .fold(0.0_f32, f32::max);
                    *size = max_content;
                    total_fixed += max_content;
                }
            }
            TrackSize::Fr(fr) => {
                total_fr += fr;
            }
        }
    }

    // STEP 2: Distribute remaining space to fr tracks.
    //
    // [§ 12.7.1 Distribute space to fr tracks](https://www.w3.org/TR/css-grid-1/#algo-find-fr-size)
    //
    // "free_space = available - fixed_total - gaps"
    // "px_per_fr = free_space / total_fr"
    if total_fr > 0.0 {
        let free_space = (available - total_fixed - total_gaps).max(0.0);
        let px_per_fr = free_space / total_fr;

        for (i, size) in sizes.iter_mut().enumerate() {
            let track_def = template.get(i).copied().unwrap_or(TrackSize::Auto);
            if let TrackSize::Fr(fr) = track_def {
                *size = px_per_fr * fr;
            }
        }
    }

    sizes
}

/// Resolve row track sizes.
///
/// Row tracks that are auto-sized use the maximum content height of items
/// in that row (determined after the first layout pass).
fn resolve_row_track_sizes(
    template: &[TrackSize],
    num_rows: usize,
    items: &[GridItem],
    child_heights: &[f32],
) -> Vec<f32> {
    let mut sizes = vec![0.0_f32; num_rows];

    for (i, size) in sizes.iter_mut().enumerate() {
        let track_def = template.get(i).copied().unwrap_or(TrackSize::Auto);
        match track_def {
            TrackSize::Fixed(px) => {
                *size = px;
            }
            TrackSize::Auto | TrackSize::Fr(_) => {
                // Use max content height of items in this row.
                let max_h = items
                    .iter()
                    .filter(|item| item.position.row_start == i && item.position.row_end == i + 1)
                    .map(|item| child_heights[item.child_index])
                    .fold(0.0_f32, f32::max);
                *size = max_h;
            }
        }
    }

    sizes
}

/// Compute the x or y offset for each track from track sizes and gaps.
fn compute_track_offsets(sizes: &[f32], gap: f32, start: f32) -> Vec<f32> {
    let mut offsets = Vec::with_capacity(sizes.len());
    let mut pos = start;
    for (i, &size) in sizes.iter().enumerate() {
        offsets.push(pos);
        pos += size;
        if i < sizes.len() - 1 {
            pos += gap;
        }
    }
    offsets
}

/// Compute the total size of a span of tracks including gaps between them.
fn track_span_size(sizes: &[f32], start: usize, end: usize, gap: f32) -> f32 {
    if start >= end || start >= sizes.len() {
        return 0.0;
    }
    let end = end.min(sizes.len());
    let track_sum: f32 = sizes[start..end].iter().sum();
    let num_gaps = (end - start).saturating_sub(1);
    track_sum + gap * num_gaps as f32
}
