//! JavaScript global objects.
//!
//! This module registers built-in global objects like `console`
//! and `document` that are available to all JavaScript code.
//!
//! # Implemented
//!
//! - `console` — [Console Standard](https://console.spec.whatwg.org/)
//! - `document` — [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//!   (Phase-2 read-only subset; see [`document`] for the method
//!   list)
//!
//! - `EventTarget` mixin on `window`, `document`, and `Element` —
//!   [§ 2.6 Interface EventTarget](https://dom.spec.whatwg.org/#interface-eventtarget)
//!   — plus a minimal `Event` constructor. Dispatch is
//!   strict-target-only for now; see [`events`] for the
//!   deferred-bubbling note.
//!
//! # Not Yet Implemented
//!
//! - `location` — [§ 7.7.1 The Location interface](https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-location-interface)
//! - `navigator` — [§ 8.8 The Navigator object](https://html.spec.whatwg.org/multipage/system-state.html#the-navigator-object)
//! - Event-handler IDL attributes (`window.onload = fn`,
//!   `document.onreadystatechange`, …)
//! - Event bubbling / capture phases (chunk-3 follow-up)

// `macros` is declared first so its `dom_interface!` macro is in
// scope for every sibling module without each needing its own
// `use` import.
#[macro_use]
pub(crate) mod macros;

mod console;
pub(crate) mod document;
pub(crate) mod element;
pub(crate) mod element_class;
pub(crate) mod event_target_class;
pub(crate) mod events;
pub(crate) mod helpers;
pub(crate) mod location;
pub(crate) mod node_class;
mod selectors;
mod text;
pub(crate) mod timers;
pub(crate) mod window;

use boa_engine::Context;

/// Register all global objects on the context.
///
/// [§ 8.1.6.1 Realms and their counterparts](https://html.spec.whatwg.org/multipage/webappapis.html#realms-settings-objects-global-objects)
///
/// "A global object is a JavaScript object that is the global
/// object for a JavaScript realm."
///
/// Called once when constructing a [`crate::JsRuntime`]. The DOM
/// handle the runtime holds is installed as the thread's current
/// tree at execute time via [`crate::dom_handle::guard`] — these
/// registration calls don't need to thread it explicitly.
///
/// Order matters: `window` is a self-pointer to the global object,
/// so it picks up `document` and `console` as properties only after
/// they've been registered.
pub fn register_globals(context: &mut Context) {
    // DOM interface chain, built bottom-up so each child can
    // read its parent's prototype off the global object when
    // setting its own `[[Prototype]]`. Every interface here
    // goes through the `dom_interface!` macro and the
    // `boa_engine::class::Class` trait — uniform shape, no
    // hand-rolled stubs.
    event_target_class::register_event_target_class(context);
    node_class::register_node_class(context);
    element_class::register_element_class(context);
    element_class::register_html_element_class(context);

    console::register_console(context);
    document::register_document(context);
    timers::register_timers(context);
    events::register_events(context);
    location::register_location(context);
    window::register_event_target(context);
    window::register_window(context);

    // Not yet implemented:
    // - navigator (Phase 2 follow-up: browser metadata)
    // - Event bubbling / capture (chunk 3 follow-up; today's
    //   dispatch is strict-target-only)
}
