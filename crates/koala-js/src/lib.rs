//! JavaScript engine integration for the Koala renderer.
//!
//! Uses [Boa](https://boajs.dev/) as the JavaScript engine.
//!
//! # Example
//!
//! ```ignore
//! use std::cell::RefCell;
//! use std::rc::Rc;
//! use koala_dom::DomTree;
//! use koala_js::JsRuntime;
//!
//! let dom = Rc::new(RefCell::new(DomTree::new()));
//! let mut runtime = JsRuntime::new(dom);
//! runtime.execute("console.log('Hello from JS!');").unwrap();
//! ```
//!
//! # Implemented
//!
//! - Script execution via `JsRuntime::execute()` /
//!   `JsRuntime::eval_to_string()`
//! - Event loop pump via `JsRuntime::pump_until_idle()`
//! - `console.log()`, `console.warn()`, `console.error()`
//! - DOM bridge (Phase 2 complete):
//!   - `document.getElementById`, `querySelector`,
//!     `querySelectorAll`, `getElementsByTagName`,
//!     `getElementsByClassName`, `createElement`, `createTextNode`
//!   - `document.body`, `document.head`, `document.documentElement`,
//!     `document.title`
//!   - `Element.tagName`, `id`, `className`, `nodeType`
//!   - `Element.getAttribute`, `hasAttribute`, `setAttribute`,
//!     `removeAttribute`
//!   - `Element.parentElement`, `children`, `firstElementChild`,
//!     `lastElementChild`, `nextElementSibling`,
//!     `previousElementSibling`
//!   - `Element.textContent` (read + write)
//!   - `Element.appendChild`, `removeChild`, `querySelector`,
//!     `querySelectorAll`
//!   - `window` (self-referential global)
//! - Timers (Phase 3 chunks 1 and 2):
//!   - `setTimeout`, `clearTimeout`
//!   - `setInterval`, `clearInterval` (shared id pool with the
//!     timeout variants)
//! - EventTarget (Phase 3 chunk 3):
//!   - `addEventListener` / `removeEventListener` /
//!     `dispatchEvent` on `window`, `document`, and `Element`
//!   - `new Event(type, { bubbles, cancelable })`,
//!     `preventDefault`, `stopImmediatePropagation`
//!   - Lifecycle events `DOMContentLoaded` and `load` fired from
//!     [`JsRuntime::dispatch_dom_content_loaded`] /
//!     [`JsRuntime::dispatch_load`]
//! - DOM mutations trigger a re-cascade + re-layout in
//!   koala-browser after scripts return.
//!
//! # Not Yet Implemented
//!
//! - Event bubbling / capture phases — dispatch is currently
//!   strict-target-only. `bubbles: true` is honoured on
//!   `Event` instances but does not yet walk parent chains.
//! - Event-handler IDL attributes (`window.onload = fn`,
//!   `document.onreadystatechange`, …)
//! - External scripts (`<script src="…">`), `async` / `defer`,
//!   module scripts (Phase 4)
//! - `Element.innerHTML` (write), Text-node accessors
//!   (`data`, `nodeValue`), `Node.firstChild` /  `nextSibling`
//!   (need Text/Comment wrappers)

mod dom_handle;
mod globals;
mod scheduler;

pub use dom_handle::DomHandle;

use std::cell::Cell;
use std::time::{Duration, Instant};

use boa_engine::{Context, JsError, JsString, JsValue, Source, js_string};

