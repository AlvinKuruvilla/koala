//! CSS `position` property keyword values.
//!
//! [CSS 2.1 § 9.3.1 `position`](https://www.w3.org/TR/CSS2/visuren.html#choose-position)

use serde::Serialize;

/// [§ 9.3.1 Choosing a positioning scheme: 'position' property](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
///
/// "The 'position' and 'float' properties determine which of the CSS 2
/// positioning algorithms is used to calculate the position of a box."
///
/// "Values have the following meanings:
///
/// static
///   The box is a normal box, laid out according to the normal flow. The
///   'top', 'right', 'bottom', and 'left' properties do not apply.
///
/// relative
///   The box's position is calculated according to the normal flow. Then
///   the box is offset relative to its normal position.
///
/// absolute
///   The box's position (and possibly size) is specified with the 'top',
///   'right', 'bottom', and 'left' properties. These properties specify
///   offsets with respect to the box's containing block.
///
/// fixed
///   The box's position is calculated according to the 'absolute' model,
///   but in addition, the box is fixed with respect to some reference.
///
/// sticky (CSS Positioned Layout Module Level 3)
///   The box's position is calculated according to the normal flow, then
///   offset relative to its nearest scrolling ancestor."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum PositionType {
    /// "The box is a normal box, laid out according to the normal flow."
    #[default]
    Static,
    /// "The box's position is calculated according to the normal flow.
    /// Then the box is offset relative to its normal position."
    Relative,
    /// "The box's position (and possibly size) is specified with the
    /// 'top', 'right', 'bottom', and 'left' properties."
    Absolute,
    /// "The box's position is calculated according to the 'absolute' model,
    /// but the box is fixed with respect to some reference."
    Fixed,
    /// [CSS Positioned Layout Module Level 3 § 3.2](https://www.w3.org/TR/css-position-3/#sticky-position)
    ///
    /// "A stickily positioned box is positioned similarly to a relatively
    /// positioned box, but the offset is computed with reference to the
    /// nearest ancestor with a scrolling mechanism."
    Sticky,
}
