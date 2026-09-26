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

// Virtual time: the pump skips the page clock to the next timer instead
// of sleeping (`koala_js::clock`). These tests would each take seconds
// or minutes of wall time if it slept.

/// A timer far in the future still fires, without the test waiting for
/// it, and `Date.now()` inside the callback shows the delay elapsed.
#[test]
fn pump_skips_to_future_timer_and_date_now_follows() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.start = Date.now();\
             globalThis.waited = -1;\
             setTimeout(function() { globalThis.waited = Date.now() - globalThis.start; }, 5000);",
        )
        .unwrap();
    let wall = std::time::Instant::now();
    rt.pump_until_idle().unwrap();
    assert!(
        wall.elapsed() < std::time::Duration::from_secs(1),
        "pump slept instead of skipping: {:?}",
        wall.elapsed()
    );
    let waited: f64 = rt.eval_to_string("globalThis.waited").unwrap().parse().unwrap();
    assert!(waited >= 5000.0, "Date.now() advanced only {waited} ms across a 5000 ms timer");
}

/// Timers due within the budget run; one due after it does not.
#[test]
fn pump_stops_at_the_timer_budget() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.log = [];\
             setTimeout(function() { globalThis.log.push('9s'); }, 9000);\
             setTimeout(function() { globalThis.log.push('11s'); }, 11000);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.log.join(',')").unwrap(), "9s");
}

/// An interval that is never cleared ends at the budget: 10 s at 1 s per
/// firing is 10 firings, give or take one for where the pump started.
#[test]
fn endless_interval_ends_at_the_budget() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.ticks = 0;\
             setInterval(function() { globalThis.ticks += 1; }, 1000);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    let ticks: u32 = rt.eval_to_string("globalThis.ticks").unwrap().parse().unwrap();
    assert!((9..=10).contains(&ticks), "expected ~10 ticks, got {ticks}");
}

/// A zero-delay chain is always due, so the budget never expires; the
/// task cap stops it.
#[test]
fn zero_delay_chain_stops_at_the_task_cap() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.runs = 0;\
             function again() { globalThis.runs += 1; setTimeout(again, 0); }\
             setTimeout(again, 0);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.runs").unwrap(), "10000");
}
