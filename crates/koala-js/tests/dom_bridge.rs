//! Phase-2 DOM bridge integration tests.
//!
//! Each test constructs a fixture, drives a `JsRuntime` through
//! `execute` / `eval_to_string`, and asserts the bridge reports
//! the value the spec demands. The companion tests in
//! `crates/koala-browser/tests/dom_bridge_tests.rs` cover the
//! same surface via real HTML parsing + script extraction.

use koala_dom::NodeId;
use koala_js::JsRuntime;

mod common;
use common::{fixture, fixture_with_head, list_fixture};

#[test]
fn document_is_a_global() {
    let mut rt = JsRuntime::new(fixture());
    assert_eq!(rt.eval_to_string("typeof document").unwrap(), "object");
}

#[test]
fn get_element_by_id_returns_an_element() {
    let mut rt = JsRuntime::new(fixture());
    // Per the DOM spec, Element.tagName is uppercase for HTML elements.
    assert_eq!(
        rt.eval_to_string("var el = document.getElementById('hello'); el.tagName")
            .unwrap(),
        "DIV",
    );
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
    assert_eq!(
        rt.eval_to_string("document.getElementById('hello').id").unwrap(),
        "hello",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('hello').className")
            .unwrap(),
        "greeting prominent",
    );
}

#[test]
fn get_attribute_returns_the_value_or_null() {
    let mut rt = JsRuntime::new(fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('hello').getAttribute('data-track')")
            .unwrap(),
        "yes",
    );
    let missing = rt
        .execute("document.getElementById('hello').getAttribute('aria-hidden')")
        .unwrap();
    assert!(missing.is_null());
}

#[test]
fn has_attribute_returns_a_boolean() {
    let mut rt = JsRuntime::new(fixture());
    let yes = rt
        .execute("document.getElementById('hello').hasAttribute('id')")
        .unwrap();
    assert_eq!(yes.as_boolean(), Some(true));
    let no = rt
        .execute("document.getElementById('hello').hasAttribute('href')")
        .unwrap();
    assert_eq!(no.as_boolean(), Some(false));
}

#[test]
fn set_attribute_writes_through_to_the_dom() {
    let dom = fixture();
    let mut rt = JsRuntime::new(dom.clone());
    let _ = rt
        .execute(
            "document.getElementById('hello').setAttribute('data-track', 'no')",
        )
        .unwrap();
    // Confirm via the JS bridge…
    assert_eq!(
        rt.eval_to_string(
            "document.getElementById('hello').getAttribute('data-track')",
        )
        .unwrap(),
        "no",
    );
    // …and via the underlying DomTree (the bridge mutates the shared
    // handle, not a copy).
    let direct = dom
        .borrow()
        .as_element(NodeId(3))
        .and_then(|e| e.attrs.get("data-track").cloned());
    assert_eq!(direct.as_deref(), Some("no"));
}

#[test]
fn set_attribute_adds_a_new_attribute() {
    let mut rt = JsRuntime::new(fixture());
    let _ = rt
        .execute(
            "document.getElementById('hello').setAttribute('aria-hidden', 'true')",
        )
        .unwrap();
    assert_eq!(
        rt.eval_to_string(
            "document.getElementById('hello').getAttribute('aria-hidden')",
        )
        .unwrap(),
        "true",
    );
}

#[test]
fn remove_attribute_clears_an_existing_attribute() {
    let mut rt = JsRuntime::new(fixture());
    let _ = rt
        .execute("document.getElementById('hello').removeAttribute('data-track')")
        .unwrap();
    let has = rt
        .execute("document.getElementById('hello').hasAttribute('data-track')")
        .unwrap();
    assert_eq!(has.as_boolean(), Some(false));
    let val = rt
        .execute("document.getElementById('hello').getAttribute('data-track')")
        .unwrap();
    assert!(val.is_null());
}

#[test]
fn remove_attribute_is_a_noop_for_missing_attribute() {
    let mut rt = JsRuntime::new(fixture());
    // Spec: "remove an attribute given qualifiedName and this, and
    // then return undefined" — no error path for "not found".
    let _ = rt
        .execute("document.getElementById('hello').removeAttribute('href')")
        .unwrap();
}

#[test]
fn parent_element_walks_up() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('a').parentElement.id").unwrap(),
        "list",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').parentElement.tagName")
            .unwrap(),
        "BODY",
    );
    // <html>'s parent is the Document node — parentElement → null.
    let html_parent = rt
        .execute(
            "var html = document.getElementById('a').parentElement.parentElement.parentElement;\
             html.parentElement",
        )
        .unwrap();
    assert!(html_parent.is_null());
}

