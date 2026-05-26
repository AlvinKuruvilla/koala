//! Shared utilities for the DOM bridge — error constructors, node
//! identity plumbing, descendant-text walking, and a small wrapper
//! that turns an fn-pointer into a Boa `JsFunction` suitable for
//! accessor properties.
//!
//! Kept leaf-module: this file imports nothing from its siblings,
//! so `element.rs`, `text.rs`, etc. can all pull from here freely
//! without dependency loops.

use boa_engine::{
    Context, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
    NativeFunction, js_string, object::FunctionObjectBuilder,
    object::builtins::JsFunction,
};
use koala_dom::{DomTree, NodeId};

/// [§ 4.4 Node.nodeType](https://dom.spec.whatwg.org/#dom-node-nodetype)
///
/// Element nodes carry `nodeType == 1`. Stored as f64 to slot
/// directly into a JS Number property without per-access conversion.
pub(super) const NODE_TYPE_ELEMENT: f64 = 1.0;

/// [§ 4.4 Node.nodeType](https://dom.spec.whatwg.org/#dom-node-nodetype)
///
/// Text nodes carry `nodeType == 3`.
pub(super) const NODE_TYPE_TEXT: f64 = 3.0;

/// Wrap a fn-pointer as a [`JsFunction`] suitable for an accessor
/// or any other place that wants a `JsFunction` rather than a
/// `NativeFunction`. fn pointers are `Copy` so the closure goes
/// through `from_copy_closure`.
pub(super) fn getter(
    context: &mut Context,
    f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) -> JsFunction {
    let realm = context.realm().clone();
    FunctionObjectBuilder::new(&realm, NativeFunction::from_copy_closure(f)).build()
}

/// Read the `__nodeId` slot off the JS-side wrapper of the
/// receiver. Returns a `TypeError` if the receiver isn't an object
/// or doesn't carry a `__nodeId` number.
pub(super) fn node_id_from_this(
    this: &JsValue,
    context: &mut Context,
) -> JsResult<NodeId> {
    let obj = this
        .as_object()
        .ok_or_else(|| type_error("method called on a non-object value"))?;
    node_id_from_object(obj, context)
}

/// Read `__nodeId` from a passed-in argument value (an Element or
/// Text wrapper). Used by `Element.appendChild` / `removeChild` and
/// anything else that takes a node as a parameter.
pub(super) fn node_id_from_value(
    value: &JsValue,
    context: &mut Context,
) -> JsResult<NodeId> {
    let obj = value
        .as_object()
        .ok_or_else(|| type_error("expected a node, got a non-object value"))?;
    node_id_from_object(obj, context)
}

fn node_id_from_object(obj: &JsObject, context: &mut Context) -> JsResult<NodeId> {
    let raw = obj.get(js_string!("__nodeId"), context)?;
    let n = raw.to_u32(context)? as usize;
    Ok(NodeId(n))
}

/// Pull arg `index` as a string with a uniform error message.
pub(super) fn required_string_arg(
    args: &[JsValue],
    index: usize,
    method: &'static str,
    arg_name: &'static str,
    context: &mut Context,
) -> JsResult<String> {
    let arg = args.get(index).ok_or_else(|| missing_arg(method, arg_name))?;
    Ok(arg.to_string(context)?.to_std_string_escaped())
}

/// Concatenate every Text descendant's data under `node_id`, in
/// tree order. Backs both `Element.textContent` (getter) and
/// `document.title`.
pub(super) fn descendant_text(dom: &DomTree, node_id: NodeId) -> String {
    let mut buf = String::new();
    for id in dom.descendants(node_id) {
        if let Some(s) = dom.as_text(id) {
            buf.push_str(s);
        }
    }
    buf
}

/// Convenience for converting a Rust string into a Boa string
/// `JsValue` without re-typing the `JsString::from(...)` dance.
pub(super) fn js_string_value(s: &str) -> JsValue {
    JsString::from(s).into()
}

pub(super) fn type_error(message: &str) -> JsError {
    JsError::from_native(JsNativeError::typ().with_message(message.to_string()))
}

pub(super) fn missing_arg(method: &'static str, arg: &'static str) -> JsError {
    type_error(&format!("{method} requires the `{arg}` argument"))
}

pub(super) fn no_dom_error() -> JsError {
    type_error("no DOM is currently installed for this JsRuntime")
}
