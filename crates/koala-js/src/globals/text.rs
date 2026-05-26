//! Minimal Text-node wrapper.
//!
//! [§ 4.10 Interface Text](https://dom.spec.whatwg.org/#interface-text)
//!
//! For Phase 2 we expose just enough shape — `__nodeId` and
//! `nodeType = 3` — for the returned object to participate in
//! `Element.appendChild`. The richer Text surface (`data`,
//! `nodeValue`, `length`, `splitText`, …) is deferred to a Phase-2
//! follow-up: in practice testharness.js builds output via
//! `el.textContent = …`, which doesn't go through Text wrappers at
//! all.

use boa_engine::{
    Context, JsResult, JsValue, js_string, object::ObjectInitializer,
    property::Attribute,
};
use koala_dom::NodeId;

use super::helpers::NODE_TYPE_TEXT;

/// Build the JS-side wrapper for the Text node at `node_id`.
#[allow(clippy::unnecessary_wraps)] // mirrors make_element_object's fallible shape
pub(super) fn make_text_object(
    context: &mut Context,
    node_id: NodeId,
) -> JsResult<JsValue> {
    #[allow(clippy::cast_precision_loss)] // NodeId well below 2^53
    let node_id_value = node_id.0 as f64;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("nodeType"),
            NODE_TYPE_TEXT,
            Attribute::READONLY,
        )
        .property(
            js_string!("__nodeId"),
            node_id_value,
            Attribute::READONLY,
        )
        .build();

    Ok(obj.into())
}
