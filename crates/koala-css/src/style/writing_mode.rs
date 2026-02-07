//! CSS Writing Mode types and parsing
//!
//! [§ 2 Block Flow Direction](https://www.w3.org/TR/css-writing-modes-4/#block-flow)

use serde::Serialize;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;

/// [§ 2 Block Flow Direction](https://www.w3.org/TR/css-writing-modes-4/#block-flow)
///
/// "The writing-mode property specifies whether lines of text are laid out
/// horizontally or vertically and the direction in which blocks progress."
///
/// This property is essential for CSS Logical Properties, which map abstract
/// directions (block-start, inline-end, etc.) to physical directions based on
/// the writing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum WritingMode {
    /// [§ 2](https://www.w3.org/TR/css-writing-modes-4/#valdef-writing-mode-horizontal-tb)
    ///
    /// "Top-to-bottom block flow direction. Both the writing mode and the
    /// typographic mode are horizontal."
    ///
    /// Mapping:
    ///   - block-start  → top
    ///   - block-end    → bottom
    ///   - inline-start → left  (in ltr)
    ///   - inline-end   → right (in ltr)
    #[default]
    HorizontalTb,

    /// [§ 2](https://www.w3.org/TR/css-writing-modes-4/#valdef-writing-mode-vertical-rl)
    ///
    /// "Right-to-left block flow direction. Both the writing mode and the
    /// typographic mode are vertical."
    ///
    /// Mapping:
    ///   - block-start  → right
    ///   - block-end    → left
    ///   - inline-start → top    (in ltr)
    ///   - inline-end   → bottom (in ltr)
    VerticalRl,

    /// [§ 2](https://www.w3.org/TR/css-writing-modes-4/#valdef-writing-mode-vertical-lr)
    ///
    /// "Left-to-right block flow direction. Both the writing mode and the
    /// typographic mode are vertical."
    ///
    /// Mapping:
    ///   - block-start  → left
    ///   - block-end    → right
    ///   - inline-start → top    (in ltr)
    ///   - inline-end   → bottom (in ltr)
    VerticalLr,
}

/// Physical side of a box
///
/// Used to map logical directions (block-start, etc.) to physical sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalSide {
    /// Top edge of the box
    Top,
    /// Right edge of the box
    Right,
    /// Bottom edge of the box
    Bottom,
    /// Left edge of the box
    Left,
}

impl WritingMode {
    /// [§ 6.2 Flow-relative Directions](https://www.w3.org/TR/css-writing-modes-4/#logical-directions)
    ///
    /// Map block-start to the corresponding physical side.
    ///
    /// | Writing Mode   | block-start |
    /// |----------------|-------------|
    /// | horizontal-tb  | top         |
    /// | vertical-rl    | right       |
    /// | vertical-lr    | left        |
    #[must_use]
    pub const fn block_start_physical(&self) -> PhysicalSide {
        match self {
            Self::HorizontalTb => PhysicalSide::Top,
            Self::VerticalRl => PhysicalSide::Right,
            Self::VerticalLr => PhysicalSide::Left,
        }
    }

    /// Map block-end to the corresponding physical side.
    ///
    /// | Writing Mode   | block-end |
    /// |----------------|-----------|
    /// | horizontal-tb  | bottom    |
    /// | vertical-rl    | left      |
    /// | vertical-lr    | right     |
    #[must_use]
    pub const fn block_end_physical(&self) -> PhysicalSide {
        match self {
            Self::HorizontalTb => PhysicalSide::Bottom,
            Self::VerticalRl => PhysicalSide::Left,
            Self::VerticalLr => PhysicalSide::Right,
        }
    }

    // NOTE: inline-start and inline-end also depend on `direction` (ltr/rtl).
    // For now, we only implement block directions. Inline direction support
    // will require adding the `direction` property.
}

/// [§ 2 Block Flow Direction](https://www.w3.org/TR/css-writing-modes-4/#block-flow)
///
/// Parse a writing-mode value from component values.
///
/// Values:
///   - horizontal-tb: Top-to-bottom block flow (default)
///   - vertical-rl: Right-to-left block flow
///   - vertical-lr: Left-to-right block flow
#[must_use]
pub fn parse_writing_mode(values: &[ComponentValue]) -> Option<WritingMode> {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
            let lower = ident.to_ascii_lowercase();
            return match lower.as_str() {
                "horizontal-tb" => Some(WritingMode::HorizontalTb),
                "vertical-rl" => Some(WritingMode::VerticalRl),
                "vertical-lr" => Some(WritingMode::VerticalLr),
                _ => None,
            };
        }
    }
    None
}
