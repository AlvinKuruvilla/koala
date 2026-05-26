//! Phase-3 chunk-3 EventTarget integration tests.
//!
//! Covers `addEventListener` / `removeEventListener` /
//! `dispatchEvent` on `window`, `document`, and `Element`, the
//! `Event` constructor (`new Event(type, options)`), the
//! `preventDefault` + `stopImmediatePropagation` flow, and the
//! lifecycle dispatches (`DOMContentLoaded`, `load`) wired
//! through `JsRuntime::dispatch_dom_content_loaded` /
//! `dispatch_load`.

use koala_js::JsRuntime;

mod common;
use common::list_fixture;

#[test]
fn window_add_event_listener_fires_on_dispatch_event() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.calls = 0;\
             window.addEventListener('ping', function(e) {\
               globalThis.calls += 1;\
               globalThis.evType = e.type;\
             });\
             window.dispatchEvent(new Event('ping'));",
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("globalThis.calls").unwrap(), "1");
    assert_eq!(rt.eval_to_string("globalThis.evType").unwrap(), "ping");
}

#[test]
fn document_add_event_listener_fires_on_dispatch_event() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.calls = 0;\
             document.addEventListener('hello', function() {\
               globalThis.calls += 1;\
             });\
             document.dispatchEvent(new Event('hello'));",
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("globalThis.calls").unwrap(), "1");
}

#[test]
fn element_add_event_listener_fires_on_dispatch_event() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "var li = document.getElementById('a');\
             globalThis.target = null;\
             li.addEventListener('tap', function(e) {\
               globalThis.target = e.target;\
             });\
             li.dispatchEvent(new Event('tap'));",
        )
        .unwrap();
    // The fired event's `target` is the same JsElement the
    // listener was registered through.
    assert_eq!(
        rt.eval_to_string("globalThis.target.id").unwrap(),
        "a",
    );
}

#[test]
fn element_listeners_survive_re_querying_the_same_node() {
    // JsElement wrappers are created fresh on every
    // `getElementById`, but listener storage is keyed by NodeId,
    // not by wrapper identity. A second wrapper for the same node
    // sees the same listener list.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.calls = 0;\
             document.getElementById('a').addEventListener('tap', function() {\
               globalThis.calls += 1;\
             });\
             document.getElementById('a').dispatchEvent(new Event('tap'));",
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("globalThis.calls").unwrap(), "1");
}

#[test]
fn remove_event_listener_stops_future_dispatch() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.calls = 0;\
             var cb = function() { globalThis.calls += 1; };\
             window.addEventListener('ping', cb);\
             window.dispatchEvent(new Event('ping'));\
             window.removeEventListener('ping', cb);\
             window.dispatchEvent(new Event('ping'));",
        )
        .unwrap();
    assert_eq!(
        rt.eval_to_string("globalThis.calls").unwrap(),
        "1",
        "the second dispatch should miss the now-removed listener",
    );
}

#[test]
fn dispatch_event_returns_false_when_default_prevented() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "window.addEventListener('cancellable', function(e) {\
               e.preventDefault();\
             });\
             globalThis.result = window.dispatchEvent(\
               new Event('cancellable', { cancelable: true })\
             );",
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("globalThis.result").unwrap(), "false");
}

#[test]
fn prevent_default_is_a_noop_on_non_cancellable_events() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "window.addEventListener('noisy', function(e) {\
               e.preventDefault();\
             });\
             globalThis.result = window.dispatchEvent(new Event('noisy'));",
        )
        .unwrap();
    // dispatchEvent returns !defaultPrevented; on a
    // non-cancellable event preventDefault leaves
    // defaultPrevented untouched, so dispatch returns true.
    assert_eq!(rt.eval_to_string("globalThis.result").unwrap(), "true");
}

#[test]
fn stop_immediate_propagation_skips_remaining_listeners() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.log = [];\
             window.addEventListener('chain', function(e) {\
               globalThis.log.push('first');\
               e.stopImmediatePropagation();\
             });\
             window.addEventListener('chain', function() {\
               globalThis.log.push('second');\
             });\
             window.dispatchEvent(new Event('chain'));",
        )
        .unwrap();
    assert_eq!(
        rt.eval_to_string("globalThis.log.join(',')").unwrap(),
        "first",
        "stopImmediatePropagation should fire the first listener and skip the second",
    );
}

#[test]
fn dispatch_dom_content_loaded_fires_document_listeners() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.ready = false;\
             document.addEventListener('DOMContentLoaded', function() {\
               globalThis.ready = true;\
             });",
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("globalThis.ready").unwrap(), "false");
    rt.dispatch_dom_content_loaded().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.ready").unwrap(), "true");
}

#[test]
fn dispatch_load_fires_window_listeners() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "globalThis.loaded = false;\
             window.addEventListener('load', function() {\
               globalThis.loaded = true;\
             });",
        )
        .unwrap();
    rt.dispatch_load().unwrap();
    assert_eq!(rt.eval_to_string("globalThis.loaded").unwrap(), "true");
}

#[test]
fn lifecycle_listener_mutations_mark_dom_dirty() {
    // Browser pipeline depends on `take_dom_dirty()` correctly
    // observing mutations performed inside lifecycle listeners,
    // so the style cascade re-runs against the post-load tree.
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "document.addEventListener('DOMContentLoaded', function() {\
               document.body.appendChild(document.createElement('p'));\
             });",
        )
        .unwrap();
    assert!(!rt.take_dom_dirty());
    rt.dispatch_dom_content_loaded().unwrap();
    assert!(
        rt.take_dom_dirty(),
        "appendChild from a DOMContentLoaded listener should mark dirty",
    );
}
