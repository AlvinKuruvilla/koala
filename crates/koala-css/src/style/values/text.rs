//! Text-related CSS keyword values.
//!
//! - [CSS 2.1 § 16.2 `text-align`](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
//! - [CSS Text Decoration Level 3 § 3 `text-decoration-line`](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)

use serde::Serialize;

/// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
///
/// "This property describes how inline-level content of a block
/// container is aligned."
///
/// "Values have the following meanings:
///
/// left
///   Inline-level content is aligned to the left line edge.
///
/// right
///   Inline-level content is aligned to the right line edge.
///
/// center
///   Inline-level content is centered within the line box.
///
/// justify
///   Inline-level content is justified. Text should be spaced to line up
///   its left and right edges to the left and right edges of the line box,
///   except for the last line."
///
/// "The initial value is 'left' if 'direction' is 'ltr', and 'right' if
/// 'direction' is 'rtl'."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum TextAlign {
    /// "Inline-level content is aligned to the left line edge."
    #[default]
    Left,
    /// "Inline-level content is aligned to the right line edge."
    Right,
    /// "Inline-level content is centered within the line box."
    Center,
    /// "Inline-level content is justified."
    Justify,
}

/// [§ 3 Text Decoration Lines](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
///
/// "Specifies what line decorations, if any, are added to the element."
///
/// "Values: none | [ underline || overline || line-through ]"
///
/// Multiple values can be combined (e.g., `underline line-through`).
/// `Default` gives all `false` = `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct TextDecorationLine {
    /// "Each line of text has an underline."
    pub underline: bool,
    /// "Each line of text has a line over it (i.e., on the opposite side
    /// from an underline)."
    pub overline: bool,
    /// "Each line of text has a line through the middle."
    pub line_through: bool,
}
