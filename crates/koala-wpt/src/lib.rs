//! WPT-runner-specific glue for the Koala browser engine.
//!
//! This crate hosts the Rust-side surface that the
//! [Web Platform Tests](https://web-platform-tests.org/)
//! runner needs but a real browsing context does not — chiefly
//! the [testharness.js] result bridge that captures structured
//! test results from JS into a Rust-readable buffer.
//!
//! The split keeps `koala-js` honest: `__koala_test_results__`
//! and friends are not browser standards, so they have no
//! business living in the generic JS-runtime crate. WPT-only
//! consumers (the koala-cli `--wpt-protocol` mode, the eventual
//! testharnessreport.js installer) pull from this crate; regular
//! koala rendering doesn't.
//!
//! # Usage
//!
//! [`install`] sets up the capture functions
//! (`__koala_emit_result__` / `__koala_emit_completion__`) plus
//! fallback `add_result_callback` / `add_completion_callback`
//! globals on a fresh [`JsRuntime`].
//!
//! Test results then flow into the capture buffer through
//! whatever path is in use:
//!
//! - **WPT runs**: wptrunner serves a koala-specific
//!   `testharnessreport.js` (configured via
//!   `env_options()["testharnessreport"]` in the Python plugin)
//!   that registers the capture functions through testharness.js's
//!   real `add_result_callback`. This is how every other
//!   WPT-integrated browser works — see Servo's
//!   `testharnessreport-servo.js` and WKTR's
//!   `testharnessreport-wktr.js` for the same pattern.
//! - **Direct / non-WPT use**: scripts can call
//!   `__koala_emit_result__(test)` themselves. The fallback
//!   `add_result_callback` registered by [`install`] also wires
//!   user callbacks through the capture path when real
//!   testharness.js isn't loaded.
//!
//! ```no_run
//! use koala_dom::DomTree;
//! use koala_js::JsRuntime;
//! use std::{cell::RefCell, rc::Rc};
//!
//! let dom = Rc::new(RefCell::new(DomTree::new()));
//! let mut rt = JsRuntime::new(dom);
//! koala_wpt::install(&mut rt);
//! ```
//!
//! After the document has settled (post-`load`, post-pump), drain:
//!
//! ```no_run
//! # use koala_dom::DomTree;
//! # use koala_js::JsRuntime;
//! # use std::{cell::RefCell, rc::Rc};
//! # let dom = Rc::new(RefCell::new(DomTree::new()));
//! # let mut rt = JsRuntime::new(dom);
//! # koala_wpt::install(&mut rt);
//! for result in koala_wpt::take_test_results(&mut rt).unwrap() {
//!     println!("{}: status={}", result.name, result.status);
//! }
//! if let Some(completion) = koala_wpt::take_test_completion(&mut rt).unwrap() {
//!     println!("harness status: {}", completion.status);
//! }
//! ```
//!
//! [`JsRuntime`]: koala_js::JsRuntime
//! [testharness.js]: https://web-platform-tests.org/writing-tests/testharness-api.html

mod testharness;

use boa_engine::{JsError, js_string};
use koala_js::JsRuntime;

/// One captured WPT test result.
///
/// [WPT testharness API — Test object](https://web-platform-tests.org/writing-tests/testharness-api.html#Test)
///
/// Populated from a `__koala_emit_result__(test)` call on the JS
/// side and surfaced to host code via [`take_test_results`].
#[derive(Debug, Clone)]
pub struct TestharnessResult {
    /// Test name, as set in the JS source. Empty if the test
    /// object had no `name` property.
    pub name: String,
    /// Numeric status code: 0 = PASS, 1 = FAIL, 2 = TIMEOUT,
    /// 3 = NOTRUN, 4 = PRECONDITION_FAILED. Out-of-range values
    /// are preserved as-is so the executor can surface them
    /// rather than silently mapping to FAIL.
    pub status: u32,
    /// Diagnostic message (e.g. assertion failure detail).
    /// Empty when the test passed cleanly.
    pub message: String,
    /// JS stack trace at the point of failure, when available.
    /// Empty when the test passed cleanly.
    pub stack: String,
}

/// Final harness status reported by the testharness.js completion
/// callback.
///
/// [WPT testharness API — TestsStatus](https://web-platform-tests.org/writing-tests/testharness-api.html#TestsStatus)
#[derive(Debug, Clone)]
pub struct TestharnessCompletion {
    /// Numeric overall status: 0 = OK, 1 = ERROR, 2 = TIMEOUT,
    /// 3 = PRECONDITION_FAILED.
    pub status: u32,
    /// Diagnostic message; empty in the clean OK case.
    pub message: String,
}

/// Install the testharness.js result bridge on `runtime`.
///
/// Registers the fallback `add_result_callback` /
/// `add_completion_callback` globals along with the hidden
/// `__koala_emit_result__` / `__koala_emit_completion__` capture
/// functions, plus the four backing storage globals.
///
/// Idempotency is not promised — call this once per
/// [`JsRuntime`], before any script executes against it.
pub fn install(runtime: &mut JsRuntime) {
    runtime.with_context_mut(testharness::register_testharness);
}


