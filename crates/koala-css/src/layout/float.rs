//! CSS Float Layout.
//!
//! [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
//!
//! "A float is a box that is shifted to the left or right on the current line.
//! The most interesting characteristic of a float is that content may flow along
//! its side (or be prohibited from doing so by the 'clear' property)."
//!
//! "A floated box is shifted to the left or right until its outer edge touches
//! the containing block edge or the outer edge of another float."

use super::box_model::Rect;

/// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
///
/// "Values have the following meanings:
///
/// left
///   The element generates a block box that is floated to the left.
///
/// right
///   The element generates a block box that is floated to the right.
///
/// none
///   The box is not floated."
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum FloatSide {
    /// "The element generates a block box that is floated to the left."
    Left,
    /// "The element generates a block box that is floated to the right."
    Right,
}

/// [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
///
/// "This property indicates which sides of an element's box(es) may not
/// be adjacent to an earlier floating box."
///
/// "Values have the following meanings:
///
/// left
///   Requires that the top border edge of the box be below the bottom
///   outer edge of any left-floating boxes.
///
/// right
///   Requires that the top border edge of the box be below the bottom
///   outer edge of any right-floating boxes.
///
/// both
///   Requires that the top border edge of the box be below the bottom
///   outer edge of any right-floating and left-floating boxes.
///
/// none
///   No constraint on the box's position with respect to floats."
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ClearSide {
    /// "Requires the top border edge be below any left-floating boxes."
    Left,
    /// "Requires the top border edge be below any right-floating boxes."
    Right,
    /// "Requires the top border edge be below any floating boxes."
    Both,
}

/// A single float that has been placed in the flow.
///
/// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
#[derive(Debug, Clone)]
pub struct PlacedFloat {
    /// Which side this float is on.
    pub side: FloatSide,
    /// The margin box of the float (absolute coordinates).
    pub margin_box: Rect,
}

/// Tracks placed floats within a block formatting context.
///
/// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
///
/// "Since a float is not in the flow, non-positioned block boxes created
/// before and after the float box flow vertically as if the float did not
/// exist. However, the current and subsequent line boxes created next to
/// the float are shortened as necessary to make room for the margin box
/// of the float."
pub struct FloatContext {
    /// All left floats that have been placed.
    pub left_floats: Vec<PlacedFloat>,
    /// All right floats that have been placed.
    pub right_floats: Vec<PlacedFloat>,
    /// Width of the containing block.
    pub containing_width: f32,
}

impl FloatContext {
    /// Create a new float context for a containing block.
    #[must_use]
    pub const fn new(containing_width: f32) -> Self {
        Self {
            left_floats: Vec::new(),
            right_floats: Vec::new(),
            containing_width,
        }
    }

