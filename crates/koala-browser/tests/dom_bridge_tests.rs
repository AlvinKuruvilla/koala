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
