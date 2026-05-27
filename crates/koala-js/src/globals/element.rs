//! `Element` wrapper — JS-side object that represents a single
//! DOM element, plus every method and accessor exposed on it.
//!
//! [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element)
//!
//! The wrapper is a plain Boa object carrying:
//!
//! - Snapshot string properties for `tagName`, `id`, `className` —
//!   these don't normally change without going through
//!   `setAttribute`, and `setAttribute` is mutating the DOM (not
//!   the wrapper), so snapshots are correct at the cost of one
//!   round-trip per re-fetch.
//! - A hidden `__nodeId` slot identifying the element in the
//!   underlying [`koala_dom::DomTree`]. Methods read this slot back
//!   via [`super::helpers::node_id_from_this`] to find "which node
//!   am I?".
//! - Live accessor properties (`parentElement`, `children`,
//!   `firstElementChild`, … `textContent`) — these re-read the DOM
//!   on every property access, so any mutation via the bridge is
//!   immediately visible through them.
//! - Methods for attribute IO, tree mutation, and scoped selector
//!   queries.

use boa_engine::{
    Context, JsResult, JsValue, js_string,
    object::ObjectInitializer, object::builtins::JsArray, property::Attribute,
};
use koala_dom::{NodeId, NodeType};

use crate::dom_handle::{
    cache_wrapper, cached_wrapper, mark_dirty, with_dom, with_dom_mut,
};

use super::events::{
    add_listener_at_scope, dispatch_event_call, remove_listener_at_scope,
};
use super::helpers::{
    descendant_text, js_string_value, no_dom_error, node_id_from_this,
    required_string_arg,
};
use super::selectors::{find_all_matches, find_first_match, parse_query_arg};

/// Build the per-element scope key used by
/// `events::dispatch_at_scope`. The Element-side EventTarget
/// methods derive this from the wrapper's `__nodeId` slot, so
/// listeners survive re-querying the same node through a new
/// JsElement wrapper.
pub(crate) fn element_scope_key(node_id: NodeId) -> String {
    format!("node:{}", node_id.0)
}

/// Build the JS object that represents the element at `node_id`.
///
/// `tagName` / `id` / `className` are snapshot string properties
/// since the spec exposes them as live properties but mutating
/// them currently requires `setAttribute`. The other accessors are
/// real getters that re-read the DOM on every JS read, so attribute
/// edits via `setAttribute` etc. show up immediately through them.
#[allow(clippy::similar_names)] // text_content_{getter,setter} are an intentional pair
pub(super) fn make_element_object(
    context: &mut Context,
    node_id: NodeId,
) -> JsResult<JsValue> {
    // Identity: every JS-side reference to the same NodeId must
    // resolve to the same JsObject. The DOM spec requires
    // `el.parentNode === el.parentNode` (and analogous identity
    // for `firstElementChild`, `children[0]`, etc.), and WPT
    // tests assert this everywhere. Caching here means the
    // identity rule holds without each accessor having to know
    // about it.
    if let Some(cached) = cached_wrapper(node_id) {
        return Ok(JsValue::from(cached));
    }

    // Bail early if the DOM doesn't actually have an element at
    // this id — same contract `dom.as_element(node_id)` already
    // imposes on every accessor down-stream.
    let exists =
        with_dom(|dom| dom.as_element(node_id).is_some()).unwrap_or(false);
    if !exists {
        return Err(no_dom_error());
    }

    #[allow(clippy::cast_precision_loss)] // NodeId well below 2^53
    let node_id_value = node_id.0 as f64;

    // The wrapper is now a thin object carrying only the
    // `__nodeId` slot. All other methods + accessors —
    // `getAttribute`, `id`, `tagName`, `addEventListener`,
    // `parentElement`, … — live on `Element.prototype` (or
    // higher up the chain) and resolve via the prototype lookup
    // chain. See `super::element_class` for the registration.
    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("__nodeId"),
            node_id_value,
            Attribute::READONLY,
        )
        .build();

    // Stitch the wrapper into the DOM interface chain so that
    // `el instanceof HTMLElement` / `Element` / `Node` /
    // `EventTarget` all walk through to true.
    let proto = html_element_prototype(context)?;
    let _ = obj.set_prototype(Some(proto));

    cache_wrapper(node_id, obj.clone());
    Ok(JsValue::from(obj))
}

/// Read `HTMLElement.prototype` off the global object. Used by
/// [`make_element_object`] to set the wrapper's `[[Prototype]]`.
/// The HTMLElement class is registered by
/// [`super::element_class::register_html_element_class`].
fn html_element_prototype(context: &mut Context) -> JsResult<boa_engine::JsObject> {
    use super::helpers::type_error;
    let global = context.global_object();
    let ctor = global.get(js_string!("HTMLElement"), context)?;
    let ctor_obj = ctor
        .as_object()
        .ok_or_else(|| type_error("HTMLElement is not an object"))?;
    let proto = ctor_obj.get(js_string!("prototype"), context)?;
    proto
        .as_object()
        .cloned()
        .ok_or_else(|| type_error("HTMLElement.prototype is not an object"))
}

