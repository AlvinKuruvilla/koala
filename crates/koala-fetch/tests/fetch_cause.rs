//! Classifying fetch failures for the error page.
//!
//! The network cases use addresses that fail locally and deterministically:
//! a `.invalid` host never resolves (RFC 6761 § 6.4), and nothing listens on
//! a port the test itself just released.

use std::net::TcpListener;

use koala_fetch::{DefaultSender, Destination, FetchCause, FetchError, Request, RequestSender};

/// Fetch `url` as a document through the production sender.
fn fetch(url: &str) -> Result<Vec<u8>, FetchError> {
    DefaultSender::default().fetch(&Request::new(url, Destination::Document))
}

fn cause_of(url: &str) -> FetchCause {
    fetch(url).expect_err("the fetch is meant to fail").cause()
}

#[test]
fn unresolvable_host_is_name_not_resolved() {
    // Pins hyper-util's "dns error" message, which `cause` matches.
    assert_eq!(cause_of("http://koala-test.invalid/"), FetchCause::NameNotResolved);
}

#[test]
fn closed_port_is_connection_refused() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("can bind a local port");
        listener.local_addr().expect("bound").port()
        // Dropping the listener closes the port.
    };
    assert_eq!(
        cause_of(&format!("http://127.0.0.1:{port}/")),
        FetchCause::ConnectionRefused
    );
}

#[test]
fn missing_file_is_file_not_found() {
    assert_eq!(cause_of("/definitely/not/a/koala/file.html"), FetchCause::FileNotFound);
}

#[test]
fn mistyped_scheme_is_unsupported_scheme() {
    assert_eq!(
        cause_of("htpps://www.google.com"),
        FetchCause::UnsupportedScheme("htpps".to_string())
    );
}

#[test]
fn malformed_data_url_is_other() {
    assert_eq!(cause_of("data:no-comma"), FetchCause::Other);
}
