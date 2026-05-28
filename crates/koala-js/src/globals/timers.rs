//! HTML timers — `setTimeout` / `setInterval` and their cancel
//! counterparts, plus the read side of the hidden callback array
//! used by [`crate::JsRuntime::pump_until_idle`].
//!
//! [§ 8.6 Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
//!
//! ### Storage model
//!
//! Each call to `setTimeout(fn, delay)` or
//! `setInterval(fn, period)` does two things:
//!
//! 1. Push `fn` onto the hidden global JS array
//!    [`TIMERS_KEY`]. That array is what keeps the callback
//!    reachable from Boa's GC roots — without it, holding a
//!    `JsFunction` from plain Rust state across a context tick is
//!    not sound.
//! 2. Register the timer with the [`crate::scheduler`]
//!    thread-local under id `array_index + 1`, with
//!    `repeat = None` for `setTimeout` and `repeat = Some(period)`
//!    for `setInterval`.
//!
//! `clearTimeout(id)` and `clearInterval(id)` both call into the
//! same [`crate::scheduler::cancel`] path. The spec treats timeout
//! and interval ids as a single id space, so passing an interval
//! id to `clearTimeout` (or vice versa) is fully supported and
//! tested.
//!
//! When the runtime's pump loop fires a timer it reads the
//! callback back out of the array by id and calls it on the
//! global object as `this`. For one-shots the slot is then set to
//! `null` so the callback can be collected; for intervals the
//! slot is left in place because the pump re-arms the same id for
//! the next firing.
//!
//! ### Not implemented yet
//!
//! - String-as-source `setTimeout("code", 0)`. Spec-deprecated and
//!   used by ~no one outside contrived eval tests.
//! - The HTML clamping rules (nesting level >= 5 ⇒ minimum 4ms).
//!   Out of scope; we honour the literal requested delay.

use std::time::Duration;

/// Cap on `setTimeout` / `setInterval` delay arguments. 2^53 ms is
/// past the f64 integer-precision boundary and over 285,000 years —
/// practical clamping for any caller that fits inside one universe.
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

/// Parallel hidden global indexed by the same `id - 1` key that
/// `__koala_timers__` uses, holding the trailing-argument list
/// each timer was scheduled with. Empty array when no extra args
/// were passed. See the module-level doc for the spec rationale.
const TIMER_ARGS_KEY: &str = "__koala_timer_args__";

/// Register `setTimeout`, `setInterval`, `clearTimeout`,
/// `clearInterval`, and the [`TIMERS_KEY`] backing array on the
/// given context. Called from
/// [`crate::globals::register_globals`] after the document global
/// so the pump can reference both from the same well-known global
/// object.
pub fn register_timers(context: &mut Context) {
    // Pre-create the callback storage array. setTimeout / setInterval
    // push into it; the runtime's pump loop reads back from it.
    let arr = JsArray::new(context);
    context
        .register_global_property(js_string!(TIMERS_KEY), arr, Attribute::all())
        .expect("__koala_timers__ should not already exist");

    // Parallel storage for trailing arguments. Same index space
    // as `__koala_timers__`; each slot is a JsArray (empty when
    // setTimeout was called without trailing args).
    let args_arr = JsArray::new(context);
    context
        .register_global_property(
            js_string!(TIMER_ARGS_KEY),
            args_arr,
            Attribute::all(),
        )
        .expect("__koala_timer_args__ should not already exist");

    context
        .register_global_callable(
            js_string!("setTimeout"),
            2,
            NativeFunction::from_copy_closure(set_timeout),
        )
        .expect("setTimeout should not already be registered");
    context
        .register_global_callable(
            js_string!("setInterval"),
            2,
            NativeFunction::from_copy_closure(set_interval),
        )
        .expect("setInterval should not already be registered");
    // clearTimeout and clearInterval share the same cancel path —
    // the spec uses a single id pool, so passing an interval id to
    // clearTimeout (or vice versa) is a supported operation.
    context
        .register_global_callable(
            js_string!("clearTimeout"),
            1,
            NativeFunction::from_copy_closure(clear_handle),
        )
        .expect("clearTimeout should not already be registered");
    context
        .register_global_callable(
            js_string!("clearInterval"),
            1,
            NativeFunction::from_copy_closure(clear_handle),
        )
        .expect("clearInterval should not already be registered");
}

