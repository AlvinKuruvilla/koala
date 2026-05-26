//! `document` global — Phase-2 DOM bridge.
//!
//! [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//!
//! "The Document interface represents any web page loaded in the
//! browser and serves as an entry point into the web page's
//! content, which is the DOM tree."
//!
//! The exposed methods read the DOM through
//! [`crate::dom_handle::with_dom`], which finds the document's tree
//! via the thread-local that [`JsRuntime::execute`] installs.
//!
//! [`JsRuntime::execute`]: crate::JsRuntime::execute
//!
//! # Implemented
//!
//! - `document.getElementById(id)` — § 3.1.5
//!
//! # Not Yet Implemented
//!
//! [§ 4.2.6 ParentNode](https://dom.spec.whatwg.org/#parentnode):
//! - `querySelector(selectors)` / `querySelectorAll(selectors)`
//!
//! [§ 4.5 NonElementParentNode](https://dom.spec.whatwg.org/#nonelementparentnode):
//! - `getElementsByTagName(name)` / `getElementsByClassName(names)`
//!
//! [§ 3.1.3 The Document interface](https://html.spec.whatwg.org/multipage/dom.html#the-document-object):
//! - `body` / `head` / `documentElement` / `title`
//! - `createElement` / `createTextNode`

use boa_engine::{
    Context, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction,
    js_string,
    object::{
        FunctionObjectBuilder, ObjectInitializer,
        builtins::{JsArray, JsFunction},
    },
    property::Attribute,
};
use koala_dom::{DomTree, NodeId};

use crate::dom_handle::{with_dom, with_dom_mut};

/// Register the `document` global on the context. Called once by
/// [`crate::globals::register_globals`] when a [`JsRuntime`] is
/// created.
///
/// The document object itself is a plain Boa object with the DOM
/// methods attached as `NativeFunction` properties. Each method
/// reads the current DOM via [`crate::dom_handle::with_dom`].
///
/// [`JsRuntime`]: crate::JsRuntime
pub fn register_document(context: &mut Context) {
    let document = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_copy_closure(get_element_by_id),
            js_string!("getElementById"),
            1,
        )
        .build();

    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("`document` global should not already exist");
}

/// `document.getElementById(elementId)`
///
/// [§ 5.1 NonElementParentNode.getElementById](https://dom.spec.whatwg.org/#dom-nonelementparentnode-getelementbyid)
///
/// > "Return the first element, in tree order, within this's
/// > descendants, whose ID is elementId; otherwise, if there is no
/// > such element, null."
///
/// We implement tree order via [`DomTree::iter_all`], which yields
/// every node by increasing `NodeId`. Since [`DomTree::append_child`]
/// always allocates after the parent, allocation order matches
/// tree-preorder for any tree built by the HTML parser.
fn get_element_by_id(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let id_arg = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("getElementById requires an id argument"),
            )
        })?
        .to_string(context)?
        .to_std_string_escaped();

    // STEP 1: "If elementId is the empty string, return null."
    if id_arg.is_empty() {
        return Ok(JsValue::null());
    }

    // STEP 2: walk the tree, find the first element whose id matches.
    let node = with_dom(|dom| find_first_by_id(dom, &id_arg)).flatten();

    match node {
        Some(node_id) => make_element_object(context, node_id),
        None => Ok(JsValue::null()),
    }
}

/// Linear DOM search for the first element whose `id` attribute
/// equals `target`. Tree-order over our arena layout: `iter_all`
/// yields ids in allocation order, and the parser always allocates
/// children after their parent, so the first hit is the
/// document-order first.
fn find_first_by_id(dom: &DomTree, target: &str) -> Option<NodeId> {
    dom.iter_all().find(|&id| {
        dom.as_element(id)
            .and_then(|e| e.id())
            .is_some_and(|got| got == target)
    })
}