    /// Returns true if there are no placed floats in this context.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.left_floats.is_empty() && self.right_floats.is_empty()
    }

    /// Return the maximum bottom edge of all placed floats.
    ///
    /// [§ 10.6.7](https://www.w3.org/TR/CSS2/visudet.html#root-height)
    ///
    /// "If the element has any floating descendants whose bottom margin edge
    /// is below the element's bottom content edge, then the height is
    /// increased to include those edges."
    #[must_use]
    pub fn max_float_bottom(&self) -> f32 {
        let left_max = self
            .left_floats
            .iter()
            .map(|f| f.margin_box.y + f.margin_box.height)
            .fold(0.0_f32, f32::max);
        let right_max = self
            .right_floats
            .iter()
            .map(|f| f.margin_box.y + f.margin_box.height)
            .fold(0.0_f32, f32::max);
        left_max.max(right_max)
    }

    /// [§ 9.5.1 Positioning the float: the 'float' property](https://www.w3.org/TR/CSS2/visuren.html#float-position)
    ///
    /// Place a float within this context.
    ///
    /// "A floated box is shifted to the left or right until its outer edge
    /// touches the containing block edge or the outer edge of another float."
    ///
    /// The spec defines 9 precise rules for float placement. This
    /// implementation covers the core behavior:
    ///
    /// - Rule 1: Float cannot extend past containing block edges.
    /// - Rules 4, 5, 8: Float is placed as high as possible (at or below `current_y`).
    /// - Rule 9: Left floats go as far left as possible; right floats as far right.
    /// - Rules 2, 3, 7: Floats do not overlap each other.
    pub fn place_float(
        &mut self,
        side: FloatSide,
        box_width: f32,
        box_height: f32,
        current_y: f32,
    ) -> Rect {
        // STEP 1: Start at the highest allowed position.
        // [§ 9.5.1 Rule 4](https://www.w3.org/TR/CSS2/visuren.html#float-position)
        //
        // "A floating box's outer top may not be higher than the top of
        // its containing block."
        //
        // [§ 9.5.1 Rule 8](https://www.w3.org/TR/CSS2/visuren.html#float-position)
        //
        // "A floating box must be placed as high as possible."
        let mut y = current_y.max(0.0);

        // STEP 2: Find a position where the float fits.
        // [§ 9.5.1 Rules 2, 3, 7](https://www.w3.org/TR/CSS2/visuren.html#float-position)
        //
        // The float must not overlap other floats. Scan downward until
        // available width at the candidate Y is sufficient.
        loop {
            let (left_offset, avail_width) = self.available_width_at(y, box_height);

            if avail_width >= box_width || avail_width >= self.containing_width {
                // STEP 3: Place the float.
                // [§ 9.5.1 Rule 9](https://www.w3.org/TR/CSS2/visuren.html#float-position)
                //
                // "A left-floating box must be put as far to the left as possible,
                // a right-floating box as far to the right as possible."
                let x = match side {
                    // [§ 9.5.1 Rule 1](https://www.w3.org/TR/CSS2/visuren.html#float-position)
                    //
                    // "The left outer edge of a left-floating box may not be to
                    // the left of the left edge of its containing block."
                    FloatSide::Left => left_offset,
                    // "An analogous rule holds for right-floating elements."
                    FloatSide::Right => (left_offset + avail_width - box_width).max(0.0),
                };

                let rect = Rect {
                    x,
                    y,
                    width: box_width,
                    height: box_height,
                };

                // Record the placed float.
                let placed = PlacedFloat {
                    side,
                    margin_box: rect,
                };
                match side {
                    FloatSide::Left => self.left_floats.push(placed),
                    FloatSide::Right => self.right_floats.push(placed),
                }

                return rect;
            }

            // STEP 4: Float doesn't fit at this Y — advance to the next
            // float bottom edge. This is more efficient than stepping 1px
            // at a time.
            let next_y = self.next_float_bottom_after(y);
            if next_y <= y {
                // No more floats below — place at the current Y even if
                // it doesn't fit (the float will overflow the containing block).
                let x = match side {
                    FloatSide::Left => left_offset,
                    FloatSide::Right => (left_offset + avail_width - box_width).max(0.0),
                };

                let rect = Rect {
                    x,
                    y,
                    width: box_width,
                    height: box_height,
                };

                let placed = PlacedFloat {
                    side,
                    margin_box: rect,
                };
                match side {
                    FloatSide::Left => self.left_floats.push(placed),
                    FloatSide::Right => self.right_floats.push(placed),
                }

                return rect;
            }

            y = next_y;
        }
    }

    /// [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
    ///
    /// "This property indicates which sides of an element's box(es) may not
    /// be adjacent to an earlier floating box."
    ///
    /// Returns the Y position that the element should be moved to in order
    /// to clear past the relevant floats.
    #[must_use]
    pub fn clear(&self, clear_side: ClearSide, current_y: f32) -> f32 {
        // STEP 1: Find the lowest float bottom on the cleared side(s).
        // [§ 9.5.2](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
        //
        // "For clear: left → below bottom edge of all left floats"
        // "For clear: right → below bottom edge of all right floats"
        // "For clear: both → below bottom edge of all floats"
        let mut cleared_y = current_y;

        if matches!(clear_side, ClearSide::Left | ClearSide::Both) {
            for f in &self.left_floats {
                let bottom = f.margin_box.y + f.margin_box.height;
                if bottom > cleared_y {
                    cleared_y = bottom;
                }
            }
        }

        if matches!(clear_side, ClearSide::Right | ClearSide::Both) {
            for f in &self.right_floats {
                let bottom = f.margin_box.y + f.margin_box.height;
                if bottom > cleared_y {
                    cleared_y = bottom;
                }
            }
        }

        // STEP 2: Return the new Y position (at least current_y).
        cleared_y
    }

    /// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
    ///
    /// "The current and subsequent line boxes created next to the float are
    /// shortened as necessary to make room for the margin box of the float."
    ///
    /// Returns `(left_offset, available_width)` for content at a given Y
    /// position, accounting for floats on both sides.
    ///
    /// A float is "active" at the band `[y, y+height)` if its margin box
    /// vertically overlaps that band.
    #[must_use]
    pub fn available_width_at(&self, y: f32, height: f32) -> (f32, f32) {
        let band_top = y;
        let band_bottom = y + height;

        // STEP 1: Find the rightmost right-edge of active left floats.
        // [§ 9.5](https://www.w3.org/TR/CSS2/visuren.html#floats)
        //
        // "line boxes created next to the float are shortened as necessary
        // to make room for the margin box of the float."
        let mut left_edge: f32 = 0.0;
        for f in &self.left_floats {
            let f_top = f.margin_box.y;
            let f_bottom = f_top + f.margin_box.height;
            // Overlap test: float active if bands intersect
            if f_top < band_bottom && f_bottom > band_top {
                let right = f.margin_box.x + f.margin_box.width;
                if right > left_edge {
                    left_edge = right;
                }
            }
        }

        // STEP 2: Find the leftmost left-edge of active right floats.
        let mut right_edge: f32 = self.containing_width;
        for f in &self.right_floats {
            let f_top = f.margin_box.y;
            let f_bottom = f_top + f.margin_box.height;
            if f_top < band_bottom && f_bottom > band_top && f.margin_box.x < right_edge {
                right_edge = f.margin_box.x;
            }
        }

        // STEP 3: Return (left_offset, available_width).
        let avail = (right_edge - left_edge).max(0.0);
        (left_edge, avail)
    }

    /// Find the smallest float bottom edge that is strictly greater than `y`.
    ///
    /// Used by `place_float()` to efficiently scan downward when a float
    /// doesn't fit at the current Y position.
    fn next_float_bottom_after(&self, y: f32) -> f32 {
        let mut next = f32::MAX;
        for f in self.left_floats.iter().chain(self.right_floats.iter()) {
            let bottom = f.margin_box.y + f.margin_box.height;
            if bottom > y && bottom < next {
                next = bottom;
            }
        }
        if next == f32::MAX {
            y // No float bottom found above y
        } else {
            next
        }
    }
}
