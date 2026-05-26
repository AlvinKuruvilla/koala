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
//!   `Element.hasAttribute()`, `Element.setAttribute()`,
//!   `Element.removeAttribute()`
//!
//! # Not Yet Implemented
//!
//! ## DOM mutations
//!
//! [§ 4.9 Interface Element](https://dom.spec.whatwg.org/#interface-element)
//! - `innerHTML` (write) / `textContent` (read + write)
//! - `appendChild` / `removeChild` / `insertBefore`
//!
//! Mutations from JS update the DOM but do not yet trigger a
//! re-layout. Scripts run after the layout pass today, so visual
//! effects of JS mutations aren't visible until that pipeline is
//! rewired in a later Phase-2 follow-up.
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
    use koala_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};
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

    #[test]
    fn set_attribute_writes_through_to_the_dom() {
        let dom = fixture();
        let mut rt = JsRuntime::new(dom.clone());
        let _ = rt.execute(
            "document.getElementById('hello').setAttribute('data-track', 'no')",
        ).unwrap();

        // Confirm via the JS bridge…
        let after = rt.execute(
            "document.getElementById('hello').getAttribute('data-track')",
        ).unwrap();
        assert_eq!(
            after.to_string(&mut rt.context).unwrap().to_std_string_escaped(),
            "no",
        );
        // …and via the underlying DomTree (the bridge is supposed to
        // mutate the *shared* handle, not a copy).
        let direct = dom.borrow().as_element(NodeId(3))
            .and_then(|e| e.attrs.get("data-track").cloned());
        assert_eq!(direct.as_deref(), Some("no"));
    }

    #[test]
    fn set_attribute_adds_a_new_attribute() {
        let mut rt = JsRuntime::new(fixture());
        let _ = rt.execute(
            "document.getElementById('hello').setAttribute('aria-hidden', 'true')",
        ).unwrap();
        let v = rt.execute(
            "document.getElementById('hello').getAttribute('aria-hidden')",
        ).unwrap();
        assert_eq!(
            v.to_string(&mut rt.context).unwrap().to_std_string_escaped(),
            "true",
        );
    }

    #[test]
    fn remove_attribute_clears_an_existing_attribute() {
        let mut rt = JsRuntime::new(fixture());
        let _ = rt.execute(
            "document.getElementById('hello').removeAttribute('data-track')",
        ).unwrap();
        let has = rt.execute(
            "document.getElementById('hello').hasAttribute('data-track')",
        ).unwrap();
        assert_eq!(has.as_boolean(), Some(false));
        let val = rt.execute(
            "document.getElementById('hello').getAttribute('data-track')",
        ).unwrap();
        assert!(val.is_null());
    }

    #[test]
    fn remove_attribute_is_a_noop_for_missing_attribute() {
        let mut rt = JsRuntime::new(fixture());
        // Should not throw — spec says "remove an attribute given
        // qualifiedName and this, and then return undefined" with no
        // error path for "not found".
        let _ = rt.execute(
            "document.getElementById('hello').removeAttribute('href')",
        ).unwrap();
    }

    /// Multi-element fixture for tree-nav tests:
    ///
    ///   <html>
    ///     <body>
    ///       <ul id="list">
    ///         <li id="a">A</li>
    ///         <li id="b">B</li>
    ///         <li id="c">C</li>
    ///       </ul>
    ///     </body>
    ///   </html>
    fn list_fixture() -> DomHandle {
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

        let mut list_attrs = AttributesMap::new();
        let _ = list_attrs.insert("id".into(), "list".into());
        let list = tree.alloc(NodeType::Element(ElementData {
            tag_name: "ul".into(),
            attrs: list_attrs,
        }));
        tree.append_child(body, list);

        for id in ["a", "b", "c"] {
            let mut attrs = AttributesMap::new();
            let _ = attrs.insert("id".into(), id.into());
            let li = tree.alloc(NodeType::Element(ElementData {
                tag_name: "li".into(),
                attrs,
            }));
            tree.append_child(list, li);
            let text = tree.alloc(NodeType::Text(id.to_ascii_uppercase()));
            tree.append_child(li, text);
        }

        Rc::new(RefCell::new(tree))
    }

    fn run_and_string(rt: &mut JsRuntime, source: &str) -> String {
        rt.execute(source)
            .unwrap()
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped()
    }

    #[test]
    fn parent_element_walks_up() {
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(
            run_and_string(&mut rt, "document.getElementById('a').parentElement.id"),
            "list",
        );
        assert_eq!(
            run_and_string(&mut rt, "document.getElementById('list').parentElement.tagName"),
            "BODY",
        );
        // <html>'s parent is the Document node — parentElement should be null.
        let html_parent = rt
            .execute(
                "var html = document.getElementById('a').parentElement.parentElement.parentElement; html.parentElement",
            )
            .unwrap();
        assert!(html_parent.is_null());
    }

    #[test]
    fn children_returns_element_children_only() {
        let mut rt = JsRuntime::new(list_fixture());
        // <ul> has 3 <li> children plus text nodes from indentation; the
        // accessor should filter out text nodes.
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children.length"), "3");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children[0].id"), "a");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children[2].id"), "c");
    }

    #[test]
    fn first_and_last_element_child() {
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').firstElementChild.id"), "a");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').lastElementChild.id"), "c");
        // Leaf element has no element children.
        let leaf_first = rt.execute("document.getElementById('a').firstElementChild").unwrap();
        assert!(leaf_first.is_null());
    }

    #[test]
    fn document_body_head_and_document_element() {
        let mut rt = JsRuntime::new(list_fixture());
        // list_fixture builds: <html><body><ul id="list">…</ul></body></html>.
        // There's no <head> in the fixture, so document.head is null.
        assert_eq!(run_and_string(&mut rt, "document.documentElement.tagName"), "HTML");
        assert_eq!(run_and_string(&mut rt, "document.body.tagName"), "BODY");
        let head = rt.execute("document.head").unwrap();
        assert!(head.is_null(), "no <head> in fixture, should be null");
    }

    fn fixture_with_head() -> DomHandle {
        let mut tree = DomTree::new();
        let root = tree.root();
        let html = tree.alloc(NodeType::Element(ElementData {
            tag_name: "html".into(),
            attrs: AttributesMap::new(),
        }));
        tree.append_child(root, html);
        let head = tree.alloc(NodeType::Element(ElementData {
            tag_name: "head".into(),
            attrs: AttributesMap::new(),
        }));
        tree.append_child(html, head);
        let title = tree.alloc(NodeType::Element(ElementData {
            tag_name: "title".into(),
            attrs: AttributesMap::new(),
        }));
        tree.append_child(head, title);
        let title_text = tree.alloc(NodeType::Text("koala test page".into()));
        tree.append_child(title, title_text);
        let body = tree.alloc(NodeType::Element(ElementData {
            tag_name: "body".into(),
            attrs: AttributesMap::new(),
        }));
        tree.append_child(html, body);
        Rc::new(RefCell::new(tree))
    }

    #[test]
    fn document_title_returns_descendant_text() {
        let mut rt = JsRuntime::new(fixture_with_head());
        assert_eq!(run_and_string(&mut rt, "document.title"), "koala test page");
        assert_eq!(run_and_string(&mut rt, "document.head.tagName"), "HEAD");
    }

    #[test]
    fn document_create_element_returns_an_unattached_element() {
        let mut rt = JsRuntime::new(list_fixture());
        // Unattached element has the right tagName but null parent.
        assert_eq!(run_and_string(&mut rt, "document.createElement('span').tagName"), "SPAN");
        let parent = rt.execute("document.createElement('span').parentElement").unwrap();
        assert!(parent.is_null());
    }

    #[test]
    fn append_child_attaches_a_created_element() {
        let mut rt = JsRuntime::new(list_fixture());
        let _ = rt.execute(
            "var p = document.createElement('p');\
             p.setAttribute('id', 'fresh');\
             document.body.appendChild(p);",
        ).unwrap();
        // Now the element is findable by id, and its parent is body.
        assert_eq!(run_and_string(&mut rt, "document.getElementById('fresh').tagName"), "P");
        assert_eq!(
            run_and_string(&mut rt, "document.getElementById('fresh').parentElement.tagName"),
            "BODY",
        );
    }

    #[test]
    fn append_child_moves_a_node_with_an_existing_parent() {
        // Move `<li id="a">` out from under `<ul id="list">` into <body>.
        let mut rt = JsRuntime::new(list_fixture());
        let _ = rt.execute(
            "var a = document.getElementById('a');\
             document.body.appendChild(a);",
        ).unwrap();
        assert_eq!(
            run_and_string(&mut rt, "document.getElementById('a').parentElement.tagName"),
            "BODY",
        );
        // <ul> now has 2 children, not 3.
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children.length"), "2");
    }

    #[test]
    fn remove_child_detaches() {
        let mut rt = JsRuntime::new(list_fixture());
        // Hold a reference to b BEFORE detaching: once it's removed it
        // isn't reachable from the document root anymore, so a fresh
        // getElementById('b') would return null.
        let _ = rt.execute(
            "globalThis.b_ref = document.getElementById('b');\
             document.getElementById('list').removeChild(globalThis.b_ref);",
        ).unwrap();
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children.length"), "2");
        assert_eq!(run_and_string(&mut rt, "globalThis.b_ref.parentElement"), "null");
        // The detached node is now orphaned: getElementById can't find it.
        let missing = rt.execute("document.getElementById('b')").unwrap();
        assert!(missing.is_null());
    }

    #[test]
    fn text_content_getter_concatenates_descendants() {
        let mut rt = JsRuntime::new(list_fixture());
        // <ul> has three <li> children, each with one text Text child
        // ("A", "B", "C"). Concat ignores element boundaries.
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').textContent"), "ABC");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('a').textContent"), "A");
    }

    #[test]
    fn text_content_setter_replaces_children() {
        let mut rt = JsRuntime::new(list_fixture());
        let _ = rt.execute(
            "document.getElementById('list').textContent = 'replaced';",
        ).unwrap();
        // No element children remain — only the new Text node.
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children.length"), "0");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').textContent"), "replaced");
    }

    #[test]
    fn text_content_setter_with_empty_string_clears() {
        let mut rt = JsRuntime::new(list_fixture());
        let _ = rt.execute(
            "document.getElementById('list').textContent = '';",
        ).unwrap();
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').textContent"), "");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('list').children.length"), "0");
    }

    #[test]
    fn document_query_selector_resolves_simple_selectors() {
        let mut rt = JsRuntime::new(list_fixture());
        // Type selector
        assert_eq!(run_and_string(&mut rt, "document.querySelector('ul').id"), "list");
        // ID selector
        assert_eq!(run_and_string(&mut rt, "document.querySelector('#b').tagName"), "LI");
        // Descendant combinator
        assert_eq!(
            run_and_string(&mut rt, "document.querySelector('ul li').id"),
            "a",
        );
        // No match
        let none = rt.execute("document.querySelector('.does-not-exist')").unwrap();
        assert!(none.is_null());
    }

    #[test]
    fn document_query_selector_all_returns_every_match() {
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(run_and_string(&mut rt, "document.querySelectorAll('li').length"), "3");
        assert_eq!(
            run_and_string(&mut rt, "document.querySelectorAll('li')[1].id"),
            "b",
        );
    }

    #[test]
    fn element_query_selector_is_scoped() {
        // querySelector on an element is scoped to its descendants, so
        // `body.querySelector('li')` shouldn't pick up `<li>` outside
        // of body — though in this fixture there are no other <li>s.
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(
            run_and_string(&mut rt, "document.getElementById('list').querySelector('li').id"),
            "a",
        );
        assert_eq!(
            run_and_string(&mut rt, "document.getElementById('list').querySelectorAll('li').length"),
            "3",
        );
    }

    #[test]
    fn get_elements_by_tag_name_returns_an_array() {
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(run_and_string(&mut rt, "document.getElementsByTagName('li').length"), "3");
        // Wildcard returns every element.
        let all = run_and_string(&mut rt, "document.getElementsByTagName('*').length");
        assert!(all.parse::<usize>().unwrap() >= 5, "wildcard count was {all}");
    }

    #[test]
    fn get_elements_by_class_name_matches_all_classes() {
        let mut rt = JsRuntime::new(fixture());
        // The fixture's <div id='hello'> has class "greeting prominent"
        assert_eq!(
            run_and_string(&mut rt, "document.getElementsByClassName('greeting').length"),
            "1",
        );
        assert_eq!(
            run_and_string(&mut rt, "document.getElementsByClassName('greeting prominent').length"),
            "1",
        );
        // All requested classes must be present; absent class → zero hits.
        assert_eq!(
            run_and_string(&mut rt, "document.getElementsByClassName('greeting missing').length"),
            "0",
        );
    }

    #[test]
    fn window_is_self_referential_and_exposes_document() {
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(run_and_string(&mut rt, "typeof window"), "object");
        assert_eq!(run_and_string(&mut rt, "window === window.window"), "true");
        assert_eq!(run_and_string(&mut rt, "window.document === document"), "true");
        assert_eq!(run_and_string(&mut rt, "window.document.body.tagName"), "BODY");
    }

    #[test]
    fn next_and_previous_element_sibling() {
        let mut rt = JsRuntime::new(list_fixture());
        assert_eq!(run_and_string(&mut rt, "document.getElementById('a').nextElementSibling.id"), "b");
        assert_eq!(run_and_string(&mut rt, "document.getElementById('b').nextElementSibling.id"), "c");
        let last_next = rt
            .execute("document.getElementById('c').nextElementSibling")
            .unwrap();
        assert!(last_next.is_null());

        assert_eq!(run_and_string(&mut rt, "document.getElementById('c').previousElementSibling.id"), "b");
        let first_prev = rt
            .execute("document.getElementById('a').previousElementSibling")
            .unwrap();
        assert!(first_prev.is_null());
    }
}
