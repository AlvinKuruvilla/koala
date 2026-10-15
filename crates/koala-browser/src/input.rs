//! What pointer input does to a document: which element is hovered, which
//! cursor shows, and where a click on a link goes.
//!
//! The caller owns the pointer and the layout. It hit tests a point with
//! [`LayoutBox::hit_test`](koala_css::LayoutBox::hit_test) and hands the
//! result here, where the rules the HTML, DOM, CSS UI and Pointer Events
//! specifications give for that point are applied:
//!
//! ```text
//!   pointer moves ──► hit_test ──► hovered_node ──► DomTree::set_hovered
//!                               └► cursor_at   ──► the platform cursor
//!   press, release ─► hit_test ×2 ──► click ──► URL to navigate to
//! ```
//!
//! Nothing here mutates the document or dispatches DOM events: there are
//! no script-visible `click` events, so a page's own click handlers do not
//! run.
//!
//! # Trade-offs
//!
//! - **Only links activate**: `a` and `area` are the only elements given
//!   activation behavior. A click on a `<button>` inside a link follows
//!   the link, where a browser would stop at the button.
//! - **`target` is ignored**: every link opens in the page it is on, so
//!   `target="_blank"` does not open a new one.
//! - **No `<base href>`**: links resolve against the document's own URL.

use koala_common::url::resolve_url;
use koala_css::{ComputedStyle, Cursor, ElementBoxId, HitTestResult};
use koala_css::selector::PseudoElement;
use koala_dom::{DomTree, Namespace, NodeId};
use koala_std::collections::HashMap;

/// The node the pointer designates, given what the hit test found there.
///
/// [CSSOM View § 5 elementFromPoint()](https://drafts.csswg.org/cssom-view/#dom-document-elementfrompoint)
///
/// "If there is a box in the viewport that would be a target for hit
/// testing at coordinates x,y, ... return the associated element and
/// terminate these steps."
///
/// "If the document has a root element, return the root element and
/// terminate these steps."
#[must_use]
pub fn hovered_node(dom: &DomTree, hit: Option<HitTestResult>) -> Option<NodeId> {
    hit.map(|hit| hit.target.node).or_else(|| dom.document_element())
}

/// The cursor to show at a point, given what the hit test found there.
///
/// [CSS UI 4 § 5.1.1](https://drafts.csswg.org/css-ui-4/#cursor)
///
/// "In the case of overlapping elements, which element determines the type
/// of cursor is based on hit testing: the element determining the cursor
/// is the one that would receive a click initiated from this position."
///
/// With no hit, that is the root element, as for [`hovered_node`].
#[must_use]
pub fn cursor_at(
    dom: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    hit: Option<HitTestResult>,
) -> Cursor {
    let (style, over_text) = match hit {
        Some(hit) => (style_of(styles, hit.target), hit.over_text),
        None => (dom.document_element().and_then(|root| styles.get(&root)), false),
    };
    match style.and_then(|style| style.cursor).unwrap_or_default() {
        // "auto behaves as text over selectable text or editable elements,
        // and default otherwise."
        //
        // NOTE: Koala has no `user-select` and no editing, so all text is
        // selectable and no element is editable.
        Cursor::Auto if over_text => Cursor::Text,
        Cursor::Auto => Cursor::Default,
        cursor => cursor,
    }
}

/// The computed style of the element or pseudo-element `id` names.
fn style_of(styles: &HashMap<NodeId, ComputedStyle>, id: ElementBoxId) -> Option<&ComputedStyle> {
    let element = styles.get(&id.node)?;
    match id.pseudo_element {
        None => Some(element),
        Some(PseudoElement::Before) => element.before.as_deref(),
        Some(PseudoElement::After) => element.after.as_deref(),
    }
}

/// The URL a click navigates to, given the elements the primary button
/// was pressed and released over, or `None` if the click does not
/// navigate.
///
/// `base_url` is the URL the document's relative references resolve
/// against.
#[must_use]
pub fn click(dom: &DomTree, pressed: NodeId, released: NodeId, base_url: Option<&str>) -> Option<String> {
    // [Pointer Events 4 § 3.3.12 click, auxclick, and contextmenu](https://www.w3.org/TR/pointerevents4/#x3-3-12-click-auxclick-and-contextmenu)
    //
    // "let target be the nearest common inclusive ancestor of the
    // corresponding pointerdown and pointerup targets in the DOM at the
    // moment event is being dispatched."
    let target = nearest_common_inclusive_ancestor(dom, pressed, released)?;

    // [DOM § 2.9 Dispatching events](https://dom.spec.whatwg.org/#concept-event-dispatch)
    //
    // "If isActivationEvent is true and target has activation behavior,
    // then set activationTarget to target."
    //
    // "If isActivationEvent is true, event's bubbles attribute is true,
    // activationTarget is null, and parent has activation behavior, then
    // set activationTarget to parent."
    //
    // `click` bubbles, so the activation target is the nearest inclusive
    // ancestor with activation behavior.
    let activation_target = std::iter::once(target)
        .chain(dom.ancestors(target))
        .find(|&node| has_activation_behavior(dom, node))?;

    hyperlink_activation(dom, activation_target, base_url)
}

