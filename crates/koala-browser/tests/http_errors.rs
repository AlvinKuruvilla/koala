//! Documents and subresources that come back with an HTTP error status.
//!
//! A fake server stands in for the network: each URL maps to a status and
//! a body, so the tests control exactly what an error response contains.

use koala_browser::load_document;
use koala_browser::net::{FetchCause, FetchError, RequestSender, install_sender};

struct FakeServer(Vec<(&'static str, u16, &'static str)>);

impl RequestSender for FakeServer {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let (_, status, body) = self
            .0
            .iter()
            .find(|(u, _, _)| *u == url)
            .unwrap_or_else(|| panic!("test requested an unexpected URL {url}"));
        if *status == 200 {
            Ok(body.as_bytes().to_vec())
        } else {
            Err(FetchError::HttpStatus {
                url: url.to_string(),
                status: *status,
                body: body.as_bytes().to_vec(),
            })
        }
    }
}

/// A 404 that comes with the server's own page shows that page, as
/// browsers do.
#[test]
fn error_status_with_a_body_shows_the_servers_page() {
    let _guard = install_sender(Box::new(FakeServer(vec![(
        "https://a.test/missing",
        404,
        "<html><body><h1>Nothing here</h1></body></html>",
    )])));
    let doc = load_document("https://a.test/missing").expect("the server sent a page");
    assert!(doc.html_source.contains("Nothing here"));
}

/// With nothing to show, the load fails, and the caller shows its own
/// error page for the status.
#[test]
fn error_status_with_an_empty_body_is_an_error() {
    let _guard = install_sender(Box::new(FakeServer(vec![(
        "https://a.test/empty",
        500,
        "",
    )])));
    let Err(koala_browser::LoadError::Fetch(fetch)) = load_document("https://a.test/empty") else {
        panic!("an empty error response should fail the load");
    };
    assert_eq!(fetch.cause(), FetchCause::HttpStatus(500));
}

/// Only the document uses an error body. A stylesheet that comes back as
/// a 404 is not applied, even when the error page happens to parse as CSS.
#[test]
fn stylesheet_error_body_is_not_applied() {
    let _guard = install_sender(Box::new(FakeServer(vec![
        (
            "https://a.test/",
            200,
            r#"<html><head><link rel="stylesheet" href="/gone.css"></head><body><p>x</p></body></html>"#,
        ),
        ("https://a.test/gone.css", 404, "p { color: red }"),
    ])));
    let doc = load_document("https://a.test/").expect("the page itself loads");
    assert!(
        !doc.css_text.contains("color: red"),
        "a 404 stylesheet body was applied: {}",
        doc.css_text
    );
}