/// Convert a list of element [`NodeId`]s into a JS Array of element
/// wrappers. Builds wrappers individually so a wrapper-construction
/// error short-circuits the whole collection.
pub(super) fn array_of_element_objects(
    ids: Vec<NodeId>,
    context: &mut Context,
) -> JsResult<JsValue> {
    let mut elements = Vec::with_capacity(ids.len());
    for id in ids {
        elements.push(make_element_object(context, id)?);
    }
    Ok(JsArray::from_iter(elements, context).into())
}

// ---- attribute IO ----

/// `Element.getAttribute(name)` — string or `null`.
pub(super) fn get_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "getAttribute", "name", context)?;

    let value = with_dom(|dom| {
        dom.as_element(node_id)
            .and_then(|e| e.attrs.get(&name).cloned())
    })
    .flatten();

    Ok(match value {
        Some(v) => js_string_value(&v),
        None => JsValue::null(),
    })
}

/// `Element.hasAttribute(name)` — true/false.
pub(super) fn has_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "hasAttribute", "name", context)?;

    let present = with_dom(|dom| {
        dom.as_element(node_id)
            .is_some_and(|e| e.attrs.contains_key(&name))
    })
    .unwrap_or(false);

    Ok(JsValue::from(present))
}

/// `Element.setAttribute(name, value)`. Always overwrites. Marks
/// the runtime DOM-dirty on a real change so koala-browser
/// re-runs the style cascade after scripts finish.
pub(super) fn set_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "setAttribute", "name", context)?;
    let value = required_string_arg(args, 1, "setAttribute", "value", context)?;

    let mutated = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.insert(name, value);
            true
        } else {
            false
        }
    });
    if mutated == Some(true) {
        mark_dirty();
    }

    Ok(JsValue::undefined())
}

/// `Element.removeAttribute(name)`. No-op if absent. Marks dirty
/// only when an attribute was actually removed.
pub(super) fn remove_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = required_string_arg(args, 0, "removeAttribute", "name", context)?;

    let mutated = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            elem.attrs.remove(&name).is_some()
        } else {
            false
        }
    });
    if mutated == Some(true) {
        mark_dirty();
    }

    Ok(JsValue::undefined())
}

// `appendChild` / `removeChild` previously lived here as own
// properties on each Element wrapper. They now live on
// `Node.prototype` (see `super::node_class`) and resolve
// through the prototype chain on every `el.appendChild(...)`
// call. The implementations themselves are unchanged — moving
// them just shares the function objects across every Element
// (and any other Node wrapper we add later) instead of
// duplicating per instance.

// ---- scoped selector queries ----

pub(super) fn query_selector(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope_id = node_id_from_this(this, context)?;
    let parsed = parse_query_arg(args, "querySelector", context)?;
    let Some(parsed) = parsed else { return Ok(JsValue::null()) };

    let result =
        with_dom(|dom| find_first_match(dom, scope_id, &parsed)).flatten();

    match result {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

pub(super) fn query_selector_all(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope_id = node_id_from_this(this, context)?;
    let parsed = parse_query_arg(args, "querySelectorAll", context)?;
    let Some(parsed) = parsed else {
        return Ok(JsArray::new(context).into());
    };

    let ids: Vec<NodeId> = with_dom(|dom| find_all_matches(dom, scope_id, &parsed))
        .unwrap_or_default();

    array_of_element_objects(ids, context)
}

// ---- live tree-nav accessors ----

pub(super) fn parent_element_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let parent = with_dom(|dom| {
        dom.parent(node_id)
            .filter(|&id| dom.as_element(id).is_some())
    })
    .flatten();
    match parent {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

pub(super) fn children_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let child_ids: Vec<NodeId> = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .copied()
            .filter(|&id| dom.as_element(id).is_some())
            .collect()
    })
    .unwrap_or_default();
    array_of_element_objects(child_ids, context)
}