fn nearest_common_inclusive_ancestor(dom: &DomTree, a: NodeId, b: NodeId) -> Option<NodeId> {
    let a_chain: Vec<NodeId> = std::iter::once(a).chain(dom.ancestors(a)).collect();
    std::iter::once(b)
        .chain(dom.ancestors(b))
        .find(|node| a_chain.contains(node))
}

/// [HTML § 4.6.2 Links created by a and area elements](https://html.spec.whatwg.org/multipage/links.html#links-created-by-a-and-area-elements)
///
/// "The activation behavior of an a or area element element given an
/// event event is: ..."
///
/// The only activation behavior Koala implements; see the module's
/// trade-offs.
fn has_activation_behavior(dom: &DomTree, node: NodeId) -> bool {
    dom.as_element(node).is_some_and(|element| {
        element.namespace == Namespace::Html
            && (element.tag_name.eq_ignore_ascii_case("a")
                || element.tag_name.eq_ignore_ascii_case("area"))
    })
}

/// [HTML § 4.6.2](https://html.spec.whatwg.org/multipage/links.html#links-created-by-a-and-area-elements):
/// the activation behavior of an `a` or `area` element, up to the URL to
/// navigate to.
fn hyperlink_activation(dom: &DomTree, element: NodeId, base_url: Option<&str>) -> Option<String> {
    let attrs = &dom.as_element(element)?.attrs;

    // STEP 1: "If element has no href attribute, then return."
    let href = attrs.get("href")?;

    // STEP 2-3: The `ismap` hyperlink suffix.
    //
    // NOTE: Not implemented; `<img ismap>` links navigate without the
    // "?x,y" suffix.

    // STEP 4-5: "If element has a download attribute, or if the user has
    // expressed a preference to download the hyperlink, then download the
    // hyperlink created by element ..."
    //
    // Koala does not download, so such a link does nothing.
    if attrs.contains_key("download") {
        return None;
    }

    // "Otherwise, follow the hyperlink created by element"
    //
    // [HTML § 4.6.5 Following hyperlinks](https://html.spec.whatwg.org/multipage/links.html#following-hyperlinks-2)
    //
    // "Let urlRecord be the result of encoding-parsing a URL given
    // subject's href attribute value, relative to subject's node
    // document."
    //
    // NOTE: `resolve_url` cannot fail, so the next step, "If urlRecord is
    // failure, then return", never applies.
    Some(resolve_url(href, base_url))
}

#[cfg(test)]
mod tests {
    use super::click;
    use koala_dom::{AttributesMap, DomTree, ElementData, Namespace, NodeId, NodeType};

    fn element(tree: &mut DomTree, parent: NodeId, tag: &str, attrs: &[(&str, &str)]) -> NodeId {
        let mut map = AttributesMap::new();
        for &(name, value) in attrs {
            let _ = map.insert(name.to_string(), value.to_string());
        }
        let id = tree.alloc(NodeType::Element(ElementData {
            tag_name: tag.into(),
            attrs: map,
            namespace: Namespace::Html,
        }));
        tree.append_child(parent, id);
        id
    }

    const BASE: Option<&str> = Some("https://example.com/docs/page.html");

    /// A click anywhere inside a link follows it, resolved against the
    /// document's URL.
    #[test]
    fn click_inside_a_link_follows_it() {
        let mut tree = DomTree::new();
        let link = element(&mut tree, NodeId::ROOT, "a", &[("href", "next.html")]);
        let em = element(&mut tree, link, "em", &[]);
        assert_eq!(
            click(&tree, em, em, BASE).as_deref(),
            Some("https://example.com/docs/next.html")
        );
    }

    /// Pressed on one link and released on another: the click lands on
    /// their nearest common ancestor, the paragraph, which is not a link.
    #[test]
    fn press_and_release_on_different_links_follows_neither() {
        let mut tree = DomTree::new();
        let p = element(&mut tree, NodeId::ROOT, "p", &[]);
        let first = element(&mut tree, p, "a", &[("href", "/one")]);
        let second = element(&mut tree, p, "a", &[("href", "/two")]);
        assert_eq!(click(&tree, first, second, BASE), None);
    }

    /// "If element has no href attribute, then return." The `a` is still
    /// the activation target, so an outer link is not followed either.
    #[test]
    fn anchor_without_href_stops_activation() {
        let mut tree = DomTree::new();
        let outer = element(&mut tree, NodeId::ROOT, "a", &[("href", "/outer")]);
        let inner = element(&mut tree, outer, "a", &[]);
        assert_eq!(click(&tree, inner, inner, BASE), None);
    }

    #[test]
    fn download_link_does_not_navigate() {
        let mut tree = DomTree::new();
        let link = element(&mut tree, NodeId::ROOT, "a", &[("href", "/file.zip"), ("download", "")]);
        assert_eq!(click(&tree, link, link, BASE), None);
    }
}
