//! EventTarget mixin + `Event` constructor — the listener storage,
//! dispatch loop, and `new Event(type, options)` exposed on
//! `Window`, `document`, and `Element` for Phase 3 chunk 3.
//!
//! [§ 2.6 Interface EventTarget](https://dom.spec.whatwg.org/#interface-eventtarget) /
//! [§ 2.2 Interface Event](https://dom.spec.whatwg.org/#interface-event)
//!
//! ### Storage model
//!
//! Listeners live in a hidden global `__koala_listeners__` object.
//! The outer key is a scope string identifying the target:
//!
//! - `"window"` — the global object
//! - `"document"` — the `document` object
//! - `"node:<NodeId>"` — a specific element
//!
//! The value at each scope is itself an object keyed by event type
//! (`"DOMContentLoaded"`, `"click"`, …) holding a JS `Array` of
//! listener functions. Storing as plain JS objects keeps every
//! listener reachable through Boa's GC roots, the same trick the
//! timer scheduler uses with `__koala_timers__`.
//!
//! ### Dispatch shape (minimal, chunk 3)
//!
//! Dispatch is strict-target-only: it walks the listener array for
//! `(scope, type)` and invokes each callback with the synthesized
//! `Event` object as the sole argument. There is no capture phase,
//! no bubbling, and no parent-chain traversal — `bubbles = true`
//! on the event is stored for spec-correctness but not yet acted
//! on. The full propagation model (capture → target → bubble) is
//! deferred until tests demand it.
//!
//! `stopImmediatePropagation()` *does* work — it breaks the local
//! iteration loop so listeners later in the same array are
//! skipped. `stopPropagation()` is accepted but has no effect
//! today (it would suppress the bubble phase that doesn't exist
//! yet).
//!
//! ### Rust-callable surface
//!
//! [`dispatch_at_scope`] is the entry point used by
//! [`crate::JsRuntime::dispatch_dom_content_loaded`] and
//! [`crate::JsRuntime::dispatch_load`]. It builds an `Event`
//! object, sets `target` / `currentTarget` to a caller-supplied
//! `this` value, and runs the dispatch loop.

use boa_engine::{
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue,
    NativeFunction, js_string,
    object::{ObjectInitializer, builtins::JsArray},
    property::Attribute,
};

use super::helpers::type_error;

/// Hidden global where event listeners are parked so Boa's GC
/// keeps them alive across script ticks. Shape:
///
/// ```text
/// __koala_listeners__ = {
///   "window":   { "load": [fn, fn], "click": [fn] },
///   "document": { "DOMContentLoaded": [fn] },
///   "node:42":  { "click": [fn] },
///   ...
/// }
/// ```
pub(crate) const LISTENERS_KEY: &str = "__koala_listeners__";

/// Property on each `Event` object that holds the internal
/// "stopImmediatePropagation was called" flag. Read by the
/// dispatch loop to know when to abort. JS code never needs to
/// look at this directly — `stopImmediatePropagation()` is the
/// public surface.
const STOP_IMMEDIATE_KEY: &str = "__koala_stopImmediate";

/// Register the `Event` constructor and pre-create the
/// [`LISTENERS_KEY`] backing object. Called from
/// [`crate::globals::register_globals`].
pub(super) fn register_events(context: &mut Context) {
    let storage = JsObject::with_null_proto();
    context
        .register_global_property(
            js_string!(LISTENERS_KEY),
            JsValue::from(storage),
            Attribute::all(),
        )
        .expect("__koala_listeners__ should not already exist");

    context
        .register_global_callable(
            js_string!("Event"),
            1,
            NativeFunction::from_copy_closure(event_constructor),
        )
        .expect("Event should not already be registered");
}

/// `new Event(type, options)` — minimal constructor matching
/// [§ 2.2.1](https://dom.spec.whatwg.org/#dom-event-event).
///
/// Recognised options: `bubbles`, `cancelable`. `composed` is
/// accepted-but-ignored (no shadow tree to traverse). The
/// constructor returns a plain Boa object — there's no `Event`
/// prototype hierarchy because nothing yet does
/// `event instanceof Event`. Add when a test demands it.
fn event_constructor(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let type_ = args
        .first()
        .ok_or_else(|| type_error("Event constructor requires a type"))?
        .to_string(context)?;

    let (bubbles, cancelable) = match args.get(1).and_then(JsValue::as_object) {
        Some(opts) => {
            let bubbles = opts
                .get(js_string!("bubbles"), context)?
                .to_boolean();
            let cancelable = opts
                .get(js_string!("cancelable"), context)?
                .to_boolean();
            (bubbles, cancelable)
        }
        None => (false, false),
    };

    Ok(make_event_object(context, type_, bubbles, cancelable).into())
}