pub(super) fn first_element_child_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let first = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .copied()
            .find(|&id| dom.as_element(id).is_some())
    })
    .flatten();
    match first {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

pub(super) fn last_element_child_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let last = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .copied()
            .rev()
            .find(|&id| dom.as_element(id).is_some())
    })
    .flatten();
    match last {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

pub(super) fn next_element_sibling_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let next = with_dom(|dom| {
        let mut cur = dom.next_sibling(node_id);
        while let Some(id) = cur {
            if dom.as_element(id).is_some() {
                return Some(id);
            }
            cur = dom.next_sibling(id);
        }
        None
    })
    .flatten();
    match next {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

pub(super) fn previous_element_sibling_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let prev = with_dom(|dom| {
        let mut cur = dom.prev_sibling(node_id);
        while let Some(id) = cur {
            if dom.as_element(id).is_some() {
                return Some(id);
            }
            cur = dom.prev_sibling(id);
        }
        None
    })
    .flatten();
    match prev {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

// ---- id / className IDL attributes ----
//
// Routing reads through `getAttribute` and writes through
// `setAttribute` keeps the IDL attribute and the content
// attribute synchronised automatically — the spec defines
// `Element.id` as a "reflected" attribute (DOM § 4.9
// "reflected IDL attributes"), and reflection means the IDL
// getter literally returns the content-attribute value. The
// same applies to `Element.className` reflecting `class`.

/// `Element.tagName` — read-only accessor that returns the
/// element's tag name uppercased (the spec form for HTML
/// elements). Lives on `Element.prototype` rather than as a
/// per-wrapper own property so it's not a stale snapshot — if a
/// future bridge mutates the tag name (e.g. via `outerHTML`),
/// readers see the live value.
pub(super) fn tag_name_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = with_dom(|dom| {
        dom.as_element(node_id)
            .map(|e| e.tag_name.to_ascii_uppercase())
    })
    .flatten()
    .unwrap_or_default();
    Ok(js_string_value(&name))
}

/// `ParentNode.childElementCount` — number of Element children
/// (excludes Text / Comment / etc.). The DOM spec puts this on
/// the `ParentNode` mixin (`Element`, `Document`, and
/// `DocumentFragment` all implement it); for now we expose it
/// on `Element.prototype` only.
#[allow(clippy::cast_precision_loss)]
pub(super) fn child_element_count_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let count = with_dom(|dom| {
        dom.children(node_id)
            .iter()
            .filter(|&&id| dom.as_element(id).is_some())
            .count()
    })
    .unwrap_or(0);
    Ok(JsValue::from(count as f64))
}

pub(super) fn id_get(this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let value = with_dom(|dom| {
        dom.as_element(node_id)
            .map(|e| e.id().cloned().unwrap_or_default())
    })
    .flatten()
    .unwrap_or_default();
    Ok(js_string_value(&value))
}

pub(super) fn id_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let new_value = args
        .first()
        .map(|v| v.to_string(context))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let mutated = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.insert("id".to_owned(), new_value);
            true
        } else {
            false
        }
    });
    if mutated == Some(true) {
        mark_dirty();
    }
    Ok(JsValue::undefined())
}

pub(super) fn class_name_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let value = with_dom(|dom| {
        dom.as_element(node_id)
            .map(|e| e.attrs.get("class").cloned().unwrap_or_default())
    })
    .flatten()
    .unwrap_or_default();
    Ok(js_string_value(&value))
}

pub(super) fn class_name_set(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let new_value = args
        .first()
        .map(|v| v.to_string(context))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let mutated = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.insert("class".to_owned(), new_value);
            true
        } else {
            false
        }
    });
    if mutated == Some(true) {
        mark_dirty();
    }
    Ok(JsValue::undefined())
}

// ---- textContent ----

/// `Element.textContent` (getter) — concatenation of every Text
/// descendant's data, in tree order.
pub(super) fn text_content_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let text = with_dom(|dom| descendant_text(dom, node_id)).unwrap_or_default();
    Ok(js_string_value(&text))
}

/// `Element.textContent` (setter) — replace all children with a
/// single Text node carrying the assigned string. Empty string
/// leaves the element with no children.
pub(super) fn text_content_set(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let new_text = args
        .first()
        .map(|v| v.to_string(context))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let _ = with_dom_mut(|dom| {
        // Snapshot then remove — we can't iterate the live slice
        // while mutating through `remove_child`.
        let children: Vec<NodeId> = dom.children(node_id).to_vec();
        for child in children {
            dom.remove_child(node_id, child);
        }
        if !new_text.is_empty() {
            let text_id = dom.alloc(NodeType::Text(new_text));
            dom.append_child(node_id, text_id);
        }
    });
    mark_dirty();

    Ok(JsValue::undefined())
}

pub(super) fn element_add_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let scope = element_scope_key(node_id);
    let type_ = args.first().cloned().unwrap_or(JsValue::undefined());
    let listener = args.get(1).cloned().unwrap_or(JsValue::undefined());
    add_listener_at_scope(&scope, &type_, &listener, context)?;
    Ok(JsValue::undefined())
}

pub(super) fn element_remove_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let scope = element_scope_key(node_id);
    let type_ = args.first().cloned().unwrap_or(JsValue::undefined());
    let listener = args.get(1).cloned().unwrap_or(JsValue::undefined());
    remove_listener_at_scope(&scope, &type_, &listener, context)?;
    Ok(JsValue::undefined())
}

pub(super) fn element_dispatch_event(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let scope = element_scope_key(node_id);
    dispatch_event_call(&scope, this, args, context)
}
