//! Tests for `koala_wpt::attach_reporter`.
//!
//! These exercise the reporter against a JS-side mock that
//! mimics the relevant slice of testharness.js: a global
//! `add_result_callback` / `add_completion_callback` that
//! append to a callback list without replaying historical
//! results, plus a `tests` object with a `tests` array that the
//! reporter walks during replay.
//!
//! No real testharness.js is loaded — that's intentional. The
//! reporter's contract is "bind through whatever
//! `add_result_callback` exists right now, and replay anything
//! sitting on `tests.tests`." That contract is testable without
//! the upstream file.

use std::cell::RefCell;
use std::rc::Rc;

use koala_dom::DomTree;
use koala_js::{DomHandle, JsRuntime};

/// Common setup: fresh runtime with the koala-wpt bridge
/// installed, plus a JS mock of testharness.js's
/// `add_result_callback` / `add_completion_callback` / `tests`
/// surface installed on top.
///
/// The mock deliberately matches testharness.js's NO-replay
/// semantics for `add_result_callback`: a callback registered
/// after a test has completed does not retroactively fire. The
/// reporter is expected to compensate via the `tests.tests`
/// walk.
fn runtime_with_mock() -> JsRuntime {
    let dom: DomHandle = Rc::new(RefCell::new(DomTree::new()));
    let mut rt = JsRuntime::new(dom);
    koala_wpt::install(&mut rt);
    let _ = rt
        .execute(
            r#"
            // Mock the slice of testharness.js the reporter touches.
            // Note: `add_result_callback` and `add_completion_callback`
            // here OVERWRITE the koala-wpt fallbacks, mirroring what
            // real testharness.js does when it loads.
            globalThis.__mockResultCallbacks = [];
            globalThis.__mockCompletionCallbacks = [];
            add_result_callback = function (cb) {
                __mockResultCallbacks.push(cb);
            };
            add_completion_callback = function (cb) {
                __mockCompletionCallbacks.push(cb);
            };
            globalThis.tests = { tests: [] };
            // Test.prototype.phases mirrors the upstream enum so the
            // reporter's COMPLETE-phase detection works.
            globalThis.Test = function () {};
            Test.prototype.phases = {
                INITIAL: 0,
                STARTED: 1,
                HAS_RESULT: 2,
                CLEANING: 3,
                COMPLETE: 4,
            };
            // Mock `test()`: complete-and-fire the callbacks list at
            // the time of the call. Tests that complete BEFORE the
            // reporter registers will fire into an empty callback
            // list — the replay path has to catch them.
            globalThis.test = function (fn, name) {
                var t = {
                    name: name,
                    status: 0,
                    message: '',
                    stack: '',
                    phase: Test.prototype.phases.COMPLETE,
                };
                try {
                    fn();
                } catch (e) {
                    t.status = 1;
                    t.message = String(e && e.message || e);
                }
                tests.tests.push(t);
                for (var i = 0; i < __mockResultCallbacks.length; i++) {
                    __mockResultCallbacks[i](t);
                }
            };
            // Mock `done()`: invoke completion callbacks.
            globalThis.done = function () {
                var status = { status: 0, message: '' };
                for (var i = 0; i < __mockCompletionCallbacks.length; i++) {
                    __mockCompletionCallbacks[i](tests.tests, status);
                }
            };
            "#,
        )
        .unwrap();
    rt
}

#[test]
fn attach_reporter_captures_async_test_after_binding() {
    // Test that runs AFTER attach_reporter — the standard
    // "register, then run" path. This is what async tests look
    // like once they pump through.
    let mut rt = runtime_with_mock();
    koala_wpt::attach_reporter(&mut rt).unwrap();
    let _ = rt
        .execute("test(function () {}, 'after binding');")
        .unwrap();

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "after binding");
    assert_eq!(results[0].status, 0);
}

#[test]
fn attach_reporter_replays_sync_tests_that_completed_before_binding() {
    // The motivating case: a sync `test()` completes before the
    // reporter binds. testharness.js's `add_result_callback`
    // doesn't replay, so the reporter has to walk `tests.tests`
    // and emit completed entries directly.
    let mut rt = runtime_with_mock();
    let _ = rt
        .execute(
            "test(function () {}, 'sync pass');\
             test(function () { throw new Error('boom') }, 'sync fail');",
        )
        .unwrap();
    // Right after `test()`, no callbacks have run because the
    // reporter isn't bound yet.
    let pre_attach = koala_wpt::take_test_results(&mut rt).unwrap();
    assert!(
        pre_attach.is_empty(),
        "no results should be captured before attach_reporter runs",
    );

    koala_wpt::attach_reporter(&mut rt).unwrap();

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(results.len(), 2, "both sync tests should be replayed");
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["sync pass", "sync fail"]);
    assert_eq!(results[0].status, 0);
    assert_eq!(results[1].status, 1);
    assert_eq!(results[1].message, "boom");
}

#[test]
fn attach_reporter_captures_completion_callback() {
    // Completion callbacks register normally — the reporter
    // binds, then a later `done()` fires the binding.
    let mut rt = runtime_with_mock();
    koala_wpt::attach_reporter(&mut rt).unwrap();
    let _ = rt.execute("done();").unwrap();

    let completion = koala_wpt::take_test_completion(&mut rt).unwrap();
    assert!(completion.is_some());
    let completion = completion.unwrap();
    assert_eq!(completion.status, 0);
}

#[test]
fn attach_reporter_handles_documents_without_testharness() {
    // When testharness.js never loaded, neither
    // `add_result_callback` nor `tests` exists in the form the
    // reporter expects. The reporter must no-op rather than
    // throwing. Use a runtime with just the koala-wpt bridge
    // and no mock — so add_result_callback is koala-wpt's
    // fallback (still a function), tests is undefined.
    let dom: DomHandle = Rc::new(RefCell::new(DomTree::new()));
    let mut rt = JsRuntime::new(dom);
    koala_wpt::install(&mut rt);
    koala_wpt::attach_reporter(&mut rt).expect("must not throw on plain documents");

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert!(results.is_empty());
    let completion = koala_wpt::take_test_completion(&mut rt).unwrap();
    assert!(completion.is_none());
}

#[test]
fn replay_path_is_idempotent_when_already_observed_test_repeats() {
    // Sanity check: if attach_reporter is somehow called twice
    // (a misconfiguration), the replay walks `tests.tests` each
    // time and emits duplicate results. We DON'T claim
    // idempotency here — the test exists to lock down the
    // current behaviour so a future change that introduces
    // dedup is a conscious decision.
    let mut rt = runtime_with_mock();
    let _ = rt.execute("test(function () {}, 'once');").unwrap();
    koala_wpt::attach_reporter(&mut rt).unwrap();
    koala_wpt::attach_reporter(&mut rt).unwrap();

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(
        results.len(),
        2,
        "second attach_reporter currently replays again (no dedup); \
         change this assertion if you intentionally add dedup",
    );
}
