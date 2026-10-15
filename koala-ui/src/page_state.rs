// PageState — a loaded page as the render worker keeps it. The loader
// worker builds one; the render worker owns it from then on.

use koala_browser::css::cascade::state_dependencies;
use koala_browser::css::selector::ElementState;
use koala_browser::css::{ComputedStyle, LayoutBox, Stylesheet};
use koala_browser::dom::{DomTree, NodeId};
use koala_browser::inline_svg::InlineSvg;
use koala_browser::{LoadedDocument, LoadedImage};

/// The Send-able subset of `LoadedDocument` needed to render a page
/// and to restyle it when the hovered node changes.
///
/// Excludes the JS runtime, the raw token stream, the HTML source,
/// and parse diagnostics — nothing in that list is consulted after
/// layout. Built on the loader thread and moved to the render worker,
/// which owns it from then on.
pub(crate) struct PageState {
    pub(crate) dom: DomTree,
    pub(crate) stylesheet: Stylesheet,
    pub(crate) styles: koala_std::collections::HashMap<NodeId, ComputedStyle>,
    pub(crate) layout_tree: LayoutBox,
    pub(crate) images: koala_std::collections::HashMap<String, LoadedImage>,
    pub(crate) image_dims: koala_std::collections::HashMap<NodeId, (f32, f32)>,
    pub(crate) inline_svgs: koala_std::collections::HashMap<NodeId, std::sync::Arc<InlineSvg>>,
    // The interaction state any selector reads. A hover change restyles
    // only when this contains `ElementState::HOVER`.
    pub(crate) state_dependencies: ElementState,
    // What links on the page resolve against.
    pub(crate) base_url: Option<String>,
    // The document's `<title>` text content, trimmed. Empty when
    // the document has no `<title>` element or its text is
    // whitespace-only. Used to set the tab label.
    pub(crate) title: String,
}

impl PageState {
    /// Destructure a freshly-parsed `LoadedDocument` into the
    /// render-relevant fields, dropping the JS runtime and other
    /// parse-time debris. Returns `None` if the document produced
    /// no layout tree (happens only on pathological input).
    pub(crate) fn from_document(doc: LoadedDocument) -> Option<Box<Self>> {
        let title = extract_title(&doc.dom);
        let state_dependencies =
            state_dependencies(koala_browser::css::ua_stylesheet::ua_stylesheet(), &doc.stylesheet);
        doc.layout_tree.map(|layout_tree| {
            Box::new(Self {
                dom: doc.dom,
                stylesheet: doc.stylesheet,
                styles: doc.styles,
                layout_tree,
                images: doc.images,
                image_dims: doc.image_dims,
                inline_svgs: doc.inline_svgs,
                state_dependencies,
                base_url: doc.base_url,
                title,
            })
        })
    }
}

/// Walks the DOM for the first `<title>` element and returns its
/// text content, trimmed. Returns an empty string when no `<title>`
/// element is present. Matches the idiom koala-browser uses for
/// `<script>` text extraction.
fn extract_title(dom: &DomTree) -> String {
    for node_id in dom.iter_all() {
        let Some(element) = dom.as_element(node_id) else {
            continue;
        };
        if !element.tag_name.eq_ignore_ascii_case("title") {
            continue;
        }
        let mut text = String::new();
        for child_id in dom.children(node_id) {
            if let Some(child_text) = dom.as_text(*child_id) {
                text.push_str(child_text);
            }
        }
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    String::new()
}
