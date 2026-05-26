//! HTML timers — `setTimeout` / `clearTimeout` (plus the read side
//! of the hidden callback array used by [`crate::JsRuntime::pump_until_idle`]).
//!
//! [§ 8.6 Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
//!
//! ### Storage model
//!
//! Each call to `setTimeout(fn, delay)` does two things:
//!
//! 1. Push `fn` onto the hidden global JS array
//!    [`TIMERS_KEY`]. That array is what keeps the callback
//!    reachable from Boa's GC roots — without it, holding a
//!    `JsFunction` from plain Rust state across a context tick is
//!    not sound.
//! 2. Register a `(due_time, id)` pair with the
//!    [`crate::scheduler`] thread-local. `id` is the index into
//!    the array.
//!
//! `clearTimeout(id)` only updates the Rust scheduler — it leaves
//! the array slot in place, which is fine since the pump never
//! invokes cancelled ids.
//!
//! When the runtime's pump loop fires a timer it reads the
//! callback back out of the array by id and calls it on the
//! global object as `this`. The slot is set to `null` after
//! firing so the callback can be collected.
//!
//! ### Not implemented yet
//!
//! - `setInterval` / `clearInterval` (Phase 3 chunk 2).
//! - String-as-source `setTimeout("code", 0)`. Spec-deprecated and
//!   used by ~no one outside contrived eval tests.
//! - The HTML clamping rules (nesting level >= 5 ⇒ minimum 4ms).
//!   Out of scope; we honour the literal requested delay.

use std::time::Duration;

/// Cap on `setTimeout` delay arguments. 2^53 ms is past the f64
/// integer-precision boundary and over 285,000 years — practical
/// clamping for any caller that fits inside one universe.
const MAX_DELAY_MS: f64 = 9_007_199_254_740_992.0; // 1u64 << 53

use boa_engine::{
    Context, JsError, JsNativeError, JsResult, JsValue, NativeFunction, js_string,
    object::builtins::JsArray, property::Attribute,
};

use crate::scheduler;

/// Hidden global where pending timer callbacks are parked so Boa's
/// GC keeps them alive between the call to `setTimeout` and the
/// pump invocation that fires them. The leading `__koala_` is
/// strictly cosmetic — JS code that goes hunting for this and
/// mutates it deserves the broken-timer behaviour it gets.
const TIMERS_KEY: &str = "__koala_timers__";

/// Register `setTimeout`, `clearTimeout`, and the
/// [`TIMERS_KEY`] backing array on the given context. Called
/// from [`crate::globals::register_globals`] after the document
/// global so the pump can reference both from the same
/// well-known global object.
pub fn register_timers(context: &mut Context) {
    // Pre-create the callback storage array. setTimeout pushes into
    // it; the runtime's pump loop reads back from it.
    let arr = JsArray::new(context);
    context
        .register_global_property(js_string!(TIMERS_KEY), arr, Attribute::all())
        .expect("__koala_timers__ should not already exist");

    context
        .register_global_callable(
            js_string!("setTimeout"),
            2,
            NativeFunction::from_copy_closure(set_timeout),
        )
        .expect("setTimeout should not already be registered");
    context
        .register_global_callable(
            js_string!("clearTimeout"),
            1,
            NativeFunction::from_copy_closure(clear_timeout),
        )
        .expect("clearTimeout should not already be registered");
}

/// `setTimeout(handler, timeout = 0, ...args)` — schedule
/// `handler` to fire once after `timeout` ms.
///
/// We don't yet support the trailing `...args` form (extra
/// arguments forwarded to the handler) — partly because nobody
/// uses it and partly because we'd need to keep those `JsValue`s
/// alive too. Add if/when a test demands it.
fn set_timeout(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let callback = args.first().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ().with_message("setTimeout requires a callback"),
        )
    })?;
    // Per spec, a non-callable handler does NOT throw — it just
    // makes the call never fire (we'd need to coerce a string here
    // and run it as code, which we don't support). For us:
    // accept any value, push it, and let `pump` skip non-callables.
    // f64 → u64 with explicit clamp via `f64::clamp`. NaN goes to
    // 0 since `clamp(0.0, MAX)` propagates NaN to NaN and the
    // subsequent `as u64` cast turns NaN into 0.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let delay_ms = match args.get(1) {
        Some(v) => v.to_number(context)?.clamp(0.0, MAX_DELAY_MS) as u64,
        None => 0,
    };

    let timers = timers_array(context)?;
    let id_index = timers.length(context)?;
    let _ = timers.push(callback.clone(), context)?;

    // JS-visible id = array_index + 1, so id `0` stays a sentinel
    // (clearTimeout(undefined) doesn't accidentally hit a real
    // timer). The pump translates back by subtracting 1.
    let js_id = u32::try_from(id_index + 1).map_err(|_| {
        JsError::from_native(
            JsNativeError::range()
                .with_message("setTimeout: too many active timers"),
        )
    })?;
    scheduler::schedule(js_id, Duration::from_millis(delay_ms));

    Ok(JsValue::from(js_id))
}

/// `clearTimeout(id)` — cancel a pending timer. Silently ignores
/// invalid / stale / already-fired ids per spec.
#[allow(clippy::unnecessary_wraps)] // NativeFunction callback shape
fn clear_timeout(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let id = match args.first() {
        Some(v) => v.to_u32(context).unwrap_or(0),
        None => 0,
    };
    scheduler::cancel(id);
    Ok(JsValue::undefined())
}

/// Get the `__koala_timers__` global as a [`JsArray`] handle.
/// Used by both `setTimeout` and the runtime's pump.
pub(crate) fn timers_array(context: &mut Context) -> JsResult<JsArray> {
    let global = context.global_object();
    let value = global.get(js_string!(TIMERS_KEY), context)?;
    let object = value.as_object().cloned().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("__koala_timers__ is missing or not an object"),
        )
    })?;
    JsArray::from_object(object)
}
