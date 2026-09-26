//! The headers `DefaultSender` sends over HTTP, checked against a local
//! server that records the request it receives.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use koala_fetch::{DefaultSender, Destination, Language, Request, RequestSender};

/// Fetch a URL on a one-shot local server as `destination` and return the
/// request headers it received, names lower-cased.
fn headers_for(destination: Destination) -> Vec<(String, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("can bind a local port");
    let url = format!("http://{}/", listener.local_addr().expect("bound"));
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the client connects");
        let mut reader = BufReader::new(stream);
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            let _ = reader.read_line(&mut line).expect("request is readable");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
            }
        }
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .expect("response is writable");
        headers
    });
    let body = DefaultSender::default()
        .fetch(&Request::new(&url, destination))
        .expect("the local server answers 200");
    assert_eq!(body, b"ok");
    server.join().expect("server thread finished")
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

/// Fetch § 4: each destination gets its `Accept` value.
#[test]
fn accept_follows_the_destination() {
    for (destination, expected) in [
        (
            Destination::Document,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
        (Destination::Style, "text/css,*/*;q=0.1"),
        (Destination::Image, "image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5"),
        (Destination::Script, "*/*"),
    ] {
        let headers = headers_for(destination);
        assert_eq!(header(&headers, "accept"), Some(expected), "{destination:?}");
    }
}

#[test]
fn accept_language_comes_from_the_senders_language() {
    let headers = headers_for(Destination::Document);
    assert_eq!(
        header(&headers, "accept-language"),
        Some(Language::EnUs.accept_language())
    );
    assert_eq!(Language::default(), Language::EnUs);
}