#[test]
fn children_returns_element_children_only() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children.length")
            .unwrap(),
        "3",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children[0].id")
            .unwrap(),
        "a",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children[2].id")
            .unwrap(),
        "c",
    );
}

#[test]
fn first_and_last_element_child() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').firstElementChild.id")
            .unwrap(),
        "a",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').lastElementChild.id")
            .unwrap(),
        "c",
    );
    let leaf_first = rt
        .execute("document.getElementById('a').firstElementChild")
        .unwrap();
    assert!(leaf_first.is_null());
}

#[test]
fn next_and_previous_element_sibling() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('a').nextElementSibling.id")
            .unwrap(),
        "b",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('b').nextElementSibling.id")
            .unwrap(),
        "c",
    );
    let last_next = rt
        .execute("document.getElementById('c').nextElementSibling")
        .unwrap();
    assert!(last_next.is_null());
    assert_eq!(
        rt.eval_to_string("document.getElementById('c').previousElementSibling.id")
            .unwrap(),
        "b",
    );
    let first_prev = rt
        .execute("document.getElementById('a').previousElementSibling")
        .unwrap();
    assert!(first_prev.is_null());
}

#[test]
fn document_body_head_and_document_element() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.documentElement.tagName").unwrap(),
        "HTML",
    );
    assert_eq!(rt.eval_to_string("document.body.tagName").unwrap(), "BODY");
    // list_fixture has no <head>.
    let head = rt.execute("document.head").unwrap();
    assert!(head.is_null());
}

#[test]
fn document_title_returns_descendant_text() {
    let mut rt = JsRuntime::new(fixture_with_head());
    assert_eq!(rt.eval_to_string("document.title").unwrap(), "koala test page");
    assert_eq!(rt.eval_to_string("document.head.tagName").unwrap(), "HEAD");
}

#[test]
fn document_create_element_returns_an_unattached_element() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.createElement('span').tagName").unwrap(),
        "SPAN",
    );
    let parent = rt
        .execute("document.createElement('span').parentElement")
        .unwrap();
    assert!(parent.is_null());
}

#[test]
fn append_child_attaches_a_created_element() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "var p = document.createElement('p');\
             p.setAttribute('id', 'fresh');\
             document.body.appendChild(p);",
        )
        .unwrap();
    assert_eq!(
        rt.eval_to_string("document.getElementById('fresh').tagName").unwrap(),
        "P",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('fresh').parentElement.tagName")
            .unwrap(),
        "BODY",
    );
}

#[test]
fn append_child_moves_a_node_with_an_existing_parent() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            "var a = document.getElementById('a');\
             document.body.appendChild(a);",
        )
        .unwrap();
    assert_eq!(
        rt.eval_to_string("document.getElementById('a').parentElement.tagName")
            .unwrap(),
        "BODY",
    );
    // <ul> now has 2 children, not 3.
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children.length")
            .unwrap(),
        "2",
    );
}

#[test]
fn remove_child_detaches() {
    let mut rt = JsRuntime::new(list_fixture());
    // Hold a reference to b BEFORE detaching: once removed, the node
    // isn't reachable from the document root, so a fresh
    // getElementById('b') would return null.
    let _ = rt
        .execute(
            "globalThis.b_ref = document.getElementById('b');\
             document.getElementById('list').removeChild(globalThis.b_ref);",
        )
        .unwrap();
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children.length")
            .unwrap(),
        "2",
    );
    assert_eq!(rt.eval_to_string("globalThis.b_ref.parentElement").unwrap(), "null");
    let missing = rt.execute("document.getElementById('b')").unwrap();
    assert!(missing.is_null());
}

#[test]
fn text_content_getter_concatenates_descendants() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').textContent").unwrap(),
        "ABC",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('a').textContent").unwrap(),
        "A",
    );
}

#[test]
fn text_content_setter_replaces_children() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute("document.getElementById('list').textContent = 'replaced';")
        .unwrap();
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children.length")
            .unwrap(),
        "0",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').textContent")
            .unwrap(),
        "replaced",
    );
}

#[test]
fn text_content_setter_with_empty_string_clears() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute("document.getElementById('list').textContent = '';")
        .unwrap();
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').textContent")
            .unwrap(),
        "",
    );
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').children.length")
            .unwrap(),
        "0",
    );
}