/// Build an Event object. Shared between the JS `Event`
/// constructor and the Rust-side dispatch helpers (so
/// lifecycle events like `DOMContentLoaded` use the same shape
/// that user code does).
pub(crate) fn make_event_object(
    context: &mut Context,
    type_: JsString,
    bubbles: bool,
    cancelable: bool,
) -> JsObject {
    let prevent_default = NativeFunction::from_copy_closure(prevent_default);
    let stop_propagation = NativeFunction::from_copy_closure(stop_propagation);
    let stop_immediate = NativeFunction::from_copy_closure(stop_immediate_propagation);

    ObjectInitializer::new(context)
        .property(js_string!("type"), type_, Attribute::READONLY)
        .property(js_string!("bubbles"), bubbles, Attribute::READONLY)
        .property(js_string!("cancelable"), cancelable, Attribute::READONLY)
        // target / currentTarget start as null; dispatch_at_scope
        // overwrites them before invoking listeners.
        .property(js_string!("target"), JsValue::null(), Attribute::all())
        .property(js_string!("currentTarget"), JsValue::null(), Attribute::all())
        .property(js_string!("defaultPrevented"), false, Attribute::all())
        .property(js_string!(STOP_IMMEDIATE_KEY), false, Attribute::all())
        .function(prevent_default, js_string!("preventDefault"), 0)
        .function(stop_propagation, js_string!("stopPropagation"), 0)
        .function(stop_immediate, js_string!("stopImmediatePropagation"), 0)
        .build()
}

/// `Event.prototype.preventDefault()` — set `defaultPrevented`
/// when the event is cancelable. No-op otherwise.
fn prevent_default(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    if let Some(obj) = this.as_object() {
        let cancelable = obj
            .get(js_string!("cancelable"), context)?
            .to_boolean();
        if cancelable {
            let _ = obj.set(
                js_string!("defaultPrevented"),
                JsValue::from(true),
                false,
                context,
            )?;
        }
    }
    Ok(JsValue::undefined())
}

/// `Event.prototype.stopPropagation()` — accepted-but-no-op until
/// the bubble phase is implemented. Documented as a known
/// limitation in the module-level comment.
#[allow(clippy::unnecessary_wraps)] // NativeFunction shape
fn stop_propagation(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

/// `Event.prototype.stopImmediatePropagation()` — flag the event
/// so the running dispatch loop skips remaining listeners.
fn stop_immediate_propagation(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    if let Some(obj) = this.as_object() {
        let _ = obj.set(
            js_string!(STOP_IMMEDIATE_KEY),
            JsValue::from(true),
            false,
            context,
        )?;
    }
    Ok(JsValue::undefined())
}

/// Push `listener` onto the array at `__koala_listeners__[scope][type]`,
/// creating the inner object / array as needed. No-op when
/// `listener` isn't callable, per spec § 2.7.2 (DOM Level 4) which
/// says non-callable listeners are silently ignored.
///
/// We do not deduplicate — re-registering the same listener
/// produces two firings. The spec requires dedup by
/// `(callback, capture)`; that's a chunk-3 follow-up.
pub(super) fn add_listener_at_scope(
    scope: &str,
    type_: &JsValue,
    listener: &JsValue,
    context: &mut Context,
) -> JsResult<()> {
    let Some(callback) = listener.as_object() else {
        return Ok(());
    };
    if !callback.is_callable() {
        return Ok(());
    }
    let type_str = type_.to_string(context)?;
    let bucket = ensure_bucket(scope, &type_str, context)?;
    let _ = bucket.push(listener.clone(), context)?;
    Ok(())
}

/// Remove `listener` from the array at
/// `__koala_listeners__[scope][type]`. Silently ignores
/// non-existent scope / type / callback combinations.
///
/// Comparison is strict equality (same JS object reference). The
/// spec compares by `(callback, capture)`; since we don't track
/// capture, callback identity alone is sufficient for now.
pub(super) fn remove_listener_at_scope(
    scope: &str,
    type_: &JsValue,
    listener: &JsValue,
    context: &mut Context,
) -> JsResult<()> {
    let type_str = type_.to_string(context)?;
    let Some(bucket) = lookup_bucket(scope, &type_str.to_std_string_escaped(), context)? else {
        return Ok(());
    };
    let len = bucket.length(context)?;
    for i in 0..len {
        let existing = bucket.get(i, context)?;
        if JsValue::strict_equals(&existing, listener) {
            // Splice out one element at index `i`: shift the tail
            // down by one then pop the now-duplicated last slot.
            // O(n) but fine — listener lists are small.
            for j in i..len.saturating_sub(1) {
                let next = bucket.get(j + 1, context)?;
                let _ = bucket.set(j, next, false, context)?;
            }
            let _ = bucket.pop(context)?;
            return Ok(());
        }
    }
    Ok(())
}

/// Walk the listener array at `__koala_listeners__[scope][type]`,
/// invoking each callback with `event` as the sole argument and
/// `this_value` as the receiver. Stops early if any callback calls
/// `event.stopImmediatePropagation()`.
///
/// Errors thrown by a listener short-circuit dispatch and bubble
/// up to the caller. The spec says errors should be reported to
/// the global error handler without aborting subsequent listeners,
/// but we don't have a global error handler yet — propagating the
/// error matches what `execute()` already does for synchronous
/// scripts.
pub(crate) fn dispatch_at_scope(
    scope: &str,
    this_value: &JsValue,
    event: &JsObject,
    context: &mut Context,
) -> JsResult<()> {
    // currentTarget mirrors target in the strict-target-only
    // model — both are set to `this_value` for the duration of
    // dispatch. When bubbling lands, currentTarget will rotate
    // per ancestor while target stays pinned to the original.
    let _ = event.set(js_string!("target"), this_value.clone(), false, context)?;
    let _ = event.set(
        js_string!("currentTarget"),
        this_value.clone(),
        false,
        context,
    )?;
    let _ = event.set(
        js_string!(STOP_IMMEDIATE_KEY),
        JsValue::from(false),
        false,
        context,
    )?;

    let type_value = event.get(js_string!("type"), context)?;
    let type_str = type_value.to_string(context)?;
    let Some(bucket) = lookup_bucket(scope, &type_str.to_std_string_escaped(), context)? else {
        return Ok(());
    };

    let event_arg = [JsValue::from(event.clone())];
    // Snapshot the length up front: spec § 2.10 says listeners
    // added during dispatch don't fire for the current event.
    let len = bucket.length(context)?;
    for i in 0..len {
        let cb_value = bucket.get(i, context)?;
        let Some(cb) = cb_value.as_object() else { continue };
        if !cb.is_callable() {
            continue;
        }
        let _ = cb.call(this_value, &event_arg, context)?;

        let stop = event
            .get(js_string!(STOP_IMMEDIATE_KEY), context)?
            .to_boolean();
        if stop {
            break;
        }
    }
    Ok(())
}

/// JS-side `dispatchEvent(event)` — uniform body for the window /
/// document / element variants. The scope is passed in; the
/// caller-supplied `this` value is what landlords get exposed as
/// the event's `target` and `currentTarget`.
///
/// Returns `!event.defaultPrevented`, per spec.
pub(super) fn dispatch_event_call(
    scope: &str,
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let event_value = args.get_or_undefined(0);
    let event = event_value.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("dispatchEvent requires an Event object"),
        )
    })?;
    dispatch_at_scope(scope, this, &event, context)?;
    let prevented = event
        .get(js_string!("defaultPrevented"), context)?
        .to_boolean();
    Ok(JsValue::from(!prevented))
}