/// JavaScript runtime for a document.
///
/// [§ 8.1.6 JavaScript execution context](https://html.spec.whatwg.org/multipage/webappapis.html)
///
/// Each document has its own JavaScript runtime with its own global
/// object and its own Boa [`Context`]. The runtime is created when
/// the document is loaded and destroyed when the document is
/// unloaded.
///
/// The runtime holds a [`DomHandle`] (a shared `Rc<RefCell<DomTree>>`).
/// Every call to [`execute`](Self::execute) installs that handle as
/// the thread's current DOM (via [`dom_handle::guard`]) before
/// evaluating, so the DOM-bridge globals see the right tree.
pub struct JsRuntime {
    /// The Boa JavaScript context.
    context: Context,
    /// Shared handle to the document's DOM tree. Made available to
    /// JS-callable closures via [`dom_handle::guard`] for the
    /// duration of each [`execute`](Self::execute) call.
    dom: DomHandle,
    /// Sticky bit set whenever any [`execute`](Self::execute) call's
    /// DOM-mutation closures flipped the per-thread dirty flag.
    /// Cleared by [`take_dom_dirty`](Self::take_dom_dirty).
    dom_dirty: Cell<bool>,
    /// Installs the timer scheduler in the per-thread slot for the
    /// life of this runtime. Held purely for its `Drop` side effect;
    /// `execute` and `pump_until_idle` read the same thread-local.
    /// Declared after `context` so it drops AFTER the context's
    /// GC sweep — any callback fired during shutdown still sees
    /// a live scheduler.
    #[allow(dead_code)] // RAII only; the compiler can't see Drop as a "read"
    scheduler_guard: scheduler::SchedulerGuard,
}

impl JsRuntime {
    /// Create a new JavaScript runtime bound to `dom`.
    ///
    /// Registers the built-in globals (`console`, `document`) on
    /// the new Boa context. The DOM handle is held for the lifetime
    /// of the runtime and re-installed as the thread-current DOM
    /// on every call to [`execute`](Self::execute).
    #[must_use]
    pub fn new(dom: DomHandle) -> Self {
        // Install the per-thread scheduler BEFORE registering
        // globals — `register_globals` calls `register_timers`,
        // which doesn't directly query the scheduler but downstream
        // `setTimeout` calls will. Installing here means the same
        // scheduler instance handles every script + pump cycle for
        // this runtime.
        let scheduler_guard = scheduler::guard();
        let mut context = Context::default();
        globals::register_globals(&mut context);
        Self {
            context,
            dom,
            dom_dirty: Cell::new(false),
            scheduler_guard,
        }
    }

    /// Execute JavaScript source code against the runtime's DOM.
    ///
    /// [§ 4.12.1.1 Processing model](https://html.spec.whatwg.org/multipage/scripting.html#script-processing-model)
    ///
    /// Installs the runtime's [`DomHandle`] as the thread's current
    /// DOM via a [`DomGuard`](crate::dom_handle::DomGuard) before
    /// the evaluation, and restores the previous binding on return.
    /// All DOM-bridge globals read the tree through that
    /// thread-local.
    ///
    /// # Arguments
    ///
    /// * `source` - The JavaScript source code to execute.
    ///
    /// # Returns
    ///
    /// The result of evaluating the script, or a [`JsError`] if
    /// execution failed.
    ///
    /// # Errors
    ///
    /// Returns [`JsError`] if the JavaScript code contains syntax
    /// errors or throws an uncaught exception.
    pub fn execute(&mut self, source: &str) -> Result<JsValue, JsError> {
        let dom_guard = dom_handle::guard(self.dom.clone());
        let result = self.context.eval(Source::from_bytes(source));
        if dom_guard.dirty_seen() {
            self.dom_dirty.set(true);
        }
        drop(dom_guard);
        result
    }

    /// Evaluate `source` and coerce the result to a Rust `String`.
    ///
    /// Convenience that combines [`execute`](Self::execute) with
    /// Boa's `toString()` conversion. Intended for tests and host
    /// code that just wants the textual value of an expression
    /// without juggling `JsValue` / `JsString`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`execute`](Self::execute), plus
    /// any error raised by the JS-side `toString()` (e.g. a
    /// `Symbol` value).
    pub fn eval_to_string(&mut self, source: &str) -> Result<String, JsError> {
        let value = self.execute(source)?;
        Ok(value.to_string(&mut self.context)?.to_std_string_escaped())
    }