#[test]
fn document_query_selector_resolves_simple_selectors() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.querySelector('ul').id").unwrap(),
        "list",
    );
    assert_eq!(
        rt.eval_to_string("document.querySelector('#b').tagName").unwrap(),
        "LI",
    );
    assert_eq!(
        rt.eval_to_string("document.querySelector('ul li').id").unwrap(),
        "a",
    );
    let none = rt
        .execute("document.querySelector('.does-not-exist')")
        .unwrap();
    assert!(none.is_null());
}

#[test]
fn document_query_selector_all_returns_every_match() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.querySelectorAll('li').length").unwrap(),
        "3",
    );
    assert_eq!(
        rt.eval_to_string("document.querySelectorAll('li')[1].id").unwrap(),
        "b",
    );
}

#[test]
fn element_query_selector_is_scoped() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementById('list').querySelector('li').id")
            .unwrap(),
        "a",
    );
    assert_eq!(
        rt.eval_to_string(
            "document.getElementById('list').querySelectorAll('li').length",
        )
        .unwrap(),
        "3",
    );
}

#[test]
fn get_elements_by_tag_name_returns_an_array() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementsByTagName('li').length").unwrap(),
        "3",
    );
    let all = rt
        .eval_to_string("document.getElementsByTagName('*').length")
        .unwrap();
    assert!(all.parse::<usize>().unwrap() >= 5, "wildcard count was {all}");
}

#[test]
fn get_elements_by_class_name_matches_all_classes() {
    let mut rt = JsRuntime::new(fixture());
    assert_eq!(
        rt.eval_to_string("document.getElementsByClassName('greeting').length")
            .unwrap(),
        "1",
    );
    assert_eq!(
        rt.eval_to_string(
            "document.getElementsByClassName('greeting prominent').length",
        )
        .unwrap(),
        "1",
    );
    // All requested classes must be present; absent class → zero hits.
    assert_eq!(
        rt.eval_to_string("document.getElementsByClassName('greeting missing').length")
            .unwrap(),
        "0",
    );
}

#[test]
fn window_is_self_referential_and_exposes_document() {
    let mut rt = JsRuntime::new(list_fixture());
    assert_eq!(rt.eval_to_string("typeof window").unwrap(), "object");
    assert_eq!(rt.eval_to_string("window === window.window").unwrap(), "true");
    assert_eq!(rt.eval_to_string("window.document === document").unwrap(), "true");
    assert_eq!(
        rt.eval_to_string("window.document.body.tagName").unwrap(),
        "BODY",
    );
}

// ---- DOM interface prototype chain (Tier 1) ----

#[test]
fn interface_constructors_are_globals() {
    let mut rt = JsRuntime::new(fixture());
    for name in ["EventTarget", "Node", "Element", "HTMLElement"] {
        let typeof_ = rt
            .eval_to_string(&format!("typeof {name}"))
            .unwrap();
        assert_eq!(
            typeof_, "function",
            "{name} should be a global constructor"
        );
    }
}

#[test]
fn element_instanceof_walks_the_full_chain() {
    let mut rt = JsRuntime::new(fixture());
    let _ = rt.execute("var el = document.getElementById('hello');").unwrap();
    for name in ["HTMLElement", "Element", "Node", "EventTarget"] {
        let answer = rt
            .eval_to_string(&format!("el instanceof {name}"))
            .unwrap();
        assert_eq!(answer, "true", "el instanceof {name} should be true");
    }
}

#[test]
fn html_element_prototype_chain_matches_spec() {
    // Object.getPrototypeOf chain must be exactly HTMLElement →
    // Element → Node → EventTarget → Object — what testharness
    // tests assert with `prototypeChain` helpers.
    let mut rt = JsRuntime::new(fixture());
    let _ = rt.execute("var el = document.getElementById('hello');").unwrap();
    assert_eq!(
        rt.eval_to_string("Object.getPrototypeOf(el) === HTMLElement.prototype")
            .unwrap(),
        "true",
    );
    assert_eq!(
        rt.eval_to_string("Object.getPrototypeOf(HTMLElement.prototype) === Element.prototype")
            .unwrap(),
        "true",
    );
    assert_eq!(
        rt.eval_to_string("Object.getPrototypeOf(Element.prototype) === Node.prototype")
            .unwrap(),
        "true",
    );
    assert_eq!(
        rt.eval_to_string("Object.getPrototypeOf(Node.prototype) === EventTarget.prototype")
            .unwrap(),
        "true",
    );
}

