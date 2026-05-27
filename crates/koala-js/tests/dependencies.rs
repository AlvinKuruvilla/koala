//! Phase-5 chunk-2 testharness.js dependency integration tests.
//!
//! Covers the four shims that testharness.js leans on:
//! `self === window`, `location.href` / `.search` / `.pathname`,
//! `setTimeout` trailing-args forwarding, and `'error'` event
//! dispatch on uncaught script errors via
//! [`JsRuntime::dispatch_error`].

use koala_js::JsRuntime;

mod common;
use common::list_fixture;

#[test]
fn self_is_an_alias_for_window() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("self === window").unwrap(),
        "true",
    );
    assert_eq!(
        rt.eval_to_string("self === globalThis").unwrap(),
        "true",
    );
    // And `self.document` resolves to the same global document
    // the rest of the bridge uses.
    assert_eq!(
        rt.eval_to_string("self.document === document").unwrap(),
        "true",
    );
}

#[test]
fn location_defaults_to_about_blank() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("location.href").unwrap(),
        "about:blank",
    );
    // Opaque schemes get empty path and search.
    assert_eq!(rt.eval_to_string("location.pathname").unwrap(), "");
    assert_eq!(rt.eval_to_string("location.search").unwrap(), "");
}

#[test]
fn set_location_updates_href_and_derived_components() {
    let mut rt = JsRuntime::new(list_fixture());
    rt.set_location("https://example.com/foo/bar?x=1&y=2#frag");
    assert_eq!(
        rt.eval_to_string("location.href").unwrap(),
        "https://example.com/foo/bar?x=1&y=2#frag",
    );
    assert_eq!(
        rt.eval_to_string("location.pathname").unwrap(),
        "/foo/bar",
    );
    assert_eq!(
        rt.eval_to_string("location.search").unwrap(),
        "?x=1&y=2",
    );
}

#[test]
fn location_to_string_returns_href() {
    let mut rt = JsRuntime::new(list_fixture());
    rt.set_location("https://example.com/page");
    assert_eq!(
        rt.eval_to_string("location.toString()").unwrap(),
        "https://example.com/page",
    );
    // String coercion via concat hits the same path.
    assert_eq!(
        rt.eval_to_string("'' + location").unwrap(),
        "https://example.com/page",
    );
}

#[test]
fn set_timeout_forwards_trailing_args_to_the_callback() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.args = null;\
             setTimeout(function(a, b, c) {\
               globalThis.args = [a, b, c];\
             }, 0, 'first', 42, true);",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(
        rt.eval_to_string("globalThis.args.join(',')").unwrap(),
        "first,42,true",
    );
}

#[test]
fn set_interval_re_forwards_the_same_args_every_firing() {
    // Each tick should see the same captured arg list.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.fired = 0;\
             globalThis.last = null;\
             var id = setInterval(function(tag) {\
               globalThis.fired += 1;\
               globalThis.last = tag;\
               if (globalThis.fired >= 3) { clearInterval(id); }\
             }, 5, 'persistent');",
        )
        .unwrap();
    rt.pump_until_idle().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.fired").unwrap(), "3");
    assert_eq!(
        rt.eval_to_string("globalThis.last").unwrap(),
        "persistent",
        "interval should re-forward the same captured arg every firing",
    );
}

#[test]
fn dispatch_error_fires_window_error_listeners_with_message() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.errors = [];\
             window.addEventListener('error', function(e) {\
               globalThis.errors.push(e.message);\
             });",
        )
        .unwrap();

    rt.dispatch_error("Uncaught TypeError: cannot read property 'x' of null")
        .unwrap();

    assert_eq!(
        rt.eval_to_string("globalThis.errors.length").unwrap(),
        "1",
    );
    assert_eq!(
        rt.eval_to_string("globalThis.errors[0]").unwrap(),
        "Uncaught TypeError: cannot read property 'x' of null",
    );
}

#[test]
fn dispatch_error_event_type_and_cancelable_flag() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.eventType = null;\
             globalThis.wasCancelable = null;\
             window.addEventListener('error', function(e) {\
               globalThis.eventType = e.type;\
               globalThis.wasCancelable = e.cancelable;\
             });",
        )
        .unwrap();
    rt.dispatch_error("boom").unwrap();
    assert_eq!(rt.eval_to_string("globalThis.eventType").unwrap(), "error");
    // Spec: the 'error' event is cancelable so handlers can
    // suppress the default action (logging in real browsers).
    assert_eq!(
        rt.eval_to_string("globalThis.wasCancelable").unwrap(),
        "true",
    );
}
