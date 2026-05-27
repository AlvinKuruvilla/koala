//! Web IDL interface constructors exposed as JS globals.
//!
//! [§ 4 Node tree](https://dom.spec.whatwg.org/#node-tree) — DOM
//! interface hierarchy.
//!
//! The DOM spec defines an inheritance chain for node types:
//!
//! ```text
//! EventTarget
//!   └─ Node
//!        ├─ Element
//!        │     └─ HTMLElement
//!        ├─ CharacterData      (added in a later slice)
//!        │     ├─ Text         (added in a later slice)
//!        │     └─ Comment      (added in a later slice)
//!        ├─ Document           (added in a later slice)
//!        └─ DocumentType       (added in a later slice)
//! ```
//!
//! Real WPT tests rely on this chain in three ways:
//!
//! 1. `instanceof` checks against the constructor:
//!    `parentEl instanceof HTMLElement`,
//!    `node instanceof Node`.
//! 2. Prototype lookups:
//!    `Object.getPrototypeOf(el) === HTMLElement.prototype`.
//! 3. `assert_throws_dom(...)` against constructor stubs
//!    (`new Node()` must throw `TypeError: Illegal constructor`).
//!
//! This module builds the prototype chain and stub constructors,
//! and stashes each prototype in a hidden global so other
//! globals (notably `element.rs`'s `make_element_object`) can
//! pull them when constructing wrapper objects and stitch the
//! prototype chain together.
//!
//! ## Scope of this slice
//!
//! This first slice covers EventTarget → Node → Element →
//! HTMLElement only — enough for the
//! `assert_true(x instanceof HTMLElement)` family that dominates
//! the current /dom/nodes/ failures. CharacterData, Text,
//! Comment, Document, and DocumentType land in follow-ups —
//! they need actual construction logic (`new Text("...")` is
//! valid in real browsers, unlike `new HTMLElement()`).
//!
//! Method exposure on the prototypes is deliberately deferred:
//! today every wrapper carries its methods as own properties
//! (see `element.rs`'s `make_element_object`). That stays
//! correct under prototype-chain lookup — own properties shadow
//! prototype properties — and lets `instanceof` work without
//! reshuffling every wrapper's method-attachment scheme.

use boa_engine::{
    Context, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};

/// Hidden global slot holding the `HTMLElement.prototype` object,
/// looked up by [`super::element::make_element_object`] when
/// constructing element wrappers. Stashing here (instead of
/// threading a struct through every with_context_mut call) keeps
/// the wiring local and matches how [`super::events`] hides its
/// listener storage.
pub(crate) const HTML_ELEMENT_PROTO_KEY: &str = "__koala_HTMLElement_proto__";

/// Sentinel: the prototype chain has been wired up. Tests on
/// [`super::element::make_element_object`] (and equivalent
/// future wrappers) can assert this is present before reading
/// any of the proto keys.
const INTERFACES_REGISTERED_KEY: &str = "__koala_dom_interfaces_registered__";

