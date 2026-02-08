//! CSS Layout Engine
//!
//! This module implements the CSS Visual Formatting Model for laying out
//! the render tree.
//!
//! # Relevant Specifications
//!
//! - [CSS Display Module Level 3](https://www.w3.org/TR/css-display-3/)
//! - [CSS Box Model Module Level 3](https://www.w3.org/TR/css-box-3/)
//! - [CSS 2.1 Visual Formatting Model](https://www.w3.org/TR/CSS2/visuren.html)
//! - [CSS Positioned Layout Module Level 3](https://www.w3.org/TR/css-position-3/)
//! - [CSS Text Module Level 3](https://www.w3.org/TR/css-text-3/)
//!
//! # Module Structure
//!
//! - [`box_model`] - Box dimensions, rectangles, and edge sizes
//! - [`values`] - Unresolved and auto value types
//! - [`formatting_context`] - Block and inline formatting contexts
//! - [`layout_box`] - Layout box types and layout algorithms
//! - [`inline`] - Inline formatting context with line box model
//! - [`positioned`] - Positioned layout (relative, absolute, fixed, sticky)
//! - [`float`] - Float layout and clearance
//! - [`stacking`] - Stacking contexts and painting order
//! - [`table`] - Table layout algorithm

pub mod box_model;
pub mod flex;
pub mod float;
pub mod formatting_context;
pub mod inline;
pub mod layout_box;
pub mod positioned;
pub mod stacking;
pub mod table;
pub mod values;

// Re-exports for convenience
pub use box_model::{BoxDimensions, EdgeSizes, Rect};
pub use float::{ClearSide, FloatContext, FloatSide};
pub use formatting_context::{BlockFormattingContext, InlineFormattingContext};
pub use inline::{
    ApproximateFontMetrics, FontMetrics, FontStyle, FragmentContent, InlineLayout, LineBox,
    LineFragment, TextAlign, TextRun, VerticalAlign,
};
pub use layout_box::{BoxType, LayoutBox};
pub use positioned::{BoxOffsets, PositionType, PositionedLayout};
pub use stacking::{StackingContext, ZIndex};
pub use table::TableLayout;
pub use values::{AutoEdgeSizes, AutoOr, UnresolvedAutoEdgeSizes, UnresolvedEdgeSizes};

use crate::style::DisplayValue;

// [HTML Living Standard § 15 Rendering](https://html.spec.whatwg.org/multipage/rendering.html)
// defines the default CSS styles for HTML elements.

/// Returns the default display value for an HTML element.
///
/// [§ 15.3.1 Hidden elements](https://html.spec.whatwg.org/multipage/rendering.html#hidden-elements)
/// [§ 15.3.2 The page](https://html.spec.whatwg.org/multipage/rendering.html#the-page)
/// [§ 15.3.3 Flow content](https://html.spec.whatwg.org/multipage/rendering.html#flow-content-3)
#[must_use]
pub fn default_display_for_element(tag_name: &str) -> Option<DisplayValue> {
    // [§ 15.3.1 Hidden elements]
    // "The following elements must have their display set to none:"
    // area, base, basefont, datalist, head, link, meta, noembed,
    // noframes, param, rp, script, style, template, title
    let hidden = [
        "area", "base", "basefont", "datalist", "head", "link", "meta", "noembed", "noframes",
        "param", "rp", "script", "style", "template", "title",
    ];
    if hidden.contains(&tag_name) {
        return None; // display: none
    }

    // [§ 15.3.3 Flow content]
    // Block-level elements by default
    let block_elements = [
        "address",
        "article",
        "aside",
        "blockquote",
        "body",
        "center",
        "dd",
        "details",
        "dialog",
        "dir",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hgroup",
        "hr",
        "html",
        "legend",
        "listing",
        "main",
        "menu",
        "nav",
        "ol",
        "p",
        "plaintext",
        "pre",
        "search",
        "section",
        "summary",
        "ul",
        "xmp",
    ];
    if block_elements.contains(&tag_name) {
        return Some(DisplayValue::block());
    }

    // [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists)
    // "li { display: list-item; }"
    if tag_name == "li" {
        return Some(DisplayValue::list_item());
    }

    // [§ 15.5.12 The input element](https://html.spec.whatwg.org/multipage/rendering.html#the-input-element-as-a-form-control)
    // [§ 15.5.13 The button element](https://html.spec.whatwg.org/multipage/rendering.html#the-button-element)
    // [§ 15.5.14 The textarea element](https://html.spec.whatwg.org/multipage/rendering.html#the-textarea-element)
    // [§ 15.5.15 The select element](https://html.spec.whatwg.org/multipage/rendering.html#the-select-element)
    //
    // Form controls are inline-block by default.
    if matches!(tag_name, "input" | "button" | "textarea" | "select") {
        return Some(DisplayValue::inline_block());
    }

    // Inline elements (default)
    // a, abbr, acronym, b, bdi, bdo, big, br, cite, code, data, del, dfn,
    // em, font, i, img, ins, kbd, label, mark, nobr, q, ruby, s, samp,
    // small, span, strike, strong, sub, sup, time, tt, u, var, wbr
    Some(DisplayValue::inline())
}
