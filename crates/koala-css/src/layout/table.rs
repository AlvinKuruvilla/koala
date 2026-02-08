//! CSS Table Layout.
//!
//! [§ 17 Tables](https://www.w3.org/TR/CSS2/tables.html)
//!
//! This module implements the automatic table layout algorithm (§ 17.5.2):
//! - `<table>`, `<tr>`, `<td>`, `<th>` basic layout
//! - `<thead>`, `<tbody>`, `<tfoot>` as row groups
//! - `colspan` attribute
//! - Automatic column width sizing
//! - `border-spacing: 2px` hardcoded
//!
//! Not yet implemented: `rowspan`, `border-collapse: collapse`, `<caption>`,
//! `table-layout: fixed`, `vertical-align` within cells.

use crate::style::{AutoLength, LengthValue};

use super::box_model::Rect;
use super::inline::FontMetrics;
use super::layout_box::LayoutBox;
use super::positioned::PositionType;

/// [§ 17.6.1 The separated borders model](https://www.w3.org/TR/CSS2/tables.html#separated-borders)
///
/// "The 'border-spacing' property specifies the distance that separates
/// adjoining cell borders."
///
/// Default value per UA stylesheet is 2px.
const BORDER_SPACING: f32 = 2.0;

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

/// A resolved row: indices into `container.children` for cells within this row.
struct TableRow {
    /// Index of the `<tr>` child (or row-group child that contains the `<tr>`)
    /// in `container.children`.
    row_group_index: Option<usize>,
    /// Index of the `<tr>` within its row-group, or directly in the table.
    row_index: usize,
    /// For each cell: (child_index_within_tr, colspan)
    cells: Vec<CellInfo>,
}

struct CellInfo {
    /// Index of this cell's `LayoutBox` within the `<tr>`'s children.
    cell_index: usize,
    /// HTML `colspan` attribute value. Default: 1.
    colspan: u32,
}

