//! Phase-3 chunk-1 + chunk-2 timer integration tests.
//!
//! Exercises `setTimeout` / `setInterval` and their cancel
//! counterparts against the `JsRuntime::pump_until_idle` callback
//! driver. The companion end-to-end test in
//! `crates/koala-browser/tests/dom_bridge_tests.rs` covers the
//! same path through real `parse_html_string`.

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

#[test]
fn set_interval_fires_repeatedly_until_cleared() {
    // The callback clears itself once it has fired three times.
    // Without `clearInterval` the pump would tick forever (up to
    // the 30s budget) — the test relies on the callback's self-
    // cancellation to terminate the pump.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = 0;\
             var id = setInterval(function() {\
               globalThis.fired += 1;\
               if (globalThis.fired >= 3) { clearInterval(id); }\
             }, 5);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(
        rt.eval_to_string("globalThis.fired").unwrap(),
        "3",
        "interval should have fired exactly the three times before self-cancelling",
    );
}

#[test]
fn clear_interval_outside_callback_stops_future_firings() {
    // Pre-cancelling the interval before pump runs at all means
    // it never fires. Mirrors the corresponding setTimeout test.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = 0;\
             var id = setInterval(function() { globalThis.fired += 1; }, 0);\
             clearInterval(id);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "0");
}

#[test]
fn clear_timeout_can_cancel_an_interval_id() {
    // Per spec, setTimeout / setInterval share an id pool: passing
    // an interval id to clearTimeout (or a timeout id to
    // clearInterval) is a supported operation.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = 0;\
             var id = setInterval(function() { globalThis.fired += 1; }, 0);\
             clearTimeout(id);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "0");
}

#[test]
fn clear_interval_can_cancel_a_timeout_id() {
    // The complementary direction of the shared id pool: a
    // setTimeout id can be passed to clearInterval.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = false;\
             var id = setTimeout(function() { globalThis.fired = true; }, 0);\
             clearInterval(id);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "false");
}