/// Drain every WPT test result captured since the last call.
///
/// Pulls entries out of the hidden `__koala_test_results__`
/// global, decoding each into a [`TestharnessResult`]. The
/// underlying JS array is replaced with a fresh empty one so a
/// subsequent call returns nothing (drain semantics, matching
/// `JsRuntime::take_dom_dirty`).
///
/// The koala-cli WPT executor calls this once after the document
/// settles — after the lifecycle `load` event has fired and the
/// pump has drained — to ferry results back over the stdout
/// protocol.
///
/// # Errors
///
/// Returns a [`JsError`] if the hidden buffer has been replaced
/// with a non-array value (only possible if a malicious script
/// clobbered it).
pub fn take_test_results(
    runtime: &mut JsRuntime,
) -> Result<Vec<TestharnessResult>, JsError> {
    runtime.with_context_mut(|context| {
        let global = context.global_object();
        let value = global.get(js_string!(testharness::RESULTS_KEY), context)?;
        let object = value.as_object().ok_or_else(|| {
            JsError::from_native(
                boa_engine::JsNativeError::typ()
                    .with_message("__koala_test_results__ is missing or not an object"),
            )
        })?;
        let array = boa_engine::object::builtins::JsArray::from_object(object)?;

        let len = array.length(context)?;
        let mut out = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
        for i in 0..len {
            let entry_value = array.get(i, context)?;
            let entry = entry_value.as_object().ok_or_else(|| {
                JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("test result entry is not an object"),
                )
            })?;
            out.push(TestharnessResult {
                name: object_field_string(&entry, "name", context)?,
                status: object_field_u32(&entry, "status", context)?,
                message: object_field_string(&entry, "message", context)?,
                stack: object_field_string(&entry, "stack", context)?,
            });
        }

        // Replace the captured array with a fresh empty one. We
        // can't truncate in place via `JsArray` — no `set_length`
        // accessor — and rebinding the slot is the cleanest drain.
        let fresh = boa_engine::object::builtins::JsArray::new(context);
        let _ = global.set(
            js_string!(testharness::RESULTS_KEY),
            boa_engine::JsValue::from(fresh),
            false,
            context,
        )?;
        Ok(out)
    })
}

/// Return `true` if the testharness completion callback has
/// fired (the `__koala_test_completion__` JS slot is non-null /
/// non-undefined).
///
/// This is a non-draining peek — repeated calls keep returning
/// `true` until [`take_test_completion`] drains the slot back to
/// `null`. Intended for use as the stop predicate of
/// [`JsRuntime::pump_until_idle_or`](koala_js::JsRuntime::pump_until_idle_or):
/// once the harness has emitted its overall verdict, the pump
/// has no reason to keep sleeping for stray watchdog timers.
///
/// # Errors
///
/// Returns a [`JsError`] only when the slot has been replaced
/// with a value that is neither null nor an object — symptomatic
/// of a malicious script having clobbered the buffer.
pub fn has_test_completion(runtime: &mut JsRuntime) -> Result<bool, JsError> {
    runtime.with_context_mut(|context| {
        let global = context.global_object();
        let value = global.get(js_string!(testharness::COMPLETION_KEY), context)?;
        Ok(!value.is_null_or_undefined())
    })
}

/// Drain the harness completion payload, if any.
///
/// Returns `None` until `__koala_emit_completion__` has fired at
/// least once on this runtime; after a drain the slot resets to
/// `null` so a second call is `None` until the next emission.
///
/// # Errors
///
/// Returns a [`JsError`] only when the slot has been replaced
/// with a value that is neither `null` nor an object.
pub fn take_test_completion(
    runtime: &mut JsRuntime,
) -> Result<Option<TestharnessCompletion>, JsError> {
    runtime.with_context_mut(|context| {
        let global = context.global_object();
        let value = global.get(js_string!(testharness::COMPLETION_KEY), context)?;
        if value.is_null_or_undefined() {
            return Ok(None);
        }
        let entry = value.as_object().ok_or_else(|| {
            JsError::from_native(
                boa_engine::JsNativeError::typ()
                    .with_message("__koala_test_completion__ must be null or an object"),
            )
        })?;
        let completion = TestharnessCompletion {
            status: object_field_u32(&entry, "status", context)?,
            message: object_field_string(&entry, "message", context)?,
        };
        let _ = global.set(
            js_string!(testharness::COMPLETION_KEY),
            boa_engine::JsValue::null(),
            false,
            context,
        )?;
        Ok(Some(completion))
    })
}

/// Helper: read `field` off `obj` as a Rust `String`. Missing or
/// undefined fields decode to `""`.
fn object_field_string(
    obj: &boa_engine::JsObject,
    field: &str,
    context: &mut boa_engine::Context,
) -> Result<String, JsError> {
    let value = obj.get(boa_engine::JsString::from(field), context)?;
    if value.is_null_or_undefined() {
        return Ok(String::new());
    }
    Ok(value.to_string(context)?.to_std_string_escaped())
}

/// Helper: read `field` off `obj` as `u32`. Missing fields
/// decode to `0`.
fn object_field_u32(
    obj: &boa_engine::JsObject,
    field: &str,
    context: &mut boa_engine::Context,
) -> Result<u32, JsError> {
    let value = obj.get(boa_engine::JsString::from(field), context)?;
    if value.is_null_or_undefined() {
        return Ok(0);
    }
    value.to_u32(context)
}