/// Main entry point for table layout.
///
/// [§ 17.5.2 Automatic table layout](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
///
/// This function is called from `LayoutBox::layout()` when the box has
/// `display.inner == InnerDisplayType::Table`.
pub fn layout_table(
    container: &mut LayoutBox,
    containing_block: Rect,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
    abs_cb: Rect,
) {
    // STEP 1 (§ 17.5.2): Resolve container's own width.
    //
    // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    //
    // Reuse the block width algorithm — the table container is a block-level
    // box, so its own width is determined by the same constraint equation.
    container.calculate_block_width(containing_block, viewport);

    // STEP 2: Resolve container position.
    container.calculate_block_position(containing_block, viewport);

    // [§ 10.1 Definition of containing block](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
    //
    // If the table container is positioned, its padding box becomes the
    // containing block for absolutely positioned descendants.
    let child_abs_cb = if container.is_positioned() {
        container.dimensions.padding_box()
    } else {
        abs_cb
    };

    let content_box = container.dimensions.content_box();

    // STEP 3 (§ 17.5.2): Identify table structure.
    //
    // [§ 17.2 The CSS table model](https://www.w3.org/TR/CSS2/tables.html#table-display)
    //
    // "A table is divided into rows and columns. Row groups (thead, tbody,
    // tfoot) group rows. The intersection of a row and a column is a cell."
    //
    // Walk container.children and categorize by tag_name:
    // - <thead>, <tbody>, <tfoot> → row groups (walk their children for <tr>)
    // - <tr> → direct rows
    // - <caption> → deferred (not implemented)
    let rows = collect_table_rows(container);

    if rows.is_empty() {
        // No rows found — nothing to lay out. Set height to 0.
        container.dimensions.content.height = 0.0;
        container.layout_absolute_children(viewport, font_metrics, child_abs_cb);
        return;
    }

    // STEP 4 (§ 17.5.2.1): Determine the column count.
    //
    // "The number of columns is determined by... the row with the most cells."
    let num_cols = determine_column_count(&rows, container);

    if num_cols == 0 {
        container.dimensions.content.height = 0.0;
        container.layout_absolute_children(viewport, font_metrics, child_abs_cb);
        return;
    }

    // STEP 5 (§ 17.5.2.2): Column width determination.
    //
    // [§ 17.5.2.2](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
    //
    // "Column widths are determined as follows: ... calculate the minimum
    // and maximum width of each cell ... For each column, determine a
    // minimum and maximum column width from the cells that span only that
    // column."
    let column_widths = determine_column_widths(
        container,
        &rows,
        num_cols,
        content_box.width,
        viewport,
        font_metrics,
    );

    // STEP 6 (§ 17.5.2): Layout cells at determined widths.
    //
    // For each row, for each cell:
    // - Set cell width to the column width (or sum of spanned column widths)
    // - Layout the cell using block layout
    // - Record cell height
    let row_heights = layout_cells_and_measure_row_heights(
        container,
        &rows,
        &column_widths,
        num_cols,
        viewport,
        font_metrics,
        child_abs_cb,
    );

    // STEP 7: Position cells at final coordinates.
    //
    // Walk rows top-to-bottom, cells left-to-right at their column offsets.
    // Include border-spacing gaps between cells and rows.
    position_cells(
        container,
        &rows,
        &column_widths,
        &row_heights,
        content_box,
        viewport,
        font_metrics,
        child_abs_cb,
    );

    // STEP 8 (§ 17.5.3): Set container height.
    //
    // [§ 17.5.3](https://www.w3.org/TR/CSS2/tables.html#height-layout)
    //
    // "The height of a table is given by the 'height' property for the
    // 'table' or 'inline-table' element."
    //
    // If height is auto: sum of row heights + border-spacing.
    let total_border_spacing_y = if row_heights.is_empty() {
        0.0
    } else {
        BORDER_SPACING * (row_heights.len() + 1) as f32
    };
    let content_height: f32 = row_heights.iter().sum::<f32>() + total_border_spacing_y;

    if let Some(AutoLength::Length(ref l)) = container.height {
        #[allow(clippy::cast_possible_truncation)]
        {
            let explicit_h = l.to_px_with_containing_block(
                f64::from(containing_block.height),
                f64::from(viewport.width),
                f64::from(viewport.height),
            ) as f32;
            // Use the larger of explicit height and content height
            // (tables expand to fit content).
            container.dimensions.content.height = explicit_h.max(content_height);
        }
    } else {
        container.dimensions.content.height = content_height;
    }

    // STEP 9: Layout absolutely positioned children.
    container.layout_absolute_children(viewport, font_metrics, child_abs_cb);
}

/// [§ 17.2 The CSS table model](https://www.w3.org/TR/CSS2/tables.html#table-display)
///
/// Collect table rows from the container's children. Handles both direct
/// `<tr>` children and rows nested inside `<thead>`, `<tbody>`, `<tfoot>`.
fn collect_table_rows(container: &LayoutBox) -> Vec<TableRow> {
    let mut rows = Vec::new();

    for (child_idx, child) in container.children.iter().enumerate() {
        // Skip absolutely positioned children.
        if matches!(
            child.position_type,
            PositionType::Absolute | PositionType::Fixed
        ) {
            continue;
        }

        let tag = child.tag_name.as_deref().unwrap_or("");

        match tag {
            // [§ 17.2.1](https://www.w3.org/TR/CSS2/tables.html#table-display)
            //
            // "table-row (In HTML: TR): Specifies that an element is a row of cells."
            "tr" => {
                let cells = collect_cells_from_row(child);
                rows.push(TableRow {
                    row_group_index: None,
                    row_index: child_idx,
                    cells,
                });
            }
            // [§ 17.2.1](https://www.w3.org/TR/CSS2/tables.html#table-display)
            //
            // "table-header-group (In HTML: THEAD), table-footer-group (TFOOT),
            // table-row-group (TBODY): These elements specify that an element
            // groups one or more rows."
            "thead" | "tbody" | "tfoot" => {
                for (row_idx, row_child) in child.children.iter().enumerate() {
                    let row_tag = row_child.tag_name.as_deref().unwrap_or("");
                    if row_tag == "tr" {
                        let cells = collect_cells_from_row(row_child);
                        rows.push(TableRow {
                            row_group_index: Some(child_idx),
                            row_index: row_idx,
                            cells,
                        });
                    }
                }
            }
            // Skip other children (<caption>, text nodes, etc.)
            _ => {}
        }
    }

    rows
}

