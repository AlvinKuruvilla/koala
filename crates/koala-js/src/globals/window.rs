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
//! - `setTimeout` / `setInterval` (Phase 3 — event loop)
//! - Event handlers (`window.onload`, etc.)

use boa_engine::{Context, js_string, property::Attribute};

/// Register the `window` global on the context. Called from
/// [`crate::globals::register_globals`] after the document and
/// console have been registered, so `window.document` resolves
/// correctly from the moment `window` is queryable.
pub fn register_window(context: &mut Context) {
    let global = context.global_object();
    context
        .register_global_property(js_string!("window"), global, Attribute::all())
        .expect("`window` global should not already exist");
}