    /// Run pending timer callbacks until the queue is empty.
    ///
    /// [§ 8.1.6.3 Processing model](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model)
    ///
    /// Simplified relative to spec: just "while there's anything
    /// pending, sleep until it's due, call it, loop." No microtask
    /// queue, no rendering between iterations — those land later
    /// when there's an actual event loop coordinated with the
    /// browser pipeline. This is sufficient for `setTimeout`-based
    /// async tests where the callback runs in isolation.
    ///
    /// Reads callbacks back out of the hidden `__koala_timers__`
    /// global array by `TimerId`; the array is rooted by the
    /// global object so Boa's GC keeps the callbacks alive across
    /// every iteration. For one-shot timers (`setTimeout`) the
    /// slot is cleared to `null` after firing so a long-running
    /// pump doesn't leak callback closures. For interval timers
    /// (`setInterval`) the slot is left in place because the same
    /// id is re-armed for the next firing via
    /// [`scheduler::schedule`].
    ///
    /// Mutations performed inside fired callbacks accumulate into
    /// `dom_dirty` the same way they do for `execute` calls.
    ///
    /// # Errors
    ///
    /// Returns the first `JsError` thrown by a callback, abandoning
    /// any remaining work. Subsequent calls to `pump_until_idle`
    /// continue with the rest of the queue.
    pub fn pump_until_idle(&mut self) -> Result<(), JsError> {
        // Belt-and-braces budget so a broken setTimeout(fn, 0) →
        // setTimeout(fn, 0) loop can't hang the parse path
        // indefinitely. Honoured wptrunner timeouts will trip
        // first in practice.
        let budget = Duration::from_secs(30);
        let started = Instant::now();

        loop {
            if started.elapsed() > budget {
                break;
            }

            let dom_guard = dom_handle::guard(self.dom.clone());

            let due_ids = scheduler::pop_due_now();
            if due_ids.is_empty() {
                let Some(next) = scheduler::next_due_time() else {
                    break; // queue truly empty
                };
                let now = Instant::now();
                if next > now {
                    let wait = next.saturating_duration_since(now)
                        .min(budget.saturating_sub(started.elapsed()));
                    std::thread::sleep(wait);
                }
                continue;
            }

            for (id, repeat) in due_ids {
                self.call_timer_callback(id, repeat.is_some())?;
                // Re-arm intervals *after* the callback returns. If
                // the callback called clearInterval(id) on itself
                // that cancellation is already recorded in the
                // scheduler, so the next pop_due_now will filter
                // this re-armed entry out before it can fire again.
                if let Some(period) = repeat {
                    scheduler::schedule(id, period, Some(period));
                }
            }

            if dom_guard.dirty_seen() {
                self.dom_dirty.set(true);
            }
            // dom_guard drops here, restoring nothing (we're the
            // outermost DOM context).
        }

        Ok(())
    }

    /// Look up a timer callback by id, call it with `this = window`,
    /// and for one-shots clear the array slot so the closure can be
    /// collected. Interval slots stay live because the same id is
    /// re-armed by the caller for the next firing.
    fn call_timer_callback(
        &mut self,
        id: scheduler::TimerId,
        is_interval: bool,
    ) -> Result<(), JsError> {
        // Per `set_timeout`'s id scheme, the JS-visible id is
        // `array_index + 1`. Translate back.
        let index = u64::from(id.saturating_sub(1));
        let timers = globals::timers::timers_array(&mut self.context)?;
        let cb_value = timers.get(index, &mut self.context)?;
        if cb_value.is_null_or_undefined() {
            return Ok(());
        }
        let Some(cb_obj) = cb_value.as_object().cloned() else {
            return Ok(());
        };
        if !cb_obj.is_callable() {
            return Ok(());
        }
        let global = JsValue::from(self.context.global_object());
        let _ = cb_obj.call(&global, &[], &mut self.context)?;
        if !is_interval {
            // One-shot: null the slot so the closure can be
            // collected on the next sweep. Setting to undefined
            // would also work; null is cheaper to type.
            let _ = timers.set(index, JsValue::null(), false, &mut self.context)?;
        }
        Ok(())
    }

