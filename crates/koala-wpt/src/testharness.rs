//! testharness.js result bridge — capture Test / TestStatus
//! payloads into a Rust-readable buffer for the WPT executor.
//!
//! [WPT testharness API](https://web-platform-tests.org/writing-tests/testharness-api.html)
//!
//! ### How testharness.js reports results
//!
//! When a WPT test file runs, [testharness.js] defines two
//! registration globals:
//!
//! - `add_result_callback(cb)` — `cb(test)` is invoked once per
//!   test, where `test` carries `{ name, status, message, stack }`.
//!   `status` is a numeric enum (0=PASS, 1=FAIL, 2=TIMEOUT,
//!   3=NOTRUN).
//! - `add_completion_callback(cb)` — `cb(tests, status)` is
//!   invoked exactly once when the harness finishes. `status`
//!   carries `{ status, message }` (0=OK, 1=ERROR, 2=TIMEOUT).
//!
//! The browser host registers callbacks via these globals and
//! ferries the payloads back to the test runner. For koala the
//! ferry runs through a hidden global buffer that the Rust side
//! drains via [`crate::JsRuntime::take_test_results`] /
//! [`crate::JsRuntime::take_test_completion`].
//!
//! ### Storage model
//!
//! All four hidden globals are owned by the koala harness:
//!
//! - `__koala_test_results__` — JS `Array` of captured result
//!   payload objects (each `{ name, status, message, stack }`).
//! - `__koala_test_completion__` — `null` until completion fires,
//!   then `{ status, message }`.
//! - `__koala_result_callbacks__` — JS `Array` of callbacks
//!   registered via [`add_result_callback`].
//! - `__koala_completion_callbacks__` — same for completion.
//!
//! `__koala_emit_result__(test)` and `__koala_emit_completion__(tests, status)`
//! are the capture entry points: they write to the buffer **and**
//! fan out to every registered callback. The capture step is what
//! the Rust side relies on; the fan-out keeps the API compatible
//! with code that registers its own listeners.
//!
//! ### testharness.js compatibility (deferred to chunk 5.2)
//!
//! Real testharness.js, once loaded via `<script src>`, defines
//! its own `add_result_callback` / `add_completion_callback` and
//! will overwrite the native fallbacks below. Chunk 5.2 will land
//! an injected reporter script that registers
//! [`__koala_emit_result__`] / [`__koala_emit_completion__`]
//! through testharness.js's real implementations so the buffer
//! still fills. The hidden buffers and emit functions are
//! deliberately scoped under `__koala_*` so testharness.js
//! cannot accidentally clobber them.
//!
//! [testharness.js]: https://github.com/web-platform-tests/wpt/blob/master/resources/testharness.js

use boa_engine::{
    Context, JsArgs, JsResult, JsValue, NativeFunction, js_string,
    object::builtins::JsArray, property::Attribute,
};

/// JS Array of captured result payload objects. Drained by
/// [`crate::JsRuntime::take_test_results`].
pub(crate) const RESULTS_KEY: &str = "__koala_test_results__";

/// JS slot for the completion payload. `null` until the harness
/// fires its completion callback; then a `{ status, message }`
/// object. Drained by [`crate::JsRuntime::take_test_completion`].
pub(crate) const COMPLETION_KEY: &str = "__koala_test_completion__";

/// JS Array of user callbacks registered via the fallback
/// [`add_result_callback`].
const RESULT_CBS_KEY: &str = "__koala_result_callbacks__";

/// JS Array of user callbacks registered via the fallback
/// [`add_completion_callback`].
const COMPLETION_CBS_KEY: &str = "__koala_completion_callbacks__";

