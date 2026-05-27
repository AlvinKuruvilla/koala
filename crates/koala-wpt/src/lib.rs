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
//! The full WPT-runner flow needs two installation steps in
//! sequence, both driven by the host's [`JsHooks`]
//! implementation:
//!
//! 1. [`install`] runs **before** any document script — registers
//!    the koala-wpt fallback globals + the
//!    `__koala_emit_result__` / `__koala_emit_completion__`
//!    capture functions.
//! 2. [`attach_reporter`] runs **after** every document script
//!    has loaded (and crucially, after `testharness.js` has had
//!    a chance to overwrite koala-wpt's fallback
//!    `add_result_callback` / `add_completion_callback` with its
//!    own implementations). The reporter then binds the capture
//!    functions through the real testharness.js machinery and
//!    replays any tests that completed synchronously before the
//!    binding ran.
//!
//! Install the bridge once on a fresh [`JsRuntime`] before any
//! script runs:
//!
//! [`JsHooks`]: koala_browser::JsHooks
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

/// JS source for the testharness.js reporter binding. Run after
/// every document `<script>` has executed (i.e. after
/// testharness.js itself has loaded) so that subsequent test
/// completions flow through `__koala_emit_result__` /
/// `__koala_emit_completion__` and into the Rust-side buffers.
///
/// Two behaviours, both guarded against the case where the
/// document didn't load testharness.js at all:
///
/// 1. Register listeners for **future** test results / completion
///    via testharness.js's real `add_result_callback` /
///    `add_completion_callback` — these are now the testharness.js
///    implementations (they overwrite koala-wpt's fallbacks when
///    testharness.js loads).
///
/// 2. **Replay** any tests that already completed before the
///    reporter ran. testharness.js's `add_result_callback`
///    appends to a callback list without replaying historical
///    results, so synchronous `test(...)` calls that ran during
///    the document's own scripts would otherwise be lost. We
///    walk testharness.js's internal `tests.tests` collection
///    and emit any entry whose phase is `COMPLETE` directly.
///
/// The replay step depends on testharness.js internals
/// (`tests` global, `tests.tests` array, `Test.prototype.phases`).
/// That's brittle if upstream rearranges, but it's the cheapest
/// way to capture sync tests without modifying or vendoring
/// testharness.js itself.
const TESTHARNESS_REPORTER: &str = r#"
(function () {
    if (typeof add_result_callback === 'function') {
        add_result_callback(function (test) {
            __koala_emit_result__(test);
        });
    }
    if (typeof add_completion_callback === 'function') {
        add_completion_callback(function (tests_, status) {
            __koala_emit_completion__(tests_, status);
        });
    }
    // Replay tests that completed before the reporter bound —
    // testharness.js doesn't do this for us.
    if (typeof tests !== 'undefined' && tests && Array.isArray(tests.tests)) {
        var completePhase = (
            typeof Test !== 'undefined' &&
            Test.prototype &&
            Test.prototype.phases &&
            Test.prototype.phases.COMPLETE
        );
        if (typeof completePhase !== 'number') {
            // Fallback if Test.prototype.phases isn't introspectable:
            // testharness.js has used 4 for COMPLETE since at least
            // 2014. Treat anything with a non-null `status` as
            // complete enough to replay.
            completePhase = 4;
        }
        for (var i = 0; i < tests.tests.length; i++) {
            var t = tests.tests[i];
            if (t && (t.phase === completePhase || t.status !== null)) {
                __koala_emit_result__(t);
            }
        }
    }
})();
"#;

/// Bind the koala-wpt capture functions through testharness.js's
/// `add_result_callback` / `add_completion_callback` (and replay
/// any already-completed tests).
///
/// Call this AFTER every document `<script>` has run — i.e. from
/// the [`JsHooks::after_scripts`] hook point. Calling earlier
/// would register against koala-wpt's fallback `add_result_callback`,
/// which testharness.js will then overwrite, losing the binding.
///
/// No-op when testharness.js never loaded: the `typeof` guards in
/// the embedded script make this safe to run on any document.
///
/// # Errors
///
/// Returns the [`JsError`] from any script-execution failure
/// while installing the reporter. In practice this only fires if
/// testharness.js's globals are themselves throwing on access —
/// which would have failed any test under the harness anyway.
///
/// [`JsHooks::after_scripts`]: koala_browser::JsHooks::after_scripts
pub fn attach_reporter(runtime: &mut JsRuntime) -> Result<(), JsError> {
    runtime.execute(TESTHARNESS_REPORTER).map(|_| ())
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
        let object = value.as_object().cloned().ok_or_else(|| {
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
            let entry = entry_value.as_object().cloned().ok_or_else(|| {
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
        let entry = value.as_object().cloned().ok_or_else(|| {
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