    /// Return whether any [`execute`](Self::execute) call against this
    /// runtime has mutated the DOM via the bridge (`setAttribute`,
    /// `appendChild`, `textContent` setter, …) and clear the flag.
    ///
    /// koala-browser calls this once after running all of a
    /// document's scripts to decide whether to re-run style cascade
    /// and layout against the post-script tree.
    pub fn take_dom_dirty(&self) -> bool {
        self.dom_dirty.replace(false)
    }

    /// Fire a `DOMContentLoaded` event at `document`.
    ///
    /// [§ 7.5 Loading the document](https://html.spec.whatwg.org/multipage/parsing.html#the-end)
    ///
    /// Called by `koala-browser` after every sync script has run
    /// and before [`pump_until_idle`](Self::pump_until_idle), so
    /// any listener registered via `document.addEventListener(
    /// 'DOMContentLoaded', …)` runs on the just-parsed tree.
    ///
    /// `bubbles: true` and `cancelable: false` per spec, but
    /// today's dispatcher is strict-target-only so the bubble
    /// flag is recorded on the event without yet propagating up
    /// to `window` listeners. That symmetry lands when the
    /// bubble phase is implemented.
    ///
    /// Mutations triggered by listeners flow through the same
    /// `dom_dirty` channel as `execute` and `pump_until_idle`.
    ///
    /// # Errors
    ///
    /// Returns any [`JsError`] thrown synchronously by a listener.
    pub fn dispatch_dom_content_loaded(&mut self) -> Result<(), JsError> {
        self.dispatch_lifecycle_event(
            globals::document::DOCUMENT_SCOPE,
            js_string!("DOMContentLoaded"),
            /* bubbles */ true,
        )
    }

    /// Fire a `load` event at `window`.
    ///
    /// [§ 7.5 Loading the document](https://html.spec.whatwg.org/multipage/parsing.html#the-end)
    ///
    /// Called by `koala-browser` after the post-script
    /// [`pump_until_idle`](Self::pump_until_idle) returns. A
    /// follow-up `pump_until_idle` is expected so that work
    /// scheduled inside `load` listeners (typical pattern:
    /// `window.addEventListener('load', () => setTimeout(...))`)
    /// also runs before the document is considered done.
    ///
    /// `bubbles: false`, `cancelable: false` per spec.
    ///
    /// # Errors
    ///
    /// Returns any [`JsError`] thrown synchronously by a listener.
    pub fn dispatch_load(&mut self) -> Result<(), JsError> {
        self.dispatch_lifecycle_event(
            globals::window::WINDOW_SCOPE,
            js_string!("load"),
            /* bubbles */ false,
        )
    }

    /// Shared body for the lifecycle dispatchers. Installs the
    /// DOM guard (listeners may touch the DOM via the bridge),
    /// synthesises an `Event` with the supplied flags, and
    /// dispatches at the requested scope using the global object
    /// as both `target` and `currentTarget`.
    fn dispatch_lifecycle_event(
        &mut self,
        scope: &str,
        type_: JsString,
        bubbles: bool,
    ) -> Result<(), JsError> {
        let dom_guard = dom_handle::guard(self.dom.clone());
        let event = globals::events::make_event_object(
            &mut self.context,
            type_,
            bubbles,
            /* cancelable */ false,
        );
        let this_value = JsValue::from(self.context.global_object());
        let result = globals::events::dispatch_at_scope(
            scope,
            &this_value,
            &event,
            &mut self.context,
        );
        if dom_guard.dirty_seen() {
            self.dom_dirty.set(true);
        }
        result
    }
}

