//! End-to-end tests for the koala-js → koala-dom Phase-2 bridge.
//!
//! Each test parses an HTML string with an inline `<script>` that
//! exercises a piece of the DOM bridge and verifies the JS ran
//! without throwing by checking `parse_issues`. A successful run
//! means: HTML parser produced a DOM, koala-browser handed it to
//! `JsRuntime` as a shared handle, the script executed against the
//! real document, and DOM-bridge methods returned the values the
//! script expected.

#![allow(clippy::missing_docs_in_private_items, clippy::needless_raw_string_hashes)]

use koala_browser::parse_html_string;

fn assert_script_ran_clean(html: &str) {
    let doc = parse_html_string(html);
    let js_issues: Vec<_> = doc
        .parse_issues
        .iter()
        .filter(|s| s.starts_with("JavaScript error"))
        .collect();
    assert!(
        js_issues.is_empty(),
        "expected no JS errors, got: {js_issues:#?}",
    );
}

#[test]
fn script_can_read_get_element_by_id() {
    // The script throws if the bridge returns the wrong thing; the
    // koala-browser pipeline captures the throw into parse_issues,
    // which the helper above asserts is empty.
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <div id="banner" class="hero prominent">hi</div>
          <script>
            var el = document.getElementById('banner');
            if (el === null) throw new Error('expected element, got null');
            if (el.tagName !== 'DIV') throw new Error('tagName ' + el.tagName);
            if (el.id !== 'banner') throw new Error('id ' + el.id);
            if (el.className !== 'hero prominent') throw new Error('class ' + el.className);
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_get_element_by_id_returns_null_for_missing() {
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <script>
            var el = document.getElementById('does-not-exist');
            if (el !== null) throw new Error('expected null');
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_can_read_attributes() {
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <a id="link" href="https://example.com" data-track="yes">x</a>
          <script>
            var el = document.getElementById('link');
            if (el.getAttribute('href') !== 'https://example.com')
              throw new Error('href ' + el.getAttribute('href'));
            if (!el.hasAttribute('data-track'))
              throw new Error('missing data-track');
            if (el.hasAttribute('aria-hidden'))
              throw new Error('spurious aria-hidden');
            if (el.getAttribute('aria-hidden') !== null)
              throw new Error('missing attr should be null');
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_runs_against_first_matching_id_in_tree_order() {
    // Two elements share an id; spec says return the first in
    // tree order (the parser order matches our arena alloc order,
    // so the outer `<div>` should win).
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <div id="dup" class="outer"><div id="dup" class="inner">x</div></div>
          <script>
            var el = document.getElementById('dup');
            if (el.className !== 'outer') throw new Error('class ' + el.className);
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_can_walk_the_element_tree() {
    // Element-only tree navigators: parentElement, children,
    // first/lastElementChild, next/previousElementSibling. Verifies
    // both that they return the right element and that they skip
    // text nodes inserted by HTML whitespace.
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <ul id="list">
            <li id="a">A</li>
            <li id="b">B</li>
            <li id="c">C</li>
          </ul>
          <script>
            var list = document.getElementById('list');
            if (list.children.length !== 3)
              throw new Error('children.length ' + list.children.length);
            if (list.firstElementChild.id !== 'a')
              throw new Error('firstElementChild ' + list.firstElementChild.id);
            if (list.lastElementChild.id !== 'c')
              throw new Error('lastElementChild ' + list.lastElementChild.id);

            var b = document.getElementById('b');
            if (b.parentElement.id !== 'list')
              throw new Error('parentElement ' + b.parentElement.id);
            if (b.previousElementSibling.id !== 'a')
              throw new Error('prev sibling ' + b.previousElementSibling.id);
            if (b.nextElementSibling.id !== 'c')
              throw new Error('next sibling ' + b.nextElementSibling.id);

            // Leaf element has no children/siblings beyond text.
            var a = document.getElementById('a');
            if (a.firstElementChild !== null)
              throw new Error('a.firstElementChild should be null');
            if (a.previousElementSibling !== null)
              throw new Error('a.previousElementSibling should be null');
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_can_use_query_selector_and_text_content() {
    // testharness.js-style usage: query a known element, set its
    // textContent dynamically.
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <div id="out" class="result"></div>
          <p>other</p>
          <script>
            // Type, id, class, descendant — all should resolve.
            if (document.querySelector('div').id !== 'out')
              throw new Error('querySelector div');
            if (document.querySelector('#out').className !== 'result')
              throw new Error('querySelector #out');
            if (document.querySelector('.result').id !== 'out')
              throw new Error('querySelector .result');
            if (document.querySelectorAll('div, p').length !== 2)
              throw new Error('querySelectorAll count ' + document.querySelectorAll('div, p').length);

            // Set textContent — should replace any prior children with
            // a single Text node.
            var out = document.getElementById('out');
            out.textContent = 'hello from JS';
            if (out.textContent !== 'hello from JS')
              throw new Error('textContent read-back ' + out.textContent);
            if (out.children.length !== 0)
              throw new Error('out.children.length ' + out.children.length);
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_can_build_dom_via_create_element_and_append_child() {
    // Classic dynamic-rendering pattern: create elements, configure
    // them, attach to the document, observe via the bridge.
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <script>
            var p = document.createElement('p');
            p.setAttribute('id', 'inserted');
            p.textContent = 'inserted by script';
            document.body.appendChild(p);

            // Re-query through getElementById to prove the element
            // landed in the tree.
            var found = document.getElementById('inserted');
            if (found === null) throw new Error('inserted not found');
            if (found.tagName !== 'P') throw new Error('tagName ' + found.tagName);
            if (found.textContent !== 'inserted by script')
              throw new Error('textContent ' + found.textContent);
            if (found.parentElement.tagName !== 'BODY')
              throw new Error('parent ' + found.parentElement.tagName);
          </script>
        </body></html>"#,
    );
}

#[test]
fn script_can_read_document_title_and_window_globals() {
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html>
          <head><title>koala loves WPT</title></head>
          <body>
            <script>
              if (document.title !== 'koala loves WPT')
                throw new Error('title ' + document.title);
              if (typeof window !== 'object')
                throw new Error('window is not object');
              if (window !== window.window)
                throw new Error('window self-reference broken');
              if (window.document !== document)
                throw new Error('window.document not aliased');
              if (window.document.documentElement.tagName !== 'HTML')
                throw new Error('documentElement broken');
            </script>
          </body>
        </html>"#,
    );
}

#[test]
fn script_can_mutate_attributes_and_observe_via_get_attribute() {
    // Verifies the mutation path: setAttribute → DOM stores it →
    // getAttribute reads it back via the bridge.
    assert_script_ran_clean(
        r#"<!DOCTYPE html>
        <html><body>
          <button id="btn">x</button>
          <script>
            var b = document.getElementById('btn');
            b.setAttribute('aria-pressed', 'true');
            if (b.getAttribute('aria-pressed') !== 'true')
              throw new Error('setAttribute did not stick');

            b.setAttribute('aria-pressed', 'false');
            if (b.getAttribute('aria-pressed') !== 'false')
              throw new Error('setAttribute did not overwrite');

            b.removeAttribute('aria-pressed');
            if (b.hasAttribute('aria-pressed'))
              throw new Error('removeAttribute did not remove');
            if (b.getAttribute('aria-pressed') !== null)
              throw new Error('removed attribute should read null');

            b.removeAttribute('never-present');  // no-op, must not throw
          </script>
        </body></html>"#,
    );
}