/// Collect cell info from a `<tr>` element's children.
fn collect_cells_from_row(tr: &LayoutBox) -> Vec<CellInfo> {
    let mut cells = Vec::new();
    for (cell_idx, cell) in tr.children.iter().enumerate() {
        let tag = cell.tag_name.as_deref().unwrap_or("");
        if tag == "td" || tag == "th" {
            cells.push(CellInfo {
                cell_index: cell_idx,
                colspan: cell.colspan.max(1),
            });
        }
    }
    cells
}

/// [§ 17.5.2.1](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
///
/// Determine the column count: the maximum number of column slots across all
/// rows, accounting for colspan.
fn determine_column_count(rows: &[TableRow], container: &LayoutBox) -> usize {
    let mut max_cols: usize = 0;
    for row in rows {
        let mut col_slots: usize = 0;
        for cell_info in &row.cells {
            col_slots += cell_info.colspan as usize;
        }
        max_cols = max_cols.max(col_slots);
    }
    // Also check the table's width attribute hint — but for now, just use row data.
    let _ = container; // suppress unused warning
    max_cols
}

/// [§ 17.5.2.2 Column width determination](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
///
/// "Calculate the minimum and maximum width of each cell. ... For each
/// column, determine a minimum and maximum column width from the cells
/// that span only that column."
fn determine_column_widths(
    container: &LayoutBox,
    rows: &[TableRow],
    num_cols: usize,
    available_width: f32,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
) -> Vec<f32> {
    // Track max-content width for each column (from single-span cells).
    let mut col_max_widths = vec![0.0_f32; num_cols];

    for row in rows {
        let tr = get_tr(container, row);
        let mut col_cursor: usize = 0;

        for cell_info in &row.cells {
            let cell = &tr.children[cell_info.cell_index];
            let cell_content_width = cell.measure_content_size(viewport, font_metrics);

            // [§ 17.5.2.2](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
            //
            // "For each column, determine a minimum and maximum column width
            // from the cells that span only that column."
            if cell_info.colspan == 1 && col_cursor < num_cols {
                // Single-span cell: its content width contributes to this column.
                col_max_widths[col_cursor] =
                    col_max_widths[col_cursor].max(cell_content_width);

                // Also respect explicit cell width.
                if let Some(ref w) = cell.width {
                    let resolved =
                        super::values::UnresolvedAutoEdgeSizes::resolve_auto_length(
                            w, viewport, available_width,
                        );
                    if !resolved.is_auto() {
                        col_max_widths[col_cursor] =
                            col_max_widths[col_cursor].max(resolved.to_px_or(0.0));
                    }
                }
            }
            // Multi-span cells: distribute later (simplified: skip for now,
            // rely on the shrink-to-fit pass to handle them).

            col_cursor += cell_info.colspan as usize;
        }
    }

    // Total border-spacing on the horizontal axis.
    let total_border_spacing_x = BORDER_SPACING * (num_cols + 1) as f32;
    let max_content_width: f32 = col_max_widths.iter().sum::<f32>() + total_border_spacing_x;

    // Determine whether the table has an explicit width.
    let table_has_explicit_width = container.width.is_some()
        && !matches!(container.width, Some(AutoLength::Auto));

    if table_has_explicit_width {
        // [§ 17.5.2.2](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
        //
        // "If the table's width is given explicitly, distribute the available
        // space among columns proportionally."
        let space_for_columns = (available_width - total_border_spacing_x).max(0.0);

        if max_content_width - total_border_spacing_x <= 0.0 {
            // All columns are zero-width: distribute evenly.
            let per_col = space_for_columns / num_cols as f32;
            return vec![per_col; num_cols];
        }

        let content_sum: f32 = col_max_widths.iter().sum();
        if content_sum <= space_for_columns {
            // All max-content widths fit: distribute remaining space evenly.
            let excess = space_for_columns - content_sum;
            let bonus = excess / num_cols as f32;
            return col_max_widths.iter().map(|w| w + bonus).collect();
        }

        // Content overflows: scale proportionally.
        let scale = space_for_columns / content_sum;
        return col_max_widths.iter().map(|w| w * scale).collect();
    }

    // [§ 17.5.2.2](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
    //
    // "If the table's width is 'auto', use the max-content widths."
    // But clamp to the available width from the containing block.
    if max_content_width <= available_width {
        // Fits within the containing block: use max-content widths.
        return col_max_widths;
    }

    // Doesn't fit: scale down proportionally.
    let space_for_columns = (available_width - total_border_spacing_x).max(0.0);
    let content_sum: f32 = col_max_widths.iter().sum();
    if content_sum <= 0.0 {
        let per_col = space_for_columns / num_cols as f32;
        return vec![per_col; num_cols];
    }
    let scale = space_for_columns / content_sum;
    col_max_widths.iter().map(|w| w * scale).collect()
}

