//! The font-wide metrics a face reports, in font units.

use crate::types::fword::FWord;

/// Vertical metrics of a face, in font units: distances above (positive)
/// and below (negative) the baseline, and the gap between lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerticalMetrics {
    /// Distance from the baseline to the top of the face's line.
    pub ascender: FWord,
    /// Distance from the baseline to the bottom of the face's line;
    /// negative below the baseline.
    pub descender: FWord,
    /// Recommended extra space between lines.
    pub line_gap: FWord,
}

/// A decoration line's placement, in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationLine {
    /// Position relative to the baseline, positive upward. See the
    /// `post` and `OS/2` field descriptions for which edge of the line
    /// it gives.
    pub position: FWord,
    /// The line's thickness.
    pub thickness: FWord,
}