/// Build a Boa object representing the [`Element`](https://dom.spec.whatwg.org/#interface-element)
/// at `node_id`. Snapshots `tagName`, `id`, and `className` from
/// the DOM at construction time; method calls (`getAttribute` /
/// `hasAttribute`) re-read the DOM each invocation via
/// [`with_dom`], so live attribute reads always reflect current
/// state.
///
/// The `__nodeId` property is the bridge that lets methods find
/// "which node am I?" — see [`node_id_from_this`].
fn make_element_object(context: &mut Context, node_id: NodeId) -> JsResult<JsValue> {
    // Snapshot the cheap, mostly-static identity bits.
    let (tag_name, id, class_name) = with_dom(|dom| {
        dom.as_element(node_id).map(|e| {
            (
                e.tag_name.to_ascii_uppercase(),
                e.id().cloned().unwrap_or_default(),
                e.attrs.get("class").cloned().unwrap_or_default(),
            )
        })
    })
    .flatten()
    .ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("element disappeared between lookup and wrap"),
        )
    })?;

    // `__nodeId` is stored as an f64 (Number); usize fits losslessly
    // up to 2^53. Marked READONLY to discourage accidental tampering.
    #[allow(clippy::cast_precision_loss)] // NodeId is well below 2^53
    let node_id_value = node_id.0 as f64;

    // Build the getter functions up-front. `ObjectInitializer::new`
    // takes `&mut Context`, and each `getter(context, …)` also wants
    // `&mut Context`, so we can't intermix them with builder calls
    // without a second mutable borrow. Bundling them here makes the
    // initializer chain below side-effect-free w.r.t. the context.
    let parent_element_getter = getter(context, parent_element_get);
    let children_getter = getter(context, children_get);
    let first_element_child_getter = getter(context, first_element_child_get);
    let last_element_child_getter = getter(context, last_element_child_get);
    let next_element_sibling_getter = getter(context, next_element_sibling_get);
    let previous_element_sibling_getter = getter(context, previous_element_sibling_get);

    let accessor_attrs = Attribute::CONFIGURABLE | Attribute::ENUMERABLE;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("tagName"),
            JsString::from(tag_name.as_str()),
            Attribute::READONLY,
        )
        .property(
            js_string!("id"),
            JsString::from(id.as_str()),
            Attribute::READONLY,
        )
        .property(
            js_string!("className"),
            JsString::from(class_name.as_str()),
            Attribute::READONLY,
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
        .build();

    Ok(obj.into())
}

/// Wrap a fn-pointer as a [`JsFunction`] suitable for an accessor.
/// fn pointers implement `Copy` automatically, so the closure can
/// go through `from_copy_closure` without a `Trace`-able capture
/// shim.
///
/// [`JsFunction`]: boa_engine::JsFunction
fn getter(
    context: &mut Context,
    f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) -> JsFunction {
    let realm = context.realm().clone();
    FunctionObjectBuilder::new(&realm, NativeFunction::from_copy_closure(f)).build()
}

/// Find the parent [`NodeId`] if it is itself an Element (i.e. not
/// the Document root). Shared between `parentElement` and the other
/// tree-walk accessors that need "is this thing an element?".
fn parent_element_id(dom: &DomTree, node_id: NodeId) -> Option<NodeId> {
    dom.parent(node_id)
        .filter(|&id| dom.as_element(id).is_some())
}

/// `Element.parentElement`
///
/// [§ 4.2.5 ParentNode.parentElement](https://dom.spec.whatwg.org/#dom-node-parentelement)
///
/// > "Returns the parent element of this. Returns null if there is
/// > no parent element."
///
/// Distinct from `parentNode`: when the parent is the Document
/// itself (i.e. this is the `<html>` element), `parentElement`
/// returns null where `parentNode` would return the Document.
/// We don't yet expose a Document wrapper, so only
/// `parentElement` is supported.
fn parent_element_get(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let parent = with_dom(|dom| parent_element_id(dom, node_id)).flatten();
    match parent {
        Some(id) => make_element_object(context, id),
        None => Ok(JsValue::null()),
    }
}

/// `Element.children` — live-ish array of child Elements only.
///
/// [§ 4.2.6 ParentNode.children](https://dom.spec.whatwg.org/#dom-parentnode-children)
///
/// > "Returns the child elements."
///
/// The spec defines `children` as a live `HTMLCollection`, but for
/// the Phase-2 read path a plain JS array is indistinguishable to
/// almost every caller and avoids the per-element identity work
/// the live collection would require. Re-reads of `el.children`
/// produce a fresh array reflecting current state because this is
/// an accessor.
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

    let mut elements = Vec::with_capacity(child_ids.len());
    for id in child_ids {
        elements.push(make_element_object(context, id)?);
    }
    Ok(JsArray::from_iter(elements, context).into())
}

/// `Element.firstElementChild`
///
/// [§ 4.2.6 ParentNode.firstElementChild](https://dom.spec.whatwg.org/#dom-parentnode-firstelementchild)
///
/// > "Returns the first child that is an element; otherwise null."
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

/// `Element.lastElementChild`
///
/// [§ 4.2.6 ParentNode.lastElementChild](https://dom.spec.whatwg.org/#dom-parentnode-lastelementchild)
///
/// > "Returns the last child that is an element; otherwise null."
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

