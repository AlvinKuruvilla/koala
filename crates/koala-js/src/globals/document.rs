//! Document interface implementation.
//!
//! [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
//!
//! "The Document interface represents any web page loaded in the browser
//! and serves as an entry point into the web page's content, which is the
//! DOM tree."
//!
//! # Not Yet Implemented
//!
//! This module is a stub. The following DOM Standard methods need implementation:
//!
//! ## Element Lookup
//!
//! [§ 5.1 getElementById](https://dom.spec.whatwg.org/#dom-nonelementparentnode-getelementbyid)
//! "Returns the first element within node's descendants whose ID is elementId."
//!
//! STEP 1: "If elementId is the empty string, return null."
//! STEP 2: "Return the first element in tree order within node's descendants
//!          whose ID is elementId; otherwise, return null."
//!
//! [§ 4.5 getElementsByClassName](https://dom.spec.whatwg.org/#dom-document-getelementsbyclassname)
//! "Returns a live `HTMLCollection` of elements with the given class names."
//!
//! [§ 4.5 getElementsByTagName](https://dom.spec.whatwg.org/#dom-document-getelementsbytagname)
//! "Returns a live `HTMLCollection` of elements with the given local name."
//!
//! ## Selectors API
//!
//! [§ 4.2.6 querySelector](https://dom.spec.whatwg.org/#dom-parentnode-queryselector)
//! "Returns the first element that is a descendant of node that matches selectors."
//!
//! STEP 1: "Let s be the result of parse a selector from selectors."
//! STEP 2: "If s is failure, throw a `SyntaxError` `DOMException`."
//! STEP 3: "Return the first result of running scope-match a selectors string
//!          selectors against node, if the result is non-empty; otherwise null."
//!
//! [§ 4.2.6 querySelectorAll](https://dom.spec.whatwg.org/#dom-parentnode-queryselectorall)
//! "Returns all element descendants of node that match selectors."
//!
//! ## Document Properties
//!
//! [§ 3.1.3 body](https://html.spec.whatwg.org/multipage/dom.html#dom-document-body)
//! "Returns the body element."
//!
//! [§ 3.1.3 head](https://html.spec.whatwg.org/multipage/dom.html#dom-document-head)
//! "Returns the head element."
//!
//! [§ 3.1.3 documentElement](https://dom.spec.whatwg.org/#dom-document-documentelement)
//! "Returns the document element."
//!
//! [§ 3.1.5 title](https://html.spec.whatwg.org/multipage/dom.html#document.title)
//! "Gets or sets the title of the document."
//!
//! ## Element Creation
//!
//! [§ 4.5 createElement](https://dom.spec.whatwg.org/#dom-document-createelement)
//! "Creates an element with the given local name."
//!
//! STEP 1: "If localName does not match the Name production, throw an
//!          `InvalidCharacterError` `DOMException`."
//! STEP 2: "If context object is an HTML document, let localName be converted
//!          to ASCII lowercase."
//! STEP 3: "Let is be null."
//! STEP 4: "If options is a dictionary and `options["is"]` exists, then set is
//!          to it."
//! STEP 5: "Let namespace be the HTML namespace if context object is an HTML
//!          document or context object's content type is 'application/xhtml+xml';
//!          otherwise null."
//! STEP 6: "Return the result of creating an element given context object,
//!          localName, namespace, null, is, and with the synchronous custom
//!          elements flag set."
//!
//! [§ 4.5 createTextNode](https://dom.spec.whatwg.org/#dom-document-createtextnode)
//! "Creates a Text node with the given data."
//!
//! [§ 4.5 createComment](https://dom.spec.whatwg.org/#dom-document-createcomment)
//! "Creates a Comment node with the given data."

use boa_engine::Context;

/// Register the document global object on the context.
///
/// [§ 4.5 Interface Document](https://dom.spec.whatwg.org/#interface-document)
///
/// NOTE: This is currently a stub. The document object requires access to the
/// DOM tree, which needs to be passed in when the runtime is created.
#[allow(dead_code)]
pub const fn register_document(_context: &mut Context) {
    // TODO: Implement document global
    //
    // This requires architectural changes:
    //
    // STEP 1: Pass a DomHandle (Rc<RefCell<DomTree>>) to JsRuntime::new()
    //
    // STEP 2: Create document object with:
    //   - getElementById(id) -> Element | null
    //   - querySelector(selectors) -> Element | null
    //   - querySelectorAll(selectors) -> NodeList
    //   - createElement(tagName) -> Element
    //   - createTextNode(data) -> Text
    //   - body -> HTMLBodyElement | null
    //   - head -> HTMLHeadElement | null
    //   - documentElement -> Element | null
    //   - title -> DOMString
    //
    // STEP 3: Create Element wrapper type (JsElement) that:
    //   - Holds NodeId + DomHandle
    //   - Implements NativeObject trait for Boa GC
    //   - Exposes tagName, id, className, getAttribute, setAttribute, etc.
    //
    // STEP 4: Register document as global property
}
