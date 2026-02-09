//! JavaScript engine integration for the Koala renderer.
//!
//! Uses [Boa](https://boajs.dev/) as the JavaScript engine.
//!
//! # Example
//!
//! ```ignore
//! use koala_js::JsRuntime;
//!
//! let mut runtime = JsRuntime::new();
//! runtime.execute("console.log('Hello from JS!');").unwrap();
//! ```
//!
//! # Implemented
//!
//! - Script execution via `JsRuntime::execute()`
//! - `console.log()`, `console.warn()`, `console.error()`
//!
//! # Not Yet Implemented
//!
//! ## DOM APIs
//!
//! [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//! - `document.getElementById()`
//! - `document.querySelector()` / `querySelectorAll()`
//! - `document.createElement()` / `createTextNode()`
//!
//! [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element)
//! - Element properties: `tagName`, `id`, `className`
//! - `getAttribute()` / `setAttribute()` / `removeAttribute()`
//! - `innerHTML` / `textContent`
//!
//! ## Events
//!
//! [§ 2.6 Interface EventTarget](https://dom.spec.whatwg.org/#interface-eventtarget)
//! - `addEventListener()` / `removeEventListener()`
//! - Event dispatch and propagation
//!
//! ## Timers
//!
//! [§ 8.6 Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
//! - `setTimeout()` / `clearTimeout()`
//! - `setInterval()` / `clearInterval()`
//!
//! ## Script Loading
//!
//! [§ 4.12.1.1 Processing model](https://html.spec.whatwg.org/multipage/scripting.html#script-processing-model)
//! - External scripts (`<script src="...">`)
//! - `async` and `defer` attributes
//! - Module scripts (`type="module"`)

mod globals;

use boa_engine::{Context, JsError, JsValue, Source};

/// JavaScript runtime for a document.
///
/// [§ 8.1.6 JavaScript execution context](https://html.spec.whatwg.org/multipage/webappapis.html)
///
/// Each document has its own JavaScript runtime with its own global object.
/// The runtime is created when the document is loaded and destroyed when
/// the document is unloaded.
pub struct JsRuntime {
    /// The Boa JavaScript context.
    context: Context,
}

impl JsRuntime {
    /// Create a new JavaScript runtime with global objects registered.
    ///
    /// This initializes the Boa context and registers built-in globals
    /// like `console`.
    #[must_use]
    pub fn new() -> Self {
        let mut context = Context::default();
        globals::register_globals(&mut context);
        Self { context }
    }

    /// Execute JavaScript source code.
    ///
    /// [§ 4.12.1.1 Processing model](https://html.spec.whatwg.org/multipage/scripting.html#script-processing-model)
    ///
    /// # Arguments
    ///
    /// * `source` - The JavaScript source code to execute.
    ///
    /// # Returns
    ///
    /// The result of evaluating the script, or a `JsError` if execution failed.
    ///
    /// # Errors
    ///
    /// Returns `JsError` if the JavaScript code contains syntax errors or
    /// throws an uncaught exception.
    pub fn execute(&mut self, source: &str) -> Result<JsValue, JsError> {
        self.context.eval(Source::from_bytes(source))
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new()
    }
}
