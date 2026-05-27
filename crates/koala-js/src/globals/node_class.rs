//! `Node` as a Boa-native [`Class`].
//!
//! [§ 4.4 Interface Node](https://dom.spec.whatwg.org/#interface-node)
//!
//! ```idl
//! [Exposed=Window]
//! interface Node : EventTarget {
//!   const unsigned short ELEMENT_NODE = 1;
//!   …
//!   readonly attribute unsigned short nodeType;
//!   readonly attribute DOMString nodeName;
//!   readonly attribute Node? parentNode;
//!   Node appendChild(Node node);
//!   Node removeChild(Node child);
//!   boolean contains(Node? other);
//!   …
//! };
//! ```
//!
//! # Why this module exists separately from `event_target_class`
//!
//! `Node` is abstract — `new Node()` throws — *and* carries
//! methods + accessors that every wrapped DOM node inherits via
//! the prototype chain. That's both axes the
//! [`event_target_class`](super::event_target_class)-style
//! pattern needs to cover before we collapse the boilerplate
//! into a `dom_interface!` macro.
//!
//! Concretely:
//!
//! - `data_constructor` returns `Err("Illegal constructor")` for
//!   the abstract case. The Class trait doesn't have a "no
//!   constructor" knob, so throwing from the data constructor is
//!   the canonical workaround.
//! - Method/accessor functions read `this.__nodeId` (the same
//!   slot the existing per-element wrappers already carry), look
//!   the node up in the thread-local DOM, and operate on it.
//!   They do **not** use [`JsObject::downcast_ref`]: the per-node
//!   wrappers don't have any native data attached today, and
//!   shoehorning a `NodeData` into every wrapper would be a much
//!   bigger restructure for no behavioural win.
//!
//! [`JsObject::downcast_ref`]: boa_engine::JsObject::downcast_ref
//!
//! # Prototype chain wiring
//!
//! After `register_node_class` runs, `Node.prototype`'s
//! `[[Prototype]]` slot is rewired to point at
//! `EventTarget.prototype` (registered earlier by
//! [`event_target_class`]). Boa's `Class` doesn't model
//! cross-class inheritance, so the link is set manually via
//! [`JsObject::set_prototype`]. The downstream
//! [`super::interfaces`] module does the same when it stitches
//! `Element.prototype` onto `Node.prototype` for the still-
//! hand-rolled half of the chain.

use boa_engine::{Context, JsArgs, JsResult, JsValue};
use boa_gc::{Finalize, Trace};
use koala_dom::NodeId;

use crate::dom_handle::{mark_dirty, with_dom, with_dom_mut};

use super::helpers::{
    NODE_TYPE_ELEMENT, NODE_TYPE_TEXT, js_string_value, missing_arg,
    node_id_from_this, node_id_from_value, type_error,
};

/// Zero-sized marker. `Node` is abstract; no wrapper is ever
/// actually constructed from it, but [`boa_engine::class::Class`]
/// requires a per-instance data type. The data slot for every
/// real DOM node wrapper remains the `__nodeId` JS property that
/// [`super::element::make_element_object`] writes — same path
/// the inherited prototype methods read from on lookup.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
pub(crate) struct NodeData;

dom_interface! {
    name: "Node",
    data: NodeData,
    parent: "EventTarget",
    constructible: false,
    methods: [
        ("appendChild", 1, node_append_child),
        ("removeChild", 1, node_remove_child),
        ("contains", 1, node_contains),
    ],
    accessors: [
        ("nodeType", get(node_node_type_get)),
        ("nodeName", get(node_node_name_get)),
        ("parentNode", get(node_parent_node_get)),
    ],
    register: register_node_class,
}

// ---- method / accessor implementations ----
//
// These mirror the existing `super::element` per-element
// methods of the same name — calling them via the prototype
// chain on an Element wrapper will produce the same observable
// behaviour as the (now-removed) own-property duplicates did
// before this migration. Diverging from the element.rs
// versions: parentNode returns a Node even when the parent is
// not an Element, whereas Element's `parentElement` filtered
// non-element parents to `null`.

fn node_append_child(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = node_id_from_this(this, context)?;
    let child_value = args
        .first()
        .ok_or_else(|| missing_arg("appendChild", "node"))?;
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

fn node_remove_child(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = node_id_from_this(this, context)?;
    let child_value = args
        .first()
        .ok_or_else(|| missing_arg("removeChild", "node"))?;
    let child_id = node_id_from_value(child_value, context)?;

    let belongs =
        with_dom(|dom| dom.parent(child_id) == Some(parent_id)).unwrap_or(false);
    if !belongs {
        return Err(type_error("removeChild: node is not a child of this node"));
    }

    let _ = with_dom_mut(|dom| dom.remove_child(parent_id, child_id));
    mark_dirty();

    Ok(child_value.clone())
}

/// `Node.contains(other)` — true when `other` is `this` or a
/// descendant. Walks `other`'s parent chain rather than `this`'s
/// subtree so the cost is bounded by tree depth, not subtree
/// size.
fn node_contains(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let self_id = node_id_from_this(this, context)?;
    let other = args.get_or_undefined(0);
    if other.is_null_or_undefined() {
        return Ok(JsValue::from(false));
    }
    let other_id = node_id_from_value(other, context)?;
    let found = with_dom(|dom| {
        let mut cursor = Some(other_id);
        while let Some(id) = cursor {
            if id == self_id {
                return true;
            }
            cursor = dom.parent(id);
        }
        false
    })
    .unwrap_or(false);
    Ok(JsValue::from(found))
}

fn node_node_type_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let kind = with_dom(|dom| classify_node(dom, node_id)).flatten();
    Ok(match kind {
        Some(NodeKind::Element) => JsValue::from(NODE_TYPE_ELEMENT),
        Some(NodeKind::Text) => JsValue::from(NODE_TYPE_TEXT),
        None => JsValue::from(0u32),
    })
}

fn node_node_name_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = with_dom(|dom| {
        if let Some(el) = dom.as_element(node_id) {
            return Some(el.tag_name.to_ascii_uppercase());
        }
        if dom.as_text(node_id).is_some() {
            return Some("#text".to_owned());
        }
        None
    })
    .flatten()
    .unwrap_or_default();
    Ok(js_string_value(&name))
}

/// `Node.parentNode` — returns the parent node, which may be of
/// any node type. Today koala only mints wrappers for elements,
/// so a non-element parent (e.g. the Document) resolves to
/// `null`. Once Document gets its own wrapper this will widen
/// without callers having to change.
fn node_parent_node_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let parent = with_dom(|dom| dom.parent(node_id)).flatten();
    let Some(parent_id) = parent else {
        return Ok(JsValue::null());
    };
    // Until non-Element wrappers exist, only return parents that
    // are elements. Tree walks from inside the engine still see
    // the real parent via the DOM; only JS observability is
    // gated here.
    let is_element =
        with_dom(|dom| dom.as_element(parent_id).is_some()).unwrap_or(false);
    if !is_element {
        return Ok(JsValue::null());
    }
    super::element::make_element_object(context, parent_id)
}

enum NodeKind {
    Element,
    Text,
}

fn classify_node(dom: &koala_dom::DomTree, id: NodeId) -> Option<NodeKind> {
    if dom.as_element(id).is_some() {
        return Some(NodeKind::Element);
    }
    if dom.as_text(id).is_some() {
        return Some(NodeKind::Text);
    }
    None
}
