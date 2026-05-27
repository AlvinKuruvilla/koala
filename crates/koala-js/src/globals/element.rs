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
    Context, JsResult, JsString, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, object::builtins::JsArray, property::Attribute,
};
use koala_dom::{NodeId, NodeType};

use crate::dom_handle::{mark_dirty, with_dom, with_dom_mut};

use super::events::{
    add_listener_at_scope, dispatch_event_call, remove_listener_at_scope,
};
use super::helpers::{
    NODE_TYPE_ELEMENT, descendant_text, getter, js_string_value, missing_arg,
    no_dom_error, node_id_from_this, node_id_from_value, required_string_arg,
    type_error,
};
use super::interfaces::html_element_prototype;
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
    let tag_name = with_dom(|dom| {
        dom.as_element(node_id)
            .map(|e| e.tag_name.to_ascii_uppercase())
    })
    .flatten()
    .ok_or_else(no_dom_error)?;

    #[allow(clippy::cast_precision_loss)] // NodeId well below 2^53
    let node_id_value = node_id.0 as f64;

    let parent_element_getter = getter(context, parent_element_get);
    let children_getter = getter(context, children_get);
    let first_element_child_getter = getter(context, first_element_child_get);
    let last_element_child_getter = getter(context, last_element_child_get);
    let next_element_sibling_getter = getter(context, next_element_sibling_get);
    let previous_element_sibling_getter = getter(context, previous_element_sibling_get);
    let text_content_getter = getter(context, text_content_get);
    let text_content_setter = getter(context, text_content_set);
    // id and className are live read/write accessors that route
    // back through the underlying attribute store. They were
    // snapshot READONLY string properties before — that broke
    // every WPT test that did `el.id = "foo"` (the cascade of
    // ~70 "cannot set non-writable property: id" failures in
    // /dom/nodes/). Routing through `setAttribute` keeps the
    // attribute view (`el.getAttribute("id")`) consistent with
    // the IDL view (`el.id`) automatically.
    let id_getter = getter(context, id_get);
    let id_setter = getter(context, id_set);
    let class_name_getter = getter(context, class_name_get);
    let class_name_setter = getter(context, class_name_set);

    let accessor_attrs = Attribute::CONFIGURABLE | Attribute::ENUMERABLE;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("nodeType"),
            NODE_TYPE_ELEMENT,
            Attribute::READONLY,
        )
        .property(
            js_string!("tagName"),
            JsString::from(tag_name.as_str()),
            Attribute::READONLY,
        )
        .accessor(
            js_string!("id"),
            Some(id_getter),
            Some(id_setter),
            accessor_attrs,
        )
        .accessor(
            js_string!("className"),
            Some(class_name_getter),
            Some(class_name_setter),
            accessor_attrs,
        )
        .property(
            js_string!("__nodeId"),
            node_id_value,
            Attribute::READONLY,
        )
        .function(
            NativeFunction::from_copy_closure(get_attribute),
            js_string!("getAttribute"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(has_attribute),
            js_string!("hasAttribute"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(set_attribute),
            js_string!("setAttribute"),
            2,
        )
        .function(
            NativeFunction::from_copy_closure(remove_attribute),
            js_string!("removeAttribute"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(append_child),
            js_string!("appendChild"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(remove_child),
            js_string!("removeChild"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(query_selector),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(query_selector_all),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_copy_closure(element_add_event_listener),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_copy_closure(element_remove_event_listener),
            js_string!("removeEventListener"),
            2,
        )
        .function(
            NativeFunction::from_copy_closure(element_dispatch_event),
            js_string!("dispatchEvent"),
            1,
        )
        .accessor(
            js_string!("parentElement"),
            Some(parent_element_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("children"),
            Some(children_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("firstElementChild"),
            Some(first_element_child_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("lastElementChild"),
            Some(last_element_child_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("nextElementSibling"),
            Some(next_element_sibling_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("previousElementSibling"),
            Some(previous_element_sibling_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("textContent"),
            Some(text_content_getter),
            Some(text_content_setter),
            accessor_attrs,
        )
        .build();

    // Stitch the wrapper into the DOM interface chain so that
    // `el instanceof HTMLElement` / `Element` / `Node` /
    // `EventTarget` all walk through to true. The actual methods
    // remain own properties on `obj` (set up by the
    // `ObjectInitializer` above) — own properties shadow
    // prototype properties, so behaviour is unchanged.
    let proto = html_element_prototype(context)?;
    let _ = obj.set_prototype(Some(proto));

    Ok(obj.into())
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
fn get_attribute(
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
fn has_attribute(
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
fn set_attribute(
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
fn remove_attribute(
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

// ---- tree mutation ----

/// `Element.appendChild(node)` — append `node` as the last child.
/// If `node` already has a parent, it is first removed (DOM spec
/// requirement for "adopt the node into this's node document").
///
/// [§ 4.4 Node.appendChild](https://dom.spec.whatwg.org/#dom-node-appendchild)
fn append_child(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = node_id_from_this(this, context)?;
    let child_value = args.first().ok_or_else(|| missing_arg("appendChild", "node"))?;
    let child_id = node_id_from_value(child_value, context)?;

    if parent_id == child_id {
        return Err(type_error("a node cannot be its own child"));
    }

    let _ = with_dom_mut(|dom| {
        if let Some(old_parent) = dom.parent(child_id) {
            dom.remove_child(old_parent, child_id);
        }
        dom.append_child(parent_id, child_id);
    });
    mark_dirty();

    Ok(child_value.clone())
}

/// `Element.removeChild(node)` — detach `node` from this element.
/// Throws (a `TypeError`, since we don't yet model `DOMException`)
/// if `node`'s parent isn't this element.
///
/// [§ 4.4 Node.removeChild](https://dom.spec.whatwg.org/#dom-node-removechild)
fn remove_child(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = node_id_from_this(this, context)?;
    let child_value = args.first().ok_or_else(|| missing_arg("removeChild", "node"))?;
    let child_id = node_id_from_value(child_value, context)?;

    let belongs = with_dom(|dom| dom.parent(child_id) == Some(parent_id)).unwrap_or(false);
    if !belongs {
        return Err(type_error("removeChild: node is not a child of this element"));
    }

    let _ = with_dom_mut(|dom| dom.remove_child(parent_id, child_id));
    mark_dirty();

    Ok(child_value.clone())
}

// ---- scoped selector queries ----

fn query_selector(
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

fn query_selector_all(
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

fn parent_element_get(
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

fn children_get(
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

fn first_element_child_get(
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

fn last_element_child_get(
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

fn next_element_sibling_get(
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

fn previous_element_sibling_get(
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

fn id_get(this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let value = with_dom(|dom| {
        dom.as_element(node_id)
            .map(|e| e.id().cloned().unwrap_or_default())
    })
    .flatten()
    .unwrap_or_default();
    Ok(js_string_value(&value))
}

fn id_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
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

fn class_name_get(
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

fn class_name_set(
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
fn text_content_get(
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
fn text_content_set(
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

fn element_add_event_listener(
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

fn element_remove_event_listener(
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

fn element_dispatch_event(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let scope = element_scope_key(node_id);
    dispatch_event_call(&scope, this, args, context)
}
