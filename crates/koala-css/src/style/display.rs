//! CSS Display property types and parsing
//!
//! [§ 2 Box Layout Modes: the display property](https://www.w3.org/TR/css-display-3/#the-display-properties)

use serde::Serialize;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;
use koala_common::warning::warn_once;

// [§ 2 Box Layout Modes: the display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
//
// "The display property defines an element's display type, which consists of
// the two basic qualities of how an element generates boxes:
//   - the inner display type, which defines the kind of formatting context
//     it generates, dictating how its descendant boxes are laid out.
//   - the outer display type, which dictates how the principal box itself
//     participates in flow layout."

/// [§ 2.1 Outer Display Roles](https://www.w3.org/TR/css-display-3/#outer-role)
///
/// "The `<display-outside>` keywords specify the element's outer display type,
/// which is essentially its principal box's role in flow layout."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OuterDisplayType {
    /// "The element generates a block-level box when placed in flow layout."
    Block,
    /// "The element generates an inline-level box when placed in flow layout."
    Inline,
    /// "The element generates a run-in box, which is a type of inline-level box."
    RunIn,
}

/// [§ 2.2 Inner Display Layout Models](https://www.w3.org/TR/css-display-3/#inner-model)
///
/// "The `<display-inside>` keywords specify the element's inner display type,
/// which defines the type of formatting context that lays out its contents."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InnerDisplayType {
    /// "The element lays out its contents using flow layout (block-and-inline layout)."
    Flow,
    /// "The element lays out its contents using flow layout (block-and-inline layout)."
    /// Same as Flow but establishes a new block formatting context.
    FlowRoot,
    /// "The element lays out its contents using table layout."
    Table,
    /// "The element lays out its contents using flex layout."
    Flex,
    /// "The element lays out its contents using grid layout."
    Grid,
}

/// Combined display value
/// [§ 2 Box Layout Modes](https://www.w3.org/TR/css-display-3/#the-display-properties)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DisplayValue {
    /// "The outer display type, which dictates how the box participates in flow layout."
    pub outer: OuterDisplayType,
    /// "The inner display type, which dictates how its descendant boxes are laid out."
    pub inner: InnerDisplayType,
}

impl DisplayValue {
    /// `display: block` - block outer, flow inner
    #[must_use]
    pub const fn block() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Flow,
        }
    }

    /// `display: inline` - inline outer, flow inner
    #[must_use]
    pub const fn inline() -> Self {
        Self {
            outer: OuterDisplayType::Inline,
            inner: InnerDisplayType::Flow,
        }
    }

    /// `display: inline-block` - inline outer, flow-root inner
    #[must_use]
    pub const fn inline_block() -> Self {
        Self {
            outer: OuterDisplayType::Inline,
            inner: InnerDisplayType::FlowRoot,
        }
    }

    /// `display: flex` - block outer, flex inner
    #[must_use]
    pub const fn flex() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Flex,
        }
    }

    /// `display: grid` - block outer, grid inner
    #[must_use]
    pub const fn grid() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Grid,
        }
    }
}

/// [§ 2 The display property](https://www.w3.org/TR/css-display-3/#the-display-properties)
///
/// Parse a display value from component values.
/// Returns None if the value is "none" or unrecognized (use `is_display_none` for "none").
#[must_use]
pub fn parse_display_value(values: &[ComponentValue]) -> Option<DisplayValue> {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v {
            let lower = ident.to_ascii_lowercase();
            match lower.as_str() {
                // [§ 2.1 Outer Display Roles]
                // "block: The element generates a block-level box."
                "block" => return Some(DisplayValue::block()),

                // "inline: The element generates an inline-level box."
                "inline" => return Some(DisplayValue::inline()),

                // [§ 2.4 Combination Display Keywords]
                // "inline-block: This value causes an element to generate an inline-level
                // block container."
                "inline-block" => return Some(DisplayValue::inline_block()),

                // [§ 2.2 Inner Display Layout Models]
                // "flex: The element generates a principal flex container box."
                "flex" => return Some(DisplayValue::flex()),

                // "grid: The element generates a principal grid container box."
                "grid" => return Some(DisplayValue::grid()),

                // "none" is handled separately by is_display_none
                "none" => return None,

                _ => {
                    warn_once("CSS", &format!("unsupported display value '{ident}'"));
                }
            }
        }
    }
    None
}

/// [§ 2.6 display: none](https://www.w3.org/TR/css-display-3/#valdef-display-none)
///
/// Check if the display value is "none".
#[must_use]
pub fn is_display_none(values: &[ComponentValue]) -> bool {
    for v in values {
        if let ComponentValue::Token(CSSToken::Ident(ident)) = v
            && ident.eq_ignore_ascii_case("none")
        {
            return true;
        }
    }
    false
}
