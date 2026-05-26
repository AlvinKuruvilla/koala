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
//! - Script execution via `JsRuntime::execute()`
//! - `console.log()`, `console.warn()`, `console.error()`
//! - DOM bridge: `document.getElementById()`, `Element.tagName`,
//!   `Element.id`, `Element.className`, `Element.getAttribute()`,
//!   `Element.hasAttribute()`
//!
//! # Not Yet Implemented
//!
//! ## DOM mutations
//!
//! [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element)
//! - `setAttribute()` / `removeAttribute()`
//! - `innerHTML` (write) / `textContent` (write)
//! - `appendChild` / `removeChild` / `insertBefore`
//!
//! ## More queries
//!
//! - `document.querySelector()` / `querySelectorAll()`
//! - `document.getElementsByTagName()` / `getElementsByClassName()`
//! - Element tree navigation (`parentNode`, `children`, …)
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

mod dom_handle;
mod globals;

pub use dom_handle::DomHandle;

use boa_engine::{Context, JsError, JsValue, Source};

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
        let mut context = Context::default();
        globals::register_globals(&mut context);
        Self { context, dom }
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
        let _guard = dom_handle::guard(self.dom.clone());
        self.context.eval(Source::from_bytes(source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koala_dom::{AttributesMap, DomTree, ElementData, NodeType};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn fixture() -> DomHandle {
        // <html><body><div id="hello">hi</div></body></html>
        let mut tree = DomTree::new();
        let root = tree.root();
        let html = tree.alloc(NodeType::Element(ElementData {
            tag_name: "html".to_string(),
            attrs: AttributesMap::new(),
        }));
        tree.append_child(root, html);
        let body = tree.alloc(NodeType::Element(ElementData {
            tag_name: "body".to_string(),
            attrs: AttributesMap::new(),
        }));
        tree.append_child(html, body);

        let mut div_attrs = AttributesMap::new();
        let _ = div_attrs.insert("id".to_string(), "hello".to_string());
        let _ = div_attrs.insert("class".to_string(), "greeting prominent".to_string());
        let _ = div_attrs.insert("data-track".to_string(), "yes".to_string());
        let div = tree.alloc(NodeType::Element(ElementData {
            tag_name: "div".to_string(),
            attrs: div_attrs,
        }));
        tree.append_child(body, div);
        let text = tree.alloc(NodeType::Text("hi".into()));
        tree.append_child(div, text);

        Rc::new(RefCell::new(tree))
    }

    #[test]
    fn document_is_a_global() {
        let mut rt = JsRuntime::new(fixture());
        let result = rt.execute("typeof document").unwrap();
        let s = result.to_string(&mut rt.context).unwrap();
        assert_eq!(s.to_std_string_escaped(), "object");
    }

    #[test]
    fn get_element_by_id_returns_an_element() {
        let mut rt = JsRuntime::new(fixture());
        let result = rt.execute(
            "var el = document.getElementById('hello'); el.tagName"
        ).unwrap();
        let s = result.to_string(&mut rt.context).unwrap();
        // Per the DOM spec, Element.tagName is uppercase for HTML elements.
        assert_eq!(s.to_std_string_escaped(), "DIV");
    }

    #[test]
    fn get_element_by_id_returns_null_for_missing() {
        let mut rt = JsRuntime::new(fixture());
        let result = rt.execute("document.getElementById('missing')").unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn element_exposes_id_and_class_name() {
        let mut rt = JsRuntime::new(fixture());
        let id = rt.execute("document.getElementById('hello').id").unwrap();
        assert_eq!(id.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "hello");
        let cls = rt.execute("document.getElementById('hello').className").unwrap();
        assert_eq!(
            cls.to_string(&mut rt.context).unwrap().to_std_string_escaped(),
            "greeting prominent",
        );
    }

    #[test]
    fn get_attribute_returns_the_value_or_null() {
        let mut rt = JsRuntime::new(fixture());
        let val = rt.execute(
            "document.getElementById('hello').getAttribute('data-track')"
        ).unwrap();
        assert_eq!(val.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "yes");

        let missing = rt.execute(
            "document.getElementById('hello').getAttribute('aria-hidden')"
        ).unwrap();
        assert!(missing.is_null());
    }

    #[test]
    fn has_attribute_returns_a_boolean() {
        let mut rt = JsRuntime::new(fixture());
        let yes = rt.execute(
            "document.getElementById('hello').hasAttribute('id')"
        ).unwrap();
        assert_eq!(yes.as_boolean(), Some(true));

        let no = rt.execute(
            "document.getElementById('hello').hasAttribute('href')"
        ).unwrap();
        assert_eq!(no.as_boolean(), Some(false));
    }
}
