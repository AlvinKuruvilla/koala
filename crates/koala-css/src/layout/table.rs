//! CSS Table Layout.
//!
//! [§ 17 Tables](https://www.w3.org/TR/CSS2/tables.html)

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