#[test]
fn interface_prototype_constructor_back_links_to_itself() {
    let mut rt = JsRuntime::new(fixture());
    for name in ["EventTarget", "Node", "Element", "HTMLElement"] {
        assert_eq!(
            rt.eval_to_string(&format!("{name}.prototype.constructor === {name}"))
                .unwrap(),
            "true",
            "{name}.prototype.constructor should round-trip",
        );
    }
}

#[test]
fn node_element_html_element_constructors_throw_on_new() {
    // `Node`, `Element`, and `HTMLElement` have no public IDL
    // constructor — web content can't say `new Element()`.
    // (Per DOM § 4.4 / § 4.9 the interface objects exist for
    // `instanceof` and prototype lookup, but their `[[Call]]`
    // and `[[Construct]]` slots throw.)
    let mut rt = JsRuntime::new(fixture());
    for name in ["Node", "Element", "HTMLElement"] {
        let err = rt
            .execute(&format!("new {name}()"))
            .expect_err(&format!("new {name}() should throw"));
        let msg = format!("{err}");
        assert!(
            msg.contains("Illegal constructor") && msg.contains(name),
            "expected `Illegal constructor: {name}` in error message, got: {msg}",
        );
    }
}

#[test]
fn event_target_is_constructible() {
    // EventTarget IS constructible per DOM § 2.6:
    //   `[Exposed=*] interface EventTarget { constructor(); ... }`.
    // The wrapper produced by `new EventTarget()` must:
    //   - be an EventTarget instance,
    //   - carry the prototype-installed addEventListener /
    //     removeEventListener / dispatchEvent methods, and
    //   - fire user listeners on dispatchEvent.
    let mut rt = JsRuntime::new(fixture());
    assert_eq!(
        rt.eval_to_string("(new EventTarget()) instanceof EventTarget")
            .unwrap(),
        "true",
    );
    // Methods come off the prototype, not own properties — the
    // whole point of moving to Boa's Class trait.
    assert_eq!(
        rt.eval_to_string(
            "Object.getPrototypeOf(new EventTarget()) === EventTarget.prototype"
        )
        .unwrap(),
        "true",
    );
    assert_eq!(
        rt.eval_to_string("typeof EventTarget.prototype.addEventListener")
            .unwrap(),
        "function",
    );

    // End-to-end: register listener, dispatch, observe.
    let _ = rt
        .execute(
            r#"
        var et = new EventTarget();
        var fired = 0;
        et.addEventListener('ping', function () { fired += 1; });
        et.dispatchEvent(new Event('ping'));
        et.dispatchEvent(new Event('ping'));
        "#,
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("fired").unwrap(), "2");
}

// ---- Node.prototype migration (Tier 1, second slice) ----

#[test]
fn node_constructor_throws_but_methods_live_on_the_prototype() {
    // Node is abstract — `new Node()` must throw — but
    // Node.prototype must still carry the methods/accessors so
    // every wrapped node inherits them. This is the canonical
    // shape for an abstract DOM interface.
    let mut rt = JsRuntime::new(fixture());
    let err = rt
        .execute("new Node()")
        .expect_err("new Node() should throw");
    assert!(
        format!("{err}").contains("Illegal constructor"),
        "expected Illegal constructor message, got: {err}",
    );
    assert_eq!(
        rt.eval_to_string("typeof Node.prototype.appendChild").unwrap(),
        "function",
    );
    assert_eq!(
        rt.eval_to_string("typeof Node.prototype.removeChild").unwrap(),
        "function",
    );
    assert_eq!(
        rt.eval_to_string("typeof Node.prototype.contains").unwrap(),
        "function",
    );
    // Accessor functions appear via `getOwnPropertyDescriptor`,
    // not as plain `typeof prototype.foo === "function"`, so
    // check existence through the descriptor.
    for name in ["nodeType", "nodeName", "parentNode"] {
        let answer = rt
            .eval_to_string(&format!(
                "typeof Object.getOwnPropertyDescriptor(Node.prototype, '{name}').get"
            ))
            .unwrap();
        assert_eq!(
            answer, "function",
            "expected Node.prototype to expose a `{name}` getter",
        );
    }
}

