//! CSS-selector plumbing for `querySelector` / `querySelectorAll`.
//!
//! [§ 4.2.6 ParentNode.querySelector](https://dom.spec.whatwg.org/#dom-parentnode-queryselector)
//!
//! `koala_css::parse_selector` only understands one complex
//! selector at a time, so the spec's "selector list" form
//! (`"div, p"`) is split here and matched as a logical OR over the
//! parsed parts.

use boa_engine::{Context, JsResult, JsValue};
use koala_css::selector::{ParsedSelector, parse_selector};
use koala_dom::{DomTree, NodeId};

use super::helpers::required_string_arg;

/// Pull the selector argument out and parse it. Returns `None` if
/// the argument is empty or every part fails to parse — per spec
/// we'd throw `SyntaxError`, but we don't yet expose
/// `DOMException`, so we surface as "no match" and let the caller
/// pick between `null` (querySelector) and `[]` (querySelectorAll).
pub(super) fn parse_query_arg(
    args: &[JsValue],
    method: &'static str,
    context: &mut Context,
) -> JsResult<Option<Vec<ParsedSelector>>> {
    let raw = required_string_arg(args, 0, method, "selectors", context)?;
    let mut parsed = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(sel) = parse_selector(trimmed) {
            parsed.push(sel);
        }
    }
    Ok(if parsed.is_empty() { None } else { Some(parsed) })
}

/// First element under `scope` (in document order) matching any
/// selector in `parsed`. Per the DOM spec, when a selector list is
/// given, the result is the first descendant matching ANY of the
/// listed selectors — chosen by *document order*, not selector
/// order.
pub(super) fn find_first_match(
    dom: &DomTree,
    scope: NodeId,
    parsed: &[ParsedSelector],
) -> Option<NodeId> {
    dom.descendants(scope).find(|&id| {
        dom.as_element(id).is_some()
            && parsed.iter().any(|p| p.matches_in_tree(dom, id))
    })
}

/// Every element under `scope` matching at least one selector in
/// `parsed`, in tree order. No deduplication needed — an element
/// is its own [`NodeId`], and we visit each one exactly once.
pub(super) fn find_all_matches(
    dom: &DomTree,
    scope: NodeId,
    parsed: &[ParsedSelector],
) -> Vec<NodeId> {
    dom.descendants(scope)
        .filter(|&id| {
            dom.as_element(id).is_some()
                && parsed.iter().any(|p| p.matches_in_tree(dom, id))
        })
        .collect()
}