/// Build the DOM interface prototype chain and expose the
/// constructor stubs as globals. Idempotency is not promised —
/// call this once per [`crate::JsRuntime`], from
/// [`super::register_globals`].
///
/// Caller contract: `EventTarget` must already be registered (by
/// [`super::event_target_class::register_event_target_class`])
/// before this runs, because `Node.prototype`'s `[[Prototype]]`
/// slot points at `EventTarget.prototype`. The single registered
/// `EventTarget` class is the source of truth — we look its
/// prototype up from the global object rather than holding our
/// own copy here, so `Object.getPrototypeOf(Node.prototype) ===
/// EventTarget.prototype` works automatically.
pub(super) fn register_dom_interfaces(context: &mut Context) {
    let event_target_proto = event_target_prototype(context)
        .expect("EventTarget must be registered before register_dom_interfaces");

    // Prototypes are built top-down (Node first) so each child
    // can set its parent as `__proto__` immediately after
    // creation, before anything reads it.
    let node_proto = ObjectInitializer::new(context).build();
    let element_proto = ObjectInitializer::new(context).build();
    let html_element_proto = ObjectInitializer::new(context).build();

    // Stitch the prototype chain. `set_prototype` is the IDL
    // [[Prototype]] slot setter; returns `false` on failure
    // (typically when the prototype would be cyclic) but our
    // fresh objects can't be cyclic, so we don't bother
    // surfacing that as an error.
    let _ = node_proto.set_prototype(Some(event_target_proto));
    let _ = element_proto.set_prototype(Some(node_proto.clone()));
    let _ = html_element_proto.set_prototype(Some(element_proto.clone()));

    register_interface(context, "Node", &node_proto);
    register_interface(context, "Element", &element_proto);
    register_interface(context, "HTMLElement", &html_element_proto);

    // Stash HTMLElement.prototype where element.rs can read it.
    // We only need to expose the leaf — the chain is reachable
    // from there via `__proto__`.
    context
        .register_global_property(
            js_string!(HTML_ELEMENT_PROTO_KEY),
            JsValue::from(html_element_proto),
            Attribute::all(),
        )
        .expect("HTMLElement proto stash should not already be registered");

    context
        .register_global_property(
            js_string!(INTERFACES_REGISTERED_KEY),
            JsValue::from(true),
            Attribute::all(),
        )
        .expect("dom interfaces sentinel should not already be registered");
}

/// Fetch the (Class-trait-registered) `EventTarget.prototype` so
/// the hand-rolled `Node` / `Element` / `HTMLElement` chain can
/// hang off it. Used by [`register_dom_interfaces`]; will go
/// away once the rest of the chain also migrates to [`Class`].
///
/// [`Class`]: boa_engine::class::Class
fn event_target_prototype(context: &mut Context) -> Option<JsObject> {
    let global = context.global_object();
    let ctor = global.get(js_string!("EventTarget"), context).ok()?;
    let ctor_obj = ctor.as_object()?;
    let proto = ctor_obj.get(js_string!("prototype"), context).ok()?;
    proto.as_object().cloned()
}

/// Register one interface as a global constructor with the
/// `prototype` <-> `constructor` round-trip wired up.
///
/// The constructor itself is a no-op throw — these interfaces
/// (`EventTarget`, `Node`, `Element`, `HTMLElement`) have no
/// public constructor in the DOM spec, and calling `new
/// HTMLElement()` from web content throws `TypeError`. The
/// closure body mirrors that.
fn register_interface(context: &mut Context, name: &'static str, proto: &JsObject) {
    let ctor = NativeFunction::from_copy_closure(move |_, _, _| {
        Err(JsError::from_native(
            JsNativeError::typ()
                .with_message(format!("Illegal constructor: {name} is not constructible")),
        ))
    });
    context
        .register_global_callable(js_string!(name), 0, ctor)
        .unwrap_or_else(|_| panic!("`{name}` global should not already be registered"));

    // Replace the auto-generated `prototype` on the just-registered
    // constructor with ours, and back-link `prototype.constructor`
    // so `HTMLElement.prototype.constructor === HTMLElement`.
    let global = context.global_object();
    let ctor_value = global
        .get(js_string!(name), context)
        .expect("interface constructor should be readable after registration");
    let ctor_obj = ctor_value
        .as_object()
        .expect("interface constructor should be an object");
    let _ = ctor_obj.set(
        js_string!("prototype"),
        JsValue::from(proto.clone()),
        false,
        context,
    );
    let _ = proto.set(
        js_string!("constructor"),
        ctor_value,
        false,
        context,
    );
}

/// Fetch the cached `HTMLElement.prototype` object. Used by
/// [`super::element::make_element_object`] to set the prototype
/// of each freshly-built element wrapper.
///
/// # Errors
///
/// Returns a [`JsError`] only when the hidden slot is missing
/// (`register_dom_interfaces` wasn't called) or has been
/// replaced with a non-object value (only possible if a
/// malicious script clobbered the slot).
pub(crate) fn html_element_prototype(context: &mut Context) -> JsResult<JsObject> {
    let global = context.global_object();
    let value = global.get(js_string!(HTML_ELEMENT_PROTO_KEY), context)?;
    value.as_object().cloned().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("HTMLElement prototype slot missing or not an object"),
        )
    })
}
