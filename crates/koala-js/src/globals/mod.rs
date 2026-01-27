//! JavaScript global objects.
//!
//! This module registers built-in global objects like `console` that are
//! available to all JavaScript code.
//!
//! # Implemented
//!
//! - `console` - [Console Standard](https://console.spec.whatwg.org/)
//!
//! # Not Yet Implemented
//!
//! - `document` - [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//! - `window` - [§ 7.2 The Window object](https://html.spec.whatwg.org/multipage/window-object.html)
//! - `location` - [§ 7.7.1 The Location interface](https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-location-interface)
//! - `navigator` - [§ 8.8 The Navigator object](https://html.spec.whatwg.org/multipage/system-state.html#the-navigator-object)
//! - `setTimeout`/`setInterval` - [§ 8.6 Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)

mod console;
mod document;

use boa_engine::Context;

/// Register all global objects on the context.
///
/// [§ 8.1.6.1 Realms and their counterparts](https://html.spec.whatwg.org/multipage/webappapis.html#realms-settings-objects-global-objects)
///
/// "A global object is a JavaScript object that is the global object for
/// a JavaScript realm."
///
/// This should be called once when creating a new `JsRuntime`.
pub fn register_globals(context: &mut Context) {
    console::register_console(context);

    // TODO: Register remaining globals when implemented
    //
    // STEP 1: document - requires DomHandle parameter
    // document::register_document(context, dom_handle);
    //
    // STEP 2: window - self-referential global object
    // window::register_window(context);
    //
    // STEP 3: location - requires URL state
    // location::register_location(context, url);
    //
    // STEP 4: navigator - browser metadata
    // navigator::register_navigator(context);
    //
    // STEP 5: Timers - requires event loop integration
    // timers::register_timers(context, event_loop);
}
