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
//! # Not Yet Implemented
//!
//! - `window` — [§ 7.2 The Window object](https://html.spec.whatwg.org/multipage/window-object.html)
//! - `location` — [§ 7.7.1 The Location interface](https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-location-interface)
//! - `navigator` — [§ 8.8 The Navigator object](https://html.spec.whatwg.org/multipage/system-state.html#the-navigator-object)
//! - `setTimeout`/`setInterval` — [§ 8.6 Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)

mod console;
mod document;

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
/// tree at execute time via
/// [`crate::dom_handle::guard`] — these registration calls don't
/// need to thread it explicitly.
pub fn register_globals(context: &mut Context) {
    console::register_console(context);
    document::register_document(context);

    // Not yet implemented:
    // - window  (Phase 2 follow-up: self-referential global object)
    // - location (Phase 2 follow-up: requires URL state)
    // - navigator (Phase 2 follow-up: browser metadata)
    // - timers (Phase 3: requires event-loop integration)
}
