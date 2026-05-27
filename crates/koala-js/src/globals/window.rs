//! `window` global.
//!
//! [§ 7.2 The Window object](https://html.spec.whatwg.org/multipage/window-object.html)
//!
//! > "The Window object provides the global scope for the
//! > navigable. It's the JavaScript runtime entry point to the
//! > rest of the browser environment."
//!
//! In a browser, the Window IS the global object — `window === globalThis`,
//! and every global (e.g. `document`, `console`) is also a property of
//! `window`. We model this directly: a `window` global property that
//! points back at Boa's global object. Then:
//!
//!   - `typeof window === "object"`
//!   - `window === window.window` (the global has a `window` property
//!     pointing to itself)
//!   - `window.document === document`
//!   - `window.console === console`
//!
//! comes "for free" because `window`'s property lookups go through the
//! same global where everything else lives.
//!
//! # Not yet implemented
//!
//! - `window.location`, `window.navigator`, `window.history`, …
//! - Event-handler IDL attributes (`window.onload`, …) — today
//!   listeners are registered via `addEventListener`, not by
//!   assigning to `on*` properties.

use boa_engine::{
    Context, JsResult, JsValue, NativeFunction, js_string, property::Attribute,
};

use super::events::{
    add_listener_at_scope, dispatch_event_call, remove_listener_at_scope,
};

/// Scope key used by `events::dispatch_at_scope` for listeners
/// attached at the window level. Public so the runtime's
/// lifecycle-event dispatcher in `crate::lib` can reuse it
/// without duplicating the literal.
pub(crate) const WINDOW_SCOPE: &str = "window";

/// Register the `window` global on the context. Called from
/// [`crate::globals::register_globals`] after the document and
/// console have been registered, so `window.document` resolves
/// correctly from the moment `window` is queryable.
///
/// Also registers `self` as a second pointer to the same global
/// object. testharness.js (and a swath of code that came up
/// through Web Workers, where `self` is the canonical name) leans
/// on `self === window`, so the alias has to exist even though
/// koala doesn't model a Worker context yet.
///
/// # Top-level browsing-context self-references
///
/// [§ 7.2.5 The Window object — the WindowProxy exotic object](https://html.spec.whatwg.org/multipage/window-object.html#the-windowproxy-exotic-object)
///
/// The HTML spec defines `parent`, `top`, and `opener` getters on
/// `Window`:
///
/// > "The parent getter steps are to … return [the] parent navigable's
/// > active WindowProxy [object], if [a parent navigable exists];
/// > otherwise, this's WindowProxy."
/// >
/// > "The top getter steps are to … return [the] top-level traversable's
/// > active WindowProxy [object]."
/// >
/// > "The opener getter steps are to … return [the] opener browsing
/// > context's WindowProxy object, if [one exists]; otherwise, null."
///
/// Koala models a single top-level document with no parent, no nested
/// browsing contexts, and no opener. The spec's degenerate cases for
/// that situation resolve to:
///
/// - `window.parent === window` (no parent navigable)
/// - `window.top   === window` (this *is* the top)
/// - `window.opener === null`   (no opener)
///
/// In particular, testharness.js's `_forEach_windows` walks
/// `self → self.parent → self.parent.parent …` until it finds a
/// fixed point; without `parent` resolving to a Window the loop
/// dereferences `.parent` on `undefined` and throws a `TypeError`
/// before any `test(...)` body runs.
pub fn register_window(context: &mut Context) {
    let global = context.global_object();
    context
        .register_global_property(js_string!("window"), global.clone(), Attribute::all())
        .expect("`window` global should not already exist");
    context
        .register_global_property(js_string!("self"), global.clone(), Attribute::all())
        .expect("`self` global should not already exist");
    context
        .register_global_property(js_string!("parent"), global.clone(), Attribute::all())
        .expect("`parent` global should not already exist");
    context
        .register_global_property(js_string!("top"), global, Attribute::all())
        .expect("`top` global should not already exist");
    context
        .register_global_property(js_string!("opener"), JsValue::null(), Attribute::all())
        .expect("`opener` global should not already exist");
}

/// Register `addEventListener` / `removeEventListener` /
/// `dispatchEvent` on the global object. Because `window === globalThis`
/// for koala, registering these as plain globals makes them
/// reachable both as bare names (`addEventListener(...)`) and as
/// methods on `window` (`window.addEventListener(...)`).
///
/// Called from [`crate::globals::register_globals`] *before*
/// `register_window` so the methods are visible on the global
/// before any user script runs.
pub fn register_event_target(context: &mut Context) {
    context
        .register_global_callable(
            js_string!("addEventListener"),
            2,
            NativeFunction::from_copy_closure(window_add_event_listener),
        )
        .expect("window.addEventListener should not already be registered");
    context
        .register_global_callable(
            js_string!("removeEventListener"),
            2,
            NativeFunction::from_copy_closure(window_remove_event_listener),
        )
        .expect("window.removeEventListener should not already be registered");
    context
        .register_global_callable(
            js_string!("dispatchEvent"),
            1,
            NativeFunction::from_copy_closure(window_dispatch_event),
        )
        .expect("window.dispatchEvent should not already be registered");
}

fn window_add_event_listener(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let type_ = args.first().cloned().unwrap_or(JsValue::undefined());
    let listener = args.get(1).cloned().unwrap_or(JsValue::undefined());
    add_listener_at_scope(WINDOW_SCOPE, &type_, &listener, context)?;
    Ok(JsValue::undefined())
}

fn window_remove_event_listener(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let type_ = args.first().cloned().unwrap_or(JsValue::undefined());
    let listener = args.get(1).cloned().unwrap_or(JsValue::undefined());
    remove_listener_at_scope(WINDOW_SCOPE, &type_, &listener, context)?;
    Ok(JsValue::undefined())
}

fn window_dispatch_event(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    dispatch_event_call(WINDOW_SCOPE, this, args, context)
}