/// Register the bridge globals on `context`. Called from
/// [`crate::globals::register_globals`].
pub(super) fn register_testharness(context: &mut Context) {
    let results = JsArray::new(context);
    context
        .register_global_property(js_string!(RESULTS_KEY), results, Attribute::all())
        .expect("__koala_test_results__ should not already exist");

    context
        .register_global_property(
            js_string!(COMPLETION_KEY),
            JsValue::null(),
            Attribute::all(),
        )
        .expect("__koala_test_completion__ should not already exist");

    let result_cbs = JsArray::new(context);
    context
        .register_global_property(
            js_string!(RESULT_CBS_KEY),
            result_cbs,
            Attribute::all(),
        )
        .expect("__koala_result_callbacks__ should not already exist");

    let completion_cbs = JsArray::new(context);
    context
        .register_global_property(
            js_string!(COMPLETION_CBS_KEY),
            completion_cbs,
            Attribute::all(),
        )
        .expect("__koala_completion_callbacks__ should not already exist");

    context
        .register_global_callable(
            js_string!("add_result_callback"),
            1,
            NativeFunction::from_copy_closure(add_result_callback),
        )
        .expect("add_result_callback should not already be registered");
    context
        .register_global_callable(
            js_string!("add_completion_callback"),
            1,
            NativeFunction::from_copy_closure(add_completion_callback),
        )
        .expect("add_completion_callback should not already be registered");
    context
        .register_global_callable(
            js_string!("__koala_emit_result__"),
            1,
            NativeFunction::from_copy_closure(emit_result),
        )
        .expect("__koala_emit_result__ should not already be registered");
    context
        .register_global_callable(
            js_string!("__koala_emit_completion__"),
            2,
            NativeFunction::from_copy_closure(emit_completion),
        )
        .expect("__koala_emit_completion__ should not already be registered");
}

/// `add_result_callback(cb)` — fallback registration used when
/// real testharness.js hasn't loaded. Stores `cb` in the hidden
/// callback array so [`emit_result`] can fan out to it.
///
/// Non-callable arguments are silently ignored to match
/// testharness.js's tolerance for `null` / `undefined` from
/// instrumentation harnesses.
fn add_result_callback(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let cb = args.get_or_undefined(0);
    push_callback(RESULT_CBS_KEY, cb, context)
}

/// `add_completion_callback(cb)` — fallback registration; same
/// shape as [`add_result_callback`].
fn add_completion_callback(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let cb = args.get_or_undefined(0);
    push_callback(COMPLETION_CBS_KEY, cb, context)
}

/// `__koala_emit_result__(test)` — capture a result payload into
/// `__koala_test_results__` **and** fan out to every callback
/// registered via [`add_result_callback`].
///
/// The capture step is the load-bearing path: the Rust harness
/// reads the buffer after script execution settles. The fan-out
/// step preserves the testharness.js semantics that any number
/// of independent callbacks can observe results.
fn emit_result(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let test = args.get_or_undefined(0);
    capture_result(test, context)?;
    fan_out(RESULT_CBS_KEY, &[test.clone()], context)?;
    Ok(JsValue::undefined())
}

/// `__koala_emit_completion__(tests, status)` — capture the
/// completion payload and fan out to every callback registered
/// via [`add_completion_callback`].
fn emit_completion(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let tests = args.get_or_undefined(0);
    let status = args.get_or_undefined(1);
    capture_completion(status, context)?;
    fan_out(COMPLETION_CBS_KEY, &[tests.clone(), status.clone()], context)?;
    Ok(JsValue::undefined())
}

/// Push `cb` to `__koala_<kind>_callbacks__` if it is callable.
/// No-op for non-callable arguments.
fn push_callback(
    key: &str,
    cb: &JsValue,
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(obj) = cb.as_object() else {
        return Ok(JsValue::undefined());
    };
    if !obj.is_callable() {
        return Ok(JsValue::undefined());
    }
    let callbacks = lookup_callback_array(key, context)?;
    let _ = callbacks.push(cb.clone(), context)?;
    Ok(JsValue::undefined())
}