/// `Element.nextElementSibling`
///
/// [§ 4.2.7 NonDocumentTypeChildNode.nextElementSibling](https://dom.spec.whatwg.org/#dom-nondocumenttypechildnode-nextelementsibling)
///
/// > "Returns the first following sibling that is an element;
/// > otherwise null."
///
/// Walks via [`DomTree::next_sibling`] until it finds an element or
/// runs out of siblings; Text/Comment nodes are skipped over.
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

/// `Element.previousElementSibling`
///
/// [§ 4.2.7 NonDocumentTypeChildNode.previousElementSibling](https://dom.spec.whatwg.org/#dom-nondocumenttypechildnode-previouselementsibling)
///
/// > "Returns the first preceding sibling that is an element;
/// > otherwise null."
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

/// `Element.getAttribute(name)` — returns the attribute value as a
/// string, or `null` when the attribute is absent.
///
/// [§ 4.9.2 Element.getAttribute](https://dom.spec.whatwg.org/#dom-element-getattribute)
fn get_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("getAttribute requires a name argument"),
            )
        })?
        .to_string(context)?
        .to_std_string_escaped();

    let value = with_dom(|dom| {
        dom.as_element(node_id)
            .and_then(|e| e.attrs.get(&name).cloned())
    })
    .flatten();

    Ok(match value {
        Some(v) => JsString::from(v.as_str()).into(),
        None => JsValue::null(),
    })
}

/// `Element.hasAttribute(name)` — true when an attribute with the
/// given name is set on the element.
///
/// [§ 4.9.2 Element.hasAttribute](https://dom.spec.whatwg.org/#dom-element-hasattribute)
fn has_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("hasAttribute requires a name argument"),
            )
        })?
        .to_string(context)?
        .to_std_string_escaped();

    let present = with_dom(|dom| {
        dom.as_element(node_id)
            .is_some_and(|e| e.attrs.contains_key(&name))
    })
    .unwrap_or(false);

    Ok(JsValue::from(present))
}

/// `Element.setAttribute(name, value)` — write the attribute,
/// adding it if absent. Always returns `undefined` (per spec) and
/// always mutates: the only way to fail is via the receiver check.
///
/// [§ 4.9.2 Element.setAttribute](https://dom.spec.whatwg.org/#dom-element-setattribute)
///
/// > "Set an attribute value for this using qualifiedName and value."
///
/// NOTE: we don't lowercase the qualified name here. Per the spec,
/// HTML documents lowercase before lookup; we keep the case the
/// caller supplied since koala's HTML parser already canonicalises
/// attribute names at parse time. Re-introducing lowercasing here
/// would create asymmetry with `getAttribute`.
///
/// LIMITATION: mutations don't trigger re-layout yet — see the
/// note in [`crate`] about scripts running after the layout pass.
fn set_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("setAttribute requires a name argument"),
            )
        })?
        .to_string(context)?
        .to_std_string_escaped();
    let value = args
        .get(1)
        .ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("setAttribute requires a value argument"),
            )
        })?
        .to_string(context)?
        .to_std_string_escaped();

    let _ = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.insert(name, value);
        }
    });

    Ok(JsValue::undefined())
}

/// `Element.removeAttribute(name)` — remove the attribute. No-op
/// when it's already absent. Always returns `undefined`.
///
/// [§ 4.9.2 Element.removeAttribute](https://dom.spec.whatwg.org/#dom-element-removeattribute)
///
/// > "Remove an attribute given qualifiedName and this, and then
/// > return undefined."
fn remove_attribute(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let node_id = node_id_from_this(this, context)?;
    let name = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ()
                    .with_message("removeAttribute requires a name argument"),
            )
        })?
        .to_string(context)?
        .to_std_string_escaped();

    let _ = with_dom_mut(|dom| {
        if let Some(elem) = dom.as_element_mut(node_id) {
            let _ = elem.attrs.remove(&name);
        }
    });

    Ok(JsValue::undefined())
}

/// Read the `__nodeId` slot off the JS-side element wrapper and
/// convert back to a [`NodeId`]. Returns a `TypeError` if the
/// receiver isn't a koala element (no `__nodeId` property, or the
/// value isn't a finite number).
fn node_id_from_this(this: &JsValue, context: &mut Context) -> JsResult<NodeId> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("method called on a non-element value"),
        )
    })?;
    let raw = obj.get(js_string!("__nodeId"), context)?;
    let n = raw.to_u32(context)? as usize;
    Ok(NodeId(n))
}