/// Get-or-create the listener array at `__koala_listeners__[scope][type]`.
/// Inserts an empty inner object for `scope` and an empty array
/// for `type` if either is missing.
fn ensure_bucket(
    scope: &str,
    type_: &JsString,
    context: &mut Context,
) -> JsResult<JsArray> {
    let storage = listeners_storage(context)?;
    let scope_key = JsString::from(scope);
    let scope_obj = match storage.get(scope_key.clone(), context)?.as_object() {
        Some(o) => o.clone(),
        None => {
            let fresh = JsObject::with_null_proto();
            let _ = storage.set(
                scope_key,
                JsValue::from(fresh.clone()),
                false,
                context,
            )?;
            fresh
        }
    };
    let bucket = match scope_obj.get(type_.clone(), context)?.as_object() {
        Some(o) => JsArray::from_object(o.clone())?,
        None => {
            let fresh = JsArray::new(context);
            let _ = scope_obj.set(
                type_.clone(),
                JsValue::from(fresh.clone()),
                false,
                context,
            )?;
            fresh
        }
    };
    Ok(bucket)
}

/// Read the listener array at `__koala_listeners__[scope][type]`
/// without creating it. Returns `Ok(None)` when no listener has
/// ever been registered for the pair.
fn lookup_bucket(
    scope: &str,
    type_: &str,
    context: &mut Context,
) -> JsResult<Option<JsArray>> {
    let storage = listeners_storage(context)?;
    let scope_value = storage.get(JsString::from(scope), context)?;
    let Some(scope_obj) = scope_value.as_object() else {
        return Ok(None);
    };
    let bucket_value = scope_obj.get(JsString::from(type_), context)?;
    let Some(bucket_obj) = bucket_value.as_object() else {
        return Ok(None);
    };
    Ok(Some(JsArray::from_object(bucket_obj.clone())?))
}

/// Get the `__koala_listeners__` global as a [`JsObject`]. Errors
/// only if a malicious script has replaced the property with a
/// primitive.
fn listeners_storage(context: &mut Context) -> JsResult<JsObject> {
    let global = context.global_object();
    let value = global.get(js_string!(LISTENERS_KEY), context)?;
    value.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("__koala_listeners__ is missing or not an object"),
        )
    })
}