/// Pull `name`, `status`, `message`, `stack` off `test` and push
/// a flat result object onto `__koala_test_results__`. Missing
/// fields default to `""` for strings and `0` for status, so a
/// half-formed test object doesn't blow up the capture path.
///
/// Fields are read out first because `ObjectInitializer::new`
/// takes a long-lived mutable borrow of the context, blocking
/// further `Context` calls inside the builder chain.
fn capture_result(test: &JsValue, context: &mut Context) -> JsResult<()> {
    let name = read_string_field(test, "name", context)?;
    let status = read_u32_field(test, "status", context)?;
    let message = read_string_field(test, "message", context)?;
    let stack = read_string_field(test, "stack", context)?;
    let entry = boa_engine::object::ObjectInitializer::new(context)
        .property(js_string!("name"), name, Attribute::all())
        .property(js_string!("status"), status, Attribute::all())
        .property(js_string!("message"), message, Attribute::all())
        .property(js_string!("stack"), stack, Attribute::all())
        .build();
    let buffer = lookup_callback_array(RESULTS_KEY, context)?;
    let _ = buffer.push(JsValue::from(entry), context)?;
    Ok(())
}

/// Pull `status` and `message` off the completion object and
/// install a fresh `{ status, message }` object at
/// `__koala_test_completion__`. Same read-first-build-second
/// dance as [`capture_result`].
fn capture_completion(status: &JsValue, context: &mut Context) -> JsResult<()> {
    let status_value = read_u32_field(status, "status", context)?;
    let message = read_string_field(status, "message", context)?;
    let entry = boa_engine::object::ObjectInitializer::new(context)
        .property(js_string!("status"), status_value, Attribute::all())
        .property(js_string!("message"), message, Attribute::all())
        .build();
    let global = context.global_object();
    let _ = global.set(
        js_string!(COMPLETION_KEY),
        JsValue::from(entry),
        false,
        context,
    )?;
    Ok(())
}

/// Invoke every callback in `__koala_<kind>_callbacks__` with
/// `args`. Snapshots the array length up front so a callback
/// registering further callbacks during dispatch doesn't expand
/// the iteration in flight (matches the spec-compatible
/// once-around contract used by EventTarget dispatch).
fn fan_out(
    key: &str,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<()> {
    let callbacks = lookup_callback_array(key, context)?;
    let len = callbacks.length(context)?;
    let global = JsValue::from(context.global_object());
    for i in 0..len {
        let cb_value = callbacks.get(i, context)?;
        let Some(cb) = cb_value.as_object() else { continue };
        if !cb.is_callable() {
            continue;
        }
        let _ = cb.call(&global, args, context)?;
    }
    Ok(())
}

/// Read `field` off `obj`, coerce to string. Missing or
/// undefined fields decode to `""`.
fn read_string_field(
    obj: &JsValue,
    field: &str,
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(target) = obj.as_object() else {
        return Ok(JsValue::from(js_string!("")));
    };
    let value = target.get(boa_engine::JsString::from(field), context)?;
    if value.is_null_or_undefined() {
        return Ok(JsValue::from(js_string!("")));
    }
    Ok(JsValue::from(value.to_string(context)?))
}

/// Read `field` off `obj`, coerce to u32 number. Missing fields
/// decode to `0`. `to_u32` clamps NaN / Infinity / negative
/// values to 0 per ECMAScript ToUint32, which happens to be
/// exactly what we want for an unknown-status fallback.
fn read_u32_field(
    obj: &JsValue,
    field: &str,
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(target) = obj.as_object() else {
        return Ok(JsValue::from(0u32));
    };
    let value = target.get(boa_engine::JsString::from(field), context)?;
    if value.is_null_or_undefined() {
        return Ok(JsValue::from(0u32));
    }
    Ok(JsValue::from(value.to_u32(context)?))
}

/// Read the named hidden array off the global object. Errors
/// only if a malicious script has replaced the property with a
/// non-array value.
fn lookup_callback_array(
    key: &str,
    context: &mut Context,
) -> JsResult<JsArray> {
    let global = context.global_object();
    let value = global.get(boa_engine::JsString::from(key), context)?;
    let object = value.as_object().cloned().ok_or_else(|| {
        boa_engine::JsError::from_native(
            boa_engine::JsNativeError::typ()
                .with_message(format!("{key} is missing or not an object")),
        )
    })?;
    JsArray::from_object(object)
}
