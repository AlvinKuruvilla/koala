//! CSS Display property types and parsing
//!
//! [§ 2 Box Layout Modes: the display property](https://www.w3.org/TR/css-display-3/#the-display-properties)

use serde::Serialize;

use crate::parser::ComponentValue;
use crate::tokenizer::CSSToken;
use koala_common::diagnostics::{self, Diagnostic};

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
    /// [§ 2.5 List Items](https://www.w3.org/TR/css-display-3/#list-items)
    ///
    /// "An element with `display: list-item` generates a principal block box
    /// for its content and, optionally, a marker box."
    ///
    /// In flow layout, list-item behaves like block but additionally generates
    /// marker content (bullets, numbers, etc.).
    ListItem,
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

    /// [§ 2.7 Automatic Box Type Transformations](https://www.w3.org/TR/css-display-3/#transformations)
    ///
    /// "Some layout effects require blockification or inlinification of the
    /// box type, which sets the box's computed outer display type to block
    /// or inline (respectively)."
    ///
    /// "For legacy reasons, if an inline block box (inline flow-root) is
    /// blockified, it becomes a block box (losing its flow-root nature). For
    /// consistency, a run-in flow-root box also blockifies to a block box."
    ///
    /// "If a layout-internal box is blockified, its inner display type
    /// converts to flow so that it becomes a block container." Koala has no
    /// layout-internal values by this point: `parse_display_value` already
    /// maps them to `block`.
    ///
    /// A list item is already block-level and is left alone. So
    /// `inline-flex` becomes `flex` and `inline-grid` becomes `grid`, keeping
    /// their inner type.
    #[must_use]
    pub const fn blockified(self) -> Self {
        match (self.outer, self.inner) {
            (OuterDisplayType::Inline | OuterDisplayType::RunIn, InnerDisplayType::FlowRoot) => {
                Self::block()
            }
            (OuterDisplayType::Inline | OuterDisplayType::RunIn, inner) => Self {
                outer: OuterDisplayType::Block,
                inner,
            },
            (OuterDisplayType::Block | OuterDisplayType::ListItem, _) => self,
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

    /// `display: table` - block outer, table inner
    ///
    /// [§ 17.2 The CSS table model](https://www.w3.org/TR/CSS2/tables.html#table-display)
    ///
    /// "Specifies that an element defines a block-level table."
    #[must_use]
    pub const fn table() -> Self {
        Self {
            outer: OuterDisplayType::Block,
            inner: InnerDisplayType::Table,
        }
    }

    /// `display: inline-table` - inline outer, table inner
    ///
    /// [§ 17.2 The CSS table model](https://www.w3.org/TR/CSS2/tables.html#table-display)
    ///
    /// "Specifies that an element defines an inline-level table."
    #[must_use]
    pub const fn inline_table() -> Self {
        Self {
            outer: OuterDisplayType::Inline,
            inner: InnerDisplayType::Table,
        }
    }

    /// `display: list-item` - list-item outer, flow inner
    ///
    /// [§ 2.5 List Items](https://www.w3.org/TR/css-display-3/#list-items)
    ///
    /// "An element with `display: list-item` generates a `::marker` pseudo-element."
    #[must_use]
    pub const fn list_item() -> Self {
        Self {
            outer: OuterDisplayType::ListItem,
            inner: InnerDisplayType::Flow,
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
                "inline-block" | "-webkit-inline-box" | "inline-flex" => return Some(DisplayValue::inline_block()),

                // [§ 2.2 Inner Display Layout Models]
                // "flex: The element generates a principal flex container box."
                "flex" | "-webkit-flex" | "-webkit-box" => return Some(DisplayValue::flex()),

                // "grid: The element generates a principal grid container box."
                "grid" => return Some(DisplayValue::grid()),

                // [§ 2.5 List Items](https://www.w3.org/TR/css-display-3/#list-items)
                // "list-item: The element generates a ::marker pseudo-element."
                "list-item" => return Some(DisplayValue::list_item()),

                // [§ 17.2 The CSS table model](https://www.w3.org/TR/CSS2/tables.html#table-display)
                //
                // "table: Specifies that an element defines a block-level table."
                "table" => return Some(DisplayValue::table()),

                // "inline-table: Specifies that an element defines an inline-level table."
                "inline-table" => return Some(DisplayValue::inline_table()),

                // Table internal display values.
                // [§ 17.2](https://www.w3.org/TR/CSS2/tables.html#table-display)
                //
                // These are "internal" table display types that don't have a
                // simple outer/inner decomposition. They require proper table
                // layout (§ 17) to be meaningful. For now, map them to block/flow
                // so they participate in layout without triggering warnings.
                // TODO: Implement proper table layout for these internal types.
                "table-row-group" | "table-header-group" | "table-footer-group" | "table-row"
                | "table-column-group" | "table-column" | "table-cell" | "table-caption" => {
                    return Some(DisplayValue::block());
                }

                // "none" is handled separately by is_display_none
                "none" => return None,

                // [§ 2.5 Box Generation](https://www.w3.org/TR/css-display-3/#box-generation)
                //
                // "contents: The element itself does not generate any boxes,
                // but its children and pseudo-elements still generate boxes
                // and text runs as normal."
                //
                // TODO: implement. Dropping the value leaves the element
                // with its default box, so its children lay out inside it
                // rather than in its parent. Code that looks for "the
                // nearest ancestor element (skipping display:contents
                // ancestors)" treats the parent as that ancestor; search for
                // `DisplayContentsNotSupported` to find those sites.
                "contents" => {
                    diagnostics::report(|| Diagnostic::DisplayContentsNotSupported);
                }

                _ => {
                    diagnostics::report(|| Diagnostic::UnsupportedDisplay {
                        value: ident.clone(),
                    });
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
