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
    object::ObjectInitializer,
    property::Attribute,
};
use koala_dom::{DomTree, NodeId};

use crate::dom_handle::with_dom;

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
        .build();

    Ok(obj.into())
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