/// `setTimeout(handler, timeout = 0, ...args)` — schedule
/// `handler` to fire once after `timeout` ms.
fn set_timeout(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    register_timer(args, context, /* is_interval */ false, "setTimeout")
}

/// `setInterval(handler, period = 0, ...args)` — schedule
/// `handler` to fire repeatedly every `period` ms. The same id is
/// re-armed by the pump after each firing; cancel via
/// `clearInterval` (or, per the spec's shared id pool,
/// `clearTimeout`).
fn set_interval(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    register_timer(args, context, /* is_interval */ true, "setInterval")
}

/// Shared implementation behind `setTimeout` and `setInterval`.
/// The only meaningful difference is whether the scheduler is
/// told to re-arm the same id after each firing.
///
/// Trailing `args.get(2..)` are captured into a parallel
/// `__koala_timer_args__` slot keyed by the same id-1 index, so
/// the pump can pass them back to the handler at firing time.
/// Intervals re-fire with the same captured args every time.
fn register_timer(
    args: &[JsValue],
    context: &mut Context,
    is_interval: bool,
    fn_name: &'static str,
) -> JsResult<JsValue> {
    let callback = args.first().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message(format!("{fn_name} requires a callback")),
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

    // Capture trailing args into a fresh JsArray BEFORE touching
    // the timers/args storage — the long-lived mutable borrow
    // ObjectInitializer-style builders take of `context` blocks
    // any further `context` calls, but `JsArray::new` only needs
    // a single borrow.
    let extra_args = JsArray::new(context);
    for arg in args.iter().skip(2) {
        let _ = extra_args.push(arg.clone(), context)?;
    }

    let timers = timers_array(context)?;
    let id_index = timers.length(context)?;
    let _ = timers.push(callback.clone(), context)?;

    let timer_args = timer_args_array(context)?;
    let _ = timer_args.push(extra_args, context)?;

    // JS-visible id = array_index + 1, so id `0` stays a sentinel
    // (clearTimeout(undefined) doesn't accidentally hit a real
    // timer). The pump translates back by subtracting 1.
    let js_id = u32::try_from(id_index + 1).map_err(|_| {
        JsError::from_native(
            JsNativeError::range()
                .with_message(format!("{fn_name}: too many active timers")),
        )
    })?;
    let delay = Duration::from_millis(delay_ms);
    // Intervals re-use the initial delay as their repeat period —
    // first firing is `delay` ms out, every subsequent firing
    // is another `delay` ms after the previous. This matches the
    // browser-visible semantics of `setInterval(fn, period)` and
    // avoids carrying a separate "initial delay" knob the spec
    // doesn't expose.
    let repeat = is_interval.then_some(delay);
    scheduler::schedule(js_id, delay, repeat);

    Ok(JsValue::from(js_id))
}

/// Shared backing for `clearTimeout(id)` and `clearInterval(id)`.
/// Silently ignores invalid / stale / already-fired ids per spec,
/// and treats the two as a single id space.
#[allow(clippy::unnecessary_wraps)] // NativeFunction callback shape
fn clear_handle(
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

/// Get the `__koala_timer_args__` global as a [`JsArray`]
/// handle. Indexed by the same id-1 key as
/// [`timers_array`]; each slot is itself a `JsArray` holding the
/// trailing arguments passed at scheduling time.
pub(crate) fn timer_args_array(context: &mut Context) -> JsResult<JsArray> {
    let global = context.global_object();
    let value = global.get(js_string!(TIMER_ARGS_KEY), context)?;
    let object = value.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("__koala_timer_args__ is missing or not an object"),
        )
    })?;
    JsArray::from_object(object)
}

/// Get the `__koala_timers__` global as a [`JsArray`] handle.
/// Used by both `setTimeout` and the runtime's pump.
pub(crate) fn timers_array(context: &mut Context) -> JsResult<JsArray> {
    let global = context.global_object();
    let value = global.get(js_string!(TIMERS_KEY), context)?;
    let object = value.as_object().ok_or_else(|| {
        JsError::from_native(
            JsNativeError::typ()
                .with_message("__koala_timers__ is missing or not an object"),
        )
    })?;
    JsArray::from_object(object)
}
