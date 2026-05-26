//! Phase-4 external script integration tests.
//!
//! Covers the `<script src="…">` loader path in koala-browser:
//! data: URLs, local files, mixed inline + external ordering,
//! and graceful failure when a `src` can't be fetched.

#![allow(clippy::missing_docs_in_private_items, clippy::needless_raw_string_hashes)]

use koala_browser::parse_html_string;
use koala_dom::{DomTree, NodeId};
use std::fs;

fn find_marker_attr(dom: &DomTree, attr: &str) -> Option<String> {
    for id in dom.iter_all() {
        if let Some(el) = dom.as_element(id) {
            if let Some(v) = el.attrs.get(attr) {
                return Some(v.clone());
            }
        }
    }
    None
}

fn find_by_id<'a>(dom: &'a DomTree, target: &str) -> Option<NodeId> {
    dom.iter_all().find(|&id| {
        dom.as_element(id)
            .and_then(|e| e.attrs.get("id"))
            .is_some_and(|v| v == target)
    })
}

fn js_errors(doc: &koala_browser::LoadedDocument) -> Vec<&str> {
    doc.parse_issues
        .iter()
        .filter(|s| s.starts_with("JavaScript error") || s.starts_with("Failed to load"))
        .map(String::as_str)
        .collect()
}

#[test]
fn data_url_script_runs_and_mutates_the_dom() {
    // data: URL bodies fetch synchronously through koala_common::net
    // without touching the network or filesystem. The script
    // tags the <body> with a marker attribute so the test can
    // verify it ran end-to-end.
    let html = r#"<!DOCTYPE html>
        <html><body id="root">
          <script src="data:text/javascript,document.body.setAttribute('data-loaded','yes')"></script>
        </body></html>"#;
    let doc = parse_html_string(html);
    assert!(js_errors(&doc).is_empty(), "unexpected issues: {:?}", doc.parse_issues);

    let marker = find_marker_attr(&doc.dom, "data-loaded");
    assert_eq!(
        marker.as_deref(),
        Some("yes"),
        "data: URL script should have set data-loaded on <body>",
    );
}

#[test]
fn inline_and_external_scripts_execute_in_document_order() {
    // Three scripts push their tags into globalThis.log. The
    // external script is sandwiched between two inline scripts;
    // the final assertion in the trailing inline block throws if
    // anything ran out of order, surfacing as a JS error in
    // parse_issues.
    let html = r#"<!DOCTYPE html>
        <html><body>
          <script>globalThis.log = ['a'];</script>
          <script src="data:text/javascript,globalThis.log.push('b')"></script>
          <script>
            globalThis.log.push('c');
            document.body.setAttribute('data-order', globalThis.log.join(','));
          </script>
        </body></html>"#;
    let doc = parse_html_string(html);
    assert!(js_errors(&doc).is_empty(), "unexpected issues: {:?}", doc.parse_issues);

    let order = find_marker_attr(&doc.dom, "data-order");
    assert_eq!(order.as_deref(), Some("a,b,c"));
}

#[test]
fn local_file_script_loads_and_executes() {
    // Write a real script to a per-test path so we exercise the
    // fs::read branch of fetch_script_source. The path is
    // absolute so it doesn't need a base URL to resolve.
    let tmp_dir = std::env::temp_dir();
    let script_path = tmp_dir.join(format!(
        "koala-phase4-{}.js",
        std::process::id(),
    ));
    fs::write(
        &script_path,
        "document.body.setAttribute('data-fs','loaded');",
    )
    .unwrap();

    let html = format!(
        r#"<!DOCTYPE html>
        <html><body>
          <script src="{}"></script>
        </body></html>"#,
        script_path.display(),
    );
    let doc = parse_html_string(&html);

    // Clean up before assertions so a failed assertion doesn't
    // leave the temp file behind.
    let _ = fs::remove_file(&script_path);

    assert!(js_errors(&doc).is_empty(), "unexpected issues: {:?}", doc.parse_issues);
    assert_eq!(
        find_marker_attr(&doc.dom, "data-fs").as_deref(),
        Some("loaded"),
    );
}

#[test]
fn missing_src_records_parse_issue_but_does_not_abort() {
    // The first script's src points at a path that won't exist;
    // the second is a working data: URL. Phase 4's contract is
    // "fetch failures are recorded but don't abort the rest of
    // the document" — verify the second one still runs.
    let html = r#"<!DOCTYPE html>
        <html><body>
          <script src="/definitely-not-a-real-path-koala-phase4.js"></script>
          <script src="data:text/javascript,document.body.setAttribute('data-after','ok')"></script>
        </body></html>"#;
    let doc = parse_html_string(html);

    // The failing script logs an issue …
    let load_failures: Vec<_> = doc
        .parse_issues
        .iter()
        .filter(|s| s.starts_with("Failed to load <script"))
        .collect();
    assert_eq!(
        load_failures.len(),
        1,
        "expected exactly one load failure, got: {:?}",
        doc.parse_issues,
    );

    // … but the trailing data: URL script still mutated the DOM.
    assert_eq!(
        find_marker_attr(&doc.dom, "data-after").as_deref(),
        Some("ok"),
    );
}

#[test]
fn empty_src_is_ignored() {
    // `<script src="">` should be skipped silently per spec — an
    // empty src is not a fetch target and not an inline script.
    let html = r#"<!DOCTYPE html>
        <html><body id="root">
          <script src=""></script>
          <script>document.body.setAttribute('data-after','ok');</script>
        </body></html>"#;
    let doc = parse_html_string(html);
    assert!(js_errors(&doc).is_empty(), "unexpected issues: {:?}", doc.parse_issues);
    assert!(find_by_id(&doc.dom, "root").is_some());
    assert_eq!(
        find_marker_attr(&doc.dom, "data-after").as_deref(),
        Some("ok"),
    );
}
