//! Phase-3 chunk-1 timer integration tests.
//!
//! Exercises `setTimeout` / `clearTimeout` scheduling and the
//! `JsRuntime::pump_until_idle` callback driver. The companion
//! end-to-end test in `crates/koala-browser/tests/dom_bridge_tests.rs`
//! covers the same path through real `parse_html_string`.

use koala_js::JsRuntime;

mod common;
use common::list_fixture;

#[test]
fn set_timeout_schedules_a_callback_that_pump_fires() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = 0;\
             setTimeout(function() { globalThis.fired += 1; }, 0);",
        )
        .unwrap();
    // Without pump, the callback hasn't run yet.
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "0");
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "1");
}

#[test]
fn set_timeout_fires_in_chronological_order_regardless_of_call_order() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.log = [];\
             setTimeout(function() { globalThis.log.push('later'); }, 20);\
             setTimeout(function() { globalThis.log.push('sooner'); }, 0);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(
        rt.eval_to_string("globalThis.log.join(',')").unwrap(),
        "sooner,later",
    );
}

#[test]
fn clear_timeout_prevents_callback_firing() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = false;\
             var id = setTimeout(function() { globalThis.fired = true; }, 0);\
             clearTimeout(id);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "false");
}

#[test]
fn timer_callback_mutations_mark_runtime_dirty() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "setTimeout(function() {\
               document.body.appendChild(document.createElement('p'));\
             }, 0);",
        )
        .unwrap();
    // Before pump, no mutation has run yet.
    assert!(!rt.take_dom_dirty());
    rt.pump_until_idle().unwrap();
    assert!(rt.take_dom_dirty(), "callback's appendChild should mark dirty");
}
