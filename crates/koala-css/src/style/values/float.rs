//! CSS float-related keyword values.
//!
//! - [CSS 2.1 § 9.5 `float`](https://www.w3.org/TR/CSS2/visuren.html#floats)
//! - [CSS 2.1 § 9.5.2 `clear`](https://www.w3.org/TR/CSS2/visuren.html#flow-control)

use serde::Serialize;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ClearSide {
    /// "Requires the top border edge be below any left-floating boxes."
    Left,
    /// "Requires the top border edge be below any right-floating boxes."
    Right,
    /// "Requires the top border edge be below any floating boxes."
    Both,
}
