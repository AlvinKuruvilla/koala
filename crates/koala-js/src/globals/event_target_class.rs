//! `EventTarget` as a Boa-native [`Class`].
//!
//! [§ 2.6 Interface EventTarget](https://dom.spec.whatwg.org/#interface-eventtarget)
//!
//! ```idl
//! [Exposed=*]
//! interface EventTarget {
//!   constructor();
//!   undefined addEventListener(DOMString type, EventListener? callback, …);
//!   undefined removeEventListener(DOMString type, EventListener? callback, …);
//!   boolean   dispatchEvent(Event event);
//! };
//! ```
//!
//! # Why a separate module
//!
//! The other DOM globals here predate the migration to Boa's
//! [`Class`] trait — they're built via [`ObjectInitializer`] and
//! attach methods directly on each wrapper as own properties.
//! That works but doesn't scale: methods aren't on the prototype,
//! so tests that do `EventTarget.prototype.addEventListener.call(other,
//! ...)` fail, and every new interface needs its own bespoke
//! `make_*_object` builder.
//!
//! This module is the timeboxed validation that Boa's [`Class`]
//! gives us a cleaner shape — prototype-installed methods,
//! constructor sugar, [`JsObject::downcast_ref`] to recover the
//! Rust state. If the pattern fits, subsequent interfaces
//! (`Node`, `Element`, `HTMLElement`, …) collapse into a
//! declarative macro on top.
//!
//! [`ObjectInitializer`]: boa_engine::object::ObjectInitializer
//! [`JsObject::downcast_ref`]: boa_engine::JsObject::downcast_ref
//!
//! # Wrapper-to-Rust binding
//!
//! Each wrapper carries an [`EventTargetData`] in its native-object
//! slot. The data holds a unique `scope_key` that the prototype
//! methods use to look up the listener bucket inside the existing
//! `__koala_listeners__` storage. Reusing the existing storage
//! means listener bookkeeping is shared with the (still-procedural)
//! `window` and per-element targets — no double-bookkeeping.

use std::sync::atomic::{AtomicU64, Ordering};

use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
};
use boa_gc::{Finalize, Trace};

use super::events::{
    add_listener_at_scope, dispatch_event_call, remove_listener_at_scope,
};

/// Monotonic counter used to mint scope keys for fresh
/// `new EventTarget()` instances. A `u64` provides ~580 years of
/// headroom at 1 GHz allocation rate, which is more than enough.
static NEXT_SCOPE_ID: AtomicU64 = AtomicU64::new(0);

/// Per-instance state for a JS `EventTarget` constructed via
/// `new EventTarget()`.
///
/// The `scope_key` namespaces the listener bucket inside the
/// existing `__koala_listeners__` storage — `"et:42"` for the
/// 42nd fresh instance — so listeners attached to two distinct
/// `new EventTarget()` instances stay isolated from each other
/// and from the per-element / per-window buckets.
#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct EventTargetData {
    // `#[unsafe_ignore_trace]` is the way to opt out for non-Gc
    // fields per `boa_gc`'s docs. A `String` doesn't hold any
    // tracked references, so this is sound — same shape that
    // boa-engine itself uses for its own `String`-typed fields
    // inside `JsData` derivers (see `BigInt`'s `Trace, Finalize,
    // JsData` derive in `boa_engine/src/bigint.rs`).
    #[unsafe_ignore_trace]
    scope_key: String,
}

impl EventTargetData {
    /// Mint a fresh instance with a unique scope key.
    fn new() -> Self {
        let id = NEXT_SCOPE_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            scope_key: format!("et:{id}"),
        }
    }
}

impl Class for EventTargetData {
    const NAME: &'static str = "EventTarget";
    const LENGTH: usize = 0;

    fn data_constructor(
        _new_target: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<Self> {
        Ok(Self::new())
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        // `ClassBuilder::method` returns the builder for chaining;
        // bind the result to `_` so the workspace's
        // `unused_must_use` lint doesn't fire on a non-chained call.
        let _ = class.method(
            js_string!("addEventListener"),
            2,
            NativeFunction::from_fn_ptr(class_add_event_listener),
        );
        let _ = class.method(
            js_string!("removeEventListener"),
            2,
            NativeFunction::from_fn_ptr(class_remove_event_listener),
        );
        let _ = class.method(
            js_string!("dispatchEvent"),
            1,
            NativeFunction::from_fn_ptr(class_dispatch_event),
        );
        Ok(())
    }
}

/// Register `EventTarget` as a global class. Replaces the
/// hand-rolled stub previously installed by
/// [`super::interfaces::register_dom_interfaces`].
pub(super) fn register_event_target_class(context: &mut Context) {
    context
        .register_global_class::<EventTargetData>()
        .expect("EventTarget class should not already be registered");
}

/// Pull the `scope_key` off a method's `this` value. Returns
/// a TypeError matching the DOM spec's behaviour when an
/// EventTarget method is invoked on a non-EventTarget receiver.
fn scope_key_from_this(this: &JsValue) -> JsResult<String> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ().with_message("EventTarget method called on non-object"),
        )
    })?;
    let data = obj.downcast_ref::<EventTargetData>().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("EventTarget method called on object not produced by `new EventTarget()`"),
        )
    })?;
    Ok(data.scope_key.clone())
}

fn class_add_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope = scope_key_from_this(this)?;
    let type_ = args.get_or_undefined(0).clone();
    let listener = args.get_or_undefined(1).clone();
    add_listener_at_scope(&scope, &type_, &listener, context)?;
    Ok(JsValue::undefined())
}

fn class_remove_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope = scope_key_from_this(this)?;
    let type_ = args.get_or_undefined(0).clone();
    let listener = args.get_or_undefined(1).clone();
    remove_listener_at_scope(&scope, &type_, &listener, context)?;
    Ok(JsValue::undefined())
}

fn class_dispatch_event(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let scope = scope_key_from_this(this)?;
    dispatch_event_call(&scope, this, args, context)
}
