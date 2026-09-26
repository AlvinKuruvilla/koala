//! Local files addressed by `file:` URL, including paths containing
//! characters that are special in URLs.

use std::path::PathBuf;

use koala_common::net::{DefaultSender, FetchError, RequestSender};
use koala_common::url::{file_url_from_path, resolve_url};

/// A fresh directory whose name contains a space, `#`, `%`, and `?`.
/// Unencoded, `#` would start a fragment, `?` a query, and `%` an
/// escape, so any of them cutting the path short shows up as a failed
/// read.
fn awkward_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "koala file #1 50% ?q-{}-{test}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir is writable");
    dir
}

#[test]
fn file_url_reads_back_the_same_file() {
    let dir = awkward_dir("read");
    let page = dir.join("page.html");
    std::fs::write(&page, "<p>hi</p>").expect("temp dir is writable");

    let url = file_url_from_path(&page).expect("file exists");
    let body = DefaultSender.fetch(&url).expect("URL names the file just written");
    std::fs::remove_dir_all(&dir).expect("dir was just created");
    assert_eq!(body, b"<p>hi</p>");
}

/// A page's relative reference resolves against the page's directory,
/// whatever the process's working directory is.
#[test]
fn relative_reference_resolves_next_to_the_page() {
    let dir = awkward_dir("relative");
    let page = dir.join("page.html");
    std::fs::write(&page, "").expect("temp dir is writable");
    std::fs::write(dir.join("style.css"), "p { color: red }").expect("temp dir is writable");

    let base = file_url_from_path(&page).expect("file exists");
    let css = DefaultSender
        .fetch(&resolve_url("style.css", Some(&base)))
        .expect("style.css sits next to page.html");
    std::fs::remove_dir_all(&dir).expect("dir was just created");
    assert_eq!(css, b"p { color: red }");
}

/// A protocol-relative reference in a page loaded from disk resolves to
/// a `file:` URL with a host. That names no local file, and the error
/// says so rather than reading some unrelated path.
#[test]
fn file_url_with_host_is_rejected() {
    let err = DefaultSender
        .fetch("file://cdn.example/lib.js")
        .expect_err("a file URL with a host is not local");
    assert!(matches!(err, FetchError::InvalidFileUrl { .. }), "got {err}");
}
