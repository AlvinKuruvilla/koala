//! Phase-5 chunk-1 testharness.js bridge integration tests.
//!
//! Exercises the JS-side capture functions
//! (`__koala_emit_result__`, `__koala_emit_completion__`) plus
//! the Rust-side drain accessors (`koala_wpt::take_test_results`,
//! `koala_wpt::take_test_completion`). Real testharness.js
//! integration is deferred to chunk 5.2; these tests simulate
//! the calls testharness.js would make.

use std::cell::RefCell;
use std::rc::Rc;

use koala_dom::DomTree;
use koala_js::{DomHandle, JsRuntime};

/// Tiny fixture: a freshly-allocated `JsRuntime` with the WPT
/// bridge installed. The DOM is the empty default tree —
/// nothing in this test file actually exercises the DOM bridge.
fn runtime() -> JsRuntime {
    let dom: DomHandle = Rc::new(RefCell::new(DomTree::new()));
    let mut rt = JsRuntime::new(dom);
    koala_wpt::install(&mut rt);
    rt
}

#[test]
fn emit_result_captures_a_single_test() {
    let mut rt = runtime();
    let _ = rt
        .execute(
            "__koala_emit_result__({\
               name: 'first test',\
               status: 0,\
               message: '',\
               stack: ''\
             });",
        )
        .unwrap();

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "first test");
    assert_eq!(results[0].status, 0);
    assert_eq!(results[0].message, "");
    assert_eq!(results[0].stack, "");
}

#[test]
fn emit_result_preserves_emit_order_across_multiple_calls() {
    let mut rt = runtime();
    let _ = rt
        .execute(
            "__koala_emit_result__({ name: 'a', status: 0 });\
             __koala_emit_result__({ name: 'b', status: 1, message: 'boom' });\
             __koala_emit_result__({ name: 'c', status: 2 });",
        )
        .unwrap();

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "c"]);
    let statuses: Vec<u32> = results.iter().map(|r| r.status).collect();
    assert_eq!(statuses, vec![0, 1, 2]);
    assert_eq!(results[1].message, "boom");
}

#[test]
fn take_test_results_drains_the_buffer() {
    // Spec contract: a second drain returns empty unless more
    // results have been emitted in between. Mirrors
    // `take_dom_dirty`'s sticky-bit semantics.
    let mut rt = runtime();
    let _ = rt
        .execute("__koala_emit_result__({ name: 'one', status: 0 });")
        .unwrap();
    assert_eq!(koala_wpt::take_test_results(&mut rt).unwrap().len(), 1);
    assert!(
        koala_wpt::take_test_results(&mut rt).unwrap().is_empty(),
        "second drain should return an empty Vec",
    );

    let _ = rt
        .execute("__koala_emit_result__({ name: 'two', status: 0 });")
        .unwrap();
    let again = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].name, "two");
}

#[test]
fn add_result_callback_fan_out_fires_for_every_emit() {
    // The fallback `add_result_callback` should let user code
    // observe results in addition to the Rust-side capture.
    let mut rt = runtime();
    let _ = rt
        .execute(
            "globalThis.seen = [];\
             add_result_callback(function(t) { globalThis.seen.push(t.name); });\
             __koala_emit_result__({ name: 'alpha', status: 0 });\
             __koala_emit_result__({ name: 'beta', status: 0 });",
        )
        .unwrap();

    assert_eq!(
        rt.eval_to_string("globalThis.seen.join(',')").unwrap(),
        "alpha,beta",
        "the registered callback should have been invoked for both emits",
    );
    // And Rust still captured the same results.
    assert_eq!(koala_wpt::take_test_results(&mut rt).unwrap().len(), 2);
}

#[test]
fn non_callable_listeners_passed_to_add_result_callback_are_silently_ignored() {
    let mut rt = runtime();
    // Passing `null` mirrors how some harness plumbing
    // initializes optional callbacks; we shouldn't blow up.
    let _ = rt
        .execute(
            "add_result_callback(null);\
             add_result_callback(undefined);\
             add_result_callback(42);\
             __koala_emit_result__({ name: 'ok', status: 0 });",
        )
        .unwrap();
    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(results.len(), 1, "capture path should still record the emit");
}

#[test]
fn emit_completion_captures_status_and_message() {
    let mut rt = runtime();
    let _ = rt
        .execute(
            "__koala_emit_completion__(\
               [],\
               { status: 1, message: 'harness aborted' }\
             );",
        )
        .unwrap();

    let completion = koala_wpt::take_test_completion(&mut rt).unwrap();
    assert!(completion.is_some());
    let completion = completion.unwrap();
    assert_eq!(completion.status, 1);
    assert_eq!(completion.message, "harness aborted");
}

#[test]
fn take_test_completion_drains_the_slot() {
    let mut rt = runtime();
    let _ = rt
        .execute("__koala_emit_completion__([], { status: 0 });")
        .unwrap();

    assert!(koala_wpt::take_test_completion(&mut rt).unwrap().is_some());
    assert!(
        koala_wpt::take_test_completion(&mut rt).unwrap().is_none(),
        "second drain should be None until the next emit_completion",
    );
}

#[test]
fn add_completion_callback_fan_out_fires_once() {
    let mut rt = runtime();
    let _ = rt
        .execute(
            "globalThis.completionStatus = null;\
             add_completion_callback(function(tests, status) {\
               globalThis.completionStatus = status.status;\
             });\
             __koala_emit_completion__([], { status: 2, message: 'timeout' });",
        )
        .unwrap();

    assert_eq!(
        rt.eval_to_string("globalThis.completionStatus").unwrap(),
        "2",
    );
    let completion = koala_wpt::take_test_completion(&mut rt).unwrap().unwrap();
    assert_eq!(completion.status, 2);
    assert_eq!(completion.message, "timeout");
}

#[test]
fn result_payload_missing_fields_default_cleanly() {
    // A half-formed test object — only `name` and `status`,
    // no message or stack — should still capture without
    // throwing.
    let mut rt = runtime();
    let _ = rt
        .execute("__koala_emit_result__({ name: 'sparse', status: 3 });")
        .unwrap();

    let results = koala_wpt::take_test_results(&mut rt).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "sparse");
    assert_eq!(results[0].status, 3);
    assert_eq!(results[0].message, "", "missing message should decode to empty string");
    assert_eq!(results[0].stack, "");
}