/// Layout each cell at its determined column width and measure row heights.
///
/// Returns a vector of row heights (one per row in `rows`).
fn layout_cells_and_measure_row_heights(
    container: &mut LayoutBox,
    rows: &[TableRow],
    column_widths: &[f32],
    num_cols: usize,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
    abs_cb: Rect,
) -> Vec<f32> {
    let mut row_heights = Vec::with_capacity(rows.len());

    for row in rows {
        let tr = get_tr_mut(container, row);
        let mut max_cell_height: f32 = 0.0;
        let mut col_cursor: usize = 0;

        for cell_info in &row.cells {
            if col_cursor >= num_cols {
                break;
            }

            let cell = &mut tr.children[cell_info.cell_index];

            // Calculate cell width from column widths + border-spacing for
            // multi-column spans.
            let span = (cell_info.colspan as usize).min(num_cols - col_cursor);
            let cell_width = cell_span_width(column_widths, col_cursor, span);

            // Override the cell's width with the resolved column width.
            cell.width = Some(AutoLength::Length(LengthValue::Px(
                f64::from(cell_width),
            )));

            // Layout with a temporary containing block to measure height.
            let temp_cb = Rect {
                x: 0.0,
                y: 0.0,
                width: cell_width,
                height: f32::MAX,
            };
            cell.layout(temp_cb, viewport, font_metrics, abs_cb);

            let cell_height = cell.dimensions.margin_box().height;
            max_cell_height = max_cell_height.max(cell_height);

            col_cursor += span;
        }

        row_heights.push(max_cell_height);
    }

    row_heights
}

/// Position all cells at their final coordinates.
///
/// [§ 17.5.2](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
///
/// "After the column widths have been calculated, cells are positioned
/// from left to right in each row, with border-spacing between them."
fn position_cells(
    container: &mut LayoutBox,
    rows: &[TableRow],
    column_widths: &[f32],
    row_heights: &[f32],
    content_box: Rect,
    viewport: Rect,
    font_metrics: &dyn FontMetrics,
    abs_cb: Rect,
) {
    let num_cols = column_widths.len();

    // Precompute column x-offsets (left edge of each column, relative to
    // the table content box).
    let col_offsets = compute_column_offsets(column_widths, content_box.x);

    let mut current_y = content_box.y + BORDER_SPACING;

    for (row_idx, row) in rows.iter().enumerate() {
        let tr = get_tr_mut(container, row);
        let row_height = row_heights[row_idx];
        let mut col_cursor: usize = 0;

        for cell_info in &row.cells {
            if col_cursor >= num_cols {
                break;
            }

            let cell = &mut tr.children[cell_info.cell_index];
            let span = (cell_info.colspan as usize).min(num_cols - col_cursor);
            let cell_width = cell_span_width(column_widths, col_cursor, span);
            let cell_x = col_offsets[col_cursor];

            // Override the cell's width for the final layout.
            cell.width = Some(AutoLength::Length(LengthValue::Px(
                f64::from(cell_width),
            )));

            // Layout the cell at its final position.
            let cell_cb = Rect {
                x: cell_x,
                y: current_y,
                width: cell_width,
                height: row_height,
            };
            cell.layout(cell_cb, viewport, font_metrics, abs_cb);

            col_cursor += span;
        }

        // Also set the <tr> box dimensions so painting traversal works.
        // The <tr> wraps all cells at this row's y-position.
        tr.dimensions.content.x = content_box.x;
        tr.dimensions.content.y = current_y;
        tr.dimensions.content.width = content_box.width;
        tr.dimensions.content.height = row_height;

        current_y += row_height + BORDER_SPACING;
    }

    // Also set row-group dimensions (<thead>, <tbody>, <tfoot>) to
    // encompass their rows, so painting traversal descends into them.
    set_row_group_dimensions(container, rows, row_heights, content_box);
}

