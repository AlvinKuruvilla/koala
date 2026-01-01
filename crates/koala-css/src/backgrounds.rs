//! CSS Backgrounds and Borders Level 3
//!
//! [CSS Backgrounds and Borders Module Level 3](https://www.w3.org/TR/css-backgrounds-3/)
//!
//! This module implements background-related computations, including the special
//! rules for canvas background propagation.

use std::collections::HashMap;

use koala_dom::{DomTree, NodeId};

use crate::style::{ColorValue, ComputedStyle};

/// [§ 2.11.2 The Canvas Background and the HTML `<body>` Element](https://www.w3.org/TR/css-backgrounds-3/#special-backgrounds)
///
/// "The background of the root element becomes the canvas background and its
/// background painting area extends to cover the entire canvas."
///
/// "For documents whose root element is an HTML `html` element or an XHTML `html`
/// element: if the computed value of background-image on the root element is
/// `none` and its background-color is `transparent`, user agents must instead
/// propagate the computed values of the background properties from that
/// element's first HTML `body` child element."
///
/// "The used value of that body element's background is then `transparent`."
#[must_use]
pub fn canvas_background(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> Option<ColorValue> {
    // "The background of the root element becomes the canvas background"
    let html_id = tree.document_element()?;
    let html_style = styles.get(&html_id);

    // Check if root element has a background-color set
    // NOTE: We don't support background-image yet, so we only check background-color.
    // Per spec, we should also check "background-image is none", which we treat as
    // the default when unset.
    if let Some(style) = html_style {
        if style.background_color.is_some() {
            // Root has background, use it
            return style.background_color.clone();
        }
    }

    // "if the computed value of background-image on the root element is none
    // and its background-color is transparent, user agents must instead
    // propagate the computed values of the background properties from that
    // element's first HTML body child element."
    let body_id = tree.body()?;
    styles.get(&body_id)?.background_color.clone()
}