#[test]
fn element_inherits_node_prototype_methods() {
    // The Element wrapper no longer carries `appendChild` etc.
    // as own properties; they resolve via the prototype chain.
    // Test both the method-presence side and behavioural
    // equivalence with what the own-properties used to do.
    let mut rt = JsRuntime::new(fixture());
    let _ = rt
        .execute("var el = document.getElementById('hello');")
        .unwrap();
    // appendChild / removeChild come off Node.prototype now.
    assert_eq!(
        rt.eval_to_string(
            "el.appendChild === Node.prototype.appendChild"
        )
        .unwrap(),
        "true",
        "el.appendChild should resolve to Node.prototype.appendChild",
    );
    // nodeType comes off Node.prototype's accessor — and still
    // returns 1 for an element.
    assert_eq!(rt.eval_to_string("el.nodeType").unwrap(), "1");
    // nodeName uppercases the tag for elements (DOM spec).
    assert_eq!(rt.eval_to_string("el.nodeName").unwrap(), "DIV");
}

#[test]
fn node_contains_walks_the_descendant_chain() {
    let mut rt = JsRuntime::new(list_fixture());
    let _ = rt
        .execute(
            r#"
        var body = document.body;
        var ul   = body.firstElementChild;
        var li   = ul.firstElementChild;
        "#,
        )
        .unwrap();
    // body contains itself, the ul, and the li
    assert_eq!(rt.eval_to_string("body.contains(body)").unwrap(), "true");
    assert_eq!(rt.eval_to_string("body.contains(ul)").unwrap(), "true");
    assert_eq!(rt.eval_to_string("body.contains(li)").unwrap(), "true");
    // ul contains the li but not the body
    assert_eq!(rt.eval_to_string("ul.contains(li)").unwrap(), "true");
    assert_eq!(rt.eval_to_string("ul.contains(body)").unwrap(), "false");
    // contains(null) returns false (spec § 4.4)
    assert_eq!(rt.eval_to_string("body.contains(null)").unwrap(), "false");
}

#[test]
fn distinct_event_target_instances_have_isolated_listener_buckets() {
    // Each `new EventTarget()` mints a fresh scope_key, so a
    // listener on `a` must not fire when an event is dispatched
    // on `b`. This is what makes EventTarget instances a real
    // identity rather than aliases for a shared bucket.
    let mut rt = JsRuntime::new(fixture());
    let _ = rt
        .execute(
            r#"
        var a = new EventTarget();
        var b = new EventTarget();
        var a_fired = 0;
        var b_fired = 0;
        a.addEventListener('x', function () { a_fired += 1; });
        b.addEventListener('x', function () { b_fired += 1; });
        a.dispatchEvent(new Event('x'));
        b.dispatchEvent(new Event('x'));
        b.dispatchEvent(new Event('x'));
        "#,
        )
        .unwrap();
    assert_eq!(rt.eval_to_string("a_fired").unwrap(), "1");
    assert_eq!(rt.eval_to_string("b_fired").unwrap(), "2");
}

#[test]
fn element_id_is_writable_and_reflects_to_attribute() {
    let mut rt = JsRuntime::new(fixture());
    let _ = rt.execute("var el = document.getElementById('hello');").unwrap();
    // Read: live accessor returns the current attribute value.
    assert_eq!(rt.eval_to_string("el.id").unwrap(), "hello");
    // Write: setter routes through the same attribute store
    // setAttribute uses, so both views stay in sync.
    let _ = rt.execute("el.id = 'changed';").unwrap();
    assert_eq!(rt.eval_to_string("el.id").unwrap(), "changed");
    assert_eq!(
        rt.eval_to_string("el.getAttribute('id')").unwrap(),
        "changed",
    );
}

#[test]
fn element_class_name_is_writable_and_reflects_to_attribute() {
    let mut rt = JsRuntime::new(fixture());
    let _ = rt.execute("var el = document.getElementById('hello');").unwrap();
    assert_eq!(
        rt.eval_to_string("el.className").unwrap(),
        "greeting prominent",
    );
    let _ = rt.execute("el.className = 'replaced';").unwrap();
    assert_eq!(rt.eval_to_string("el.className").unwrap(), "replaced");
    assert_eq!(
        rt.eval_to_string("el.getAttribute('class')").unwrap(),
        "replaced",
    );
}

#[test]
fn set_attribute_id_is_visible_through_idl_getter() {
    // The reverse direction: writing the content attribute via
    // setAttribute must immediately show up on the IDL getter.
    let mut rt = JsRuntime::new(fixture());
    let _ = rt.execute("var el = document.getElementById('hello');").unwrap();
    let _ = rt.execute("el.setAttribute('id', 'attr-route');").unwrap();
    assert_eq!(rt.eval_to_string("el.id").unwrap(), "attr-route");
}