/// Set dimensions on row-group elements (<thead>, <tbody>, <tfoot>) so
/// that the paint tree can find cells inside them.
fn set_row_group_dimensions(
    container: &mut LayoutBox,
    rows: &[TableRow],
    row_heights: &[f32],
    content_box: Rect,
) {
    // Collect row-group info: for each row-group child index, find the
    // min y and max y+height of its rows.
    use std::collections::HashMap;
    let mut group_bounds: HashMap<usize, (f32, f32)> = HashMap::new();

    for (row_idx, row) in rows.iter().enumerate() {
        if let Some(group_idx) = row.row_group_index {
            let tr = &container.children[group_idx].children[row.row_index];
            let row_y = tr.dimensions.content.y;
            let row_bottom = row_y + row_heights[row_idx];

            let entry = group_bounds
                .entry(group_idx)
                .or_insert((row_y, row_bottom));
            entry.0 = entry.0.min(row_y);
            entry.1 = entry.1.max(row_bottom);
        }
    }

    for (group_idx, (min_y, max_y)) in group_bounds {
        let group = &mut container.children[group_idx];
        group.dimensions.content.x = content_box.x;
        group.dimensions.content.y = min_y;
        group.dimensions.content.width = content_box.width;
        group.dimensions.content.height = max_y - min_y;
    }
}

/// Compute column x-offsets (left edge of each column).
///
/// Each column starts after border-spacing from the previous column's right
/// edge (or from the table content box left edge for the first column).
fn compute_column_offsets(column_widths: &[f32], start_x: f32) -> Vec<f32> {
    let mut offsets = Vec::with_capacity(column_widths.len());
    let mut x = start_x + BORDER_SPACING;
    for &w in column_widths {
        offsets.push(x);
        x += w + BORDER_SPACING;
    }
    offsets
}

/// Calculate the width of a cell that spans `span` columns starting at
/// `col_start`, including intervening border-spacing.
fn cell_span_width(column_widths: &[f32], col_start: usize, span: usize) -> f32 {
    let col_end = (col_start + span).min(column_widths.len());
    let mut width: f32 = 0.0;
    for i in col_start..col_end {
        width += column_widths[i];
    }
    // Add border-spacing between spanned columns (span-1 gaps).
    if span > 1 {
        width += BORDER_SPACING * (span - 1) as f32;
    }
    width
}

/// Get an immutable reference to the `<tr>` LayoutBox for a given row.
fn get_tr<'a>(container: &'a LayoutBox, row: &TableRow) -> &'a LayoutBox {
    if let Some(group_idx) = row.row_group_index {
        &container.children[group_idx].children[row.row_index]
    } else {
        &container.children[row.row_index]
    }
}

/// Get a mutable reference to the `<tr>` LayoutBox for a given row.
fn get_tr_mut<'a>(container: &'a mut LayoutBox, row: &TableRow) -> &'a mut LayoutBox {
    if let Some(group_idx) = row.row_group_index {
        &mut container.children[group_idx].children[row.row_index]
    } else {
        &mut container.children[row.row_index]
    }
}
