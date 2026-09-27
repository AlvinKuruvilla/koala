//! Record/replay through the archive senders.
//!
//! The inner sender is a fixed URL → response table, so every test runs
//! without the network and knows exactly what "the page" returned.

use std::collections::HashMap;
use std::path::PathBuf;

use super::{Archive, ArchiveError, RecordingSender, ReplaySender};
use koala_fetch::{Destination, FetchError, Request, RequestSender, fetch_bytes, install_sender};

/// Fetch `url` as a document through `sender`.
fn get(sender: &impl RequestSender, url: &str) -> Result<Vec<u8>, FetchError> {
    sender.fetch(&Request::new(url, Destination::Document))
}

/// Stand-in for the network: known URLs return their body, a host named
/// `down.example` cannot be reached, and anything else is a 404 with the
/// server's error page.
struct FakeSite(HashMap<&'static str, &'static [u8]>);

const NOT_FOUND_PAGE: &[u8] = b"<h1>Not Found</h1>";

impl RequestSender for FakeSite {
    fn fetch(&self, request: &Request<'_>) -> Result<Vec<u8>, FetchError> {
        let url = request.url;
        if url.contains("down.example") {
            // Any non-HTTP failure will do; the archive keeps only its message.
            return Err(FetchError::InvalidFileUrl { url: url.to_string() });
        }
        self.0.get(url).map(|body| body.to_vec()).ok_or_else(|| FetchError::HttpStatus {
            url: url.to_string(),
            status: 404,
            body: NOT_FOUND_PAGE.to_vec(),
        })
    }
}

fn site() -> FakeSite {
    FakeSite(HashMap::from([
        ("https://example.com/", &b"<link rel=stylesheet href=a.css>"[..]),
        ("https://example.com/a.css", &b"body { color: red }"[..]),
        // Not valid UTF-8, as image bodies generally are not.
        ("https://example.com/logo.png", &[0x89, b'P', b'N', b'G', 0xff, 0x00][..]),
    ]))
}

/// A path under the system temp dir, unique per test so tests can run in
/// parallel.
fn temp_archive(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("koala-archive-{}-{name}.json", std::process::id()))
}

/// Fetch `urls` through `sender` and return each outcome as a string, so
/// two runs can be compared with one `assert_eq!`.
fn fetch_all(sender: Box<dyn RequestSender>, urls: &[&str]) -> Vec<String> {
    let _guard = install_sender(sender);
    urls.iter()
        .map(|url| match fetch_bytes(&Request::new(url, Destination::Document)) {
            Ok(body) => format!("ok {body:?}"),
            Err(e) => format!("err {e}"),
        })
        .collect()
}

/// Record a load, then replay it from the saved file: every fetch returns
/// what it returned while recording, including an HTTP error and a failure.
#[test]
fn replay_reproduces_recorded_load() {
    let urls = [
        "https://example.com/",
        "https://example.com/a.css",
        "https://example.com/logo.png",
        "https://example.com/missing.js",
        "https://down.example/x.js",
    ];
    let recorder = RecordingSender::new(site());
    let live = fetch_all(Box::new(recorder.clone()), &urls);

    let path = temp_archive("round-trip");
    recorder.archive().save(&path).expect("temp dir is writable");
    let archive = Archive::load(&path).expect("archive was just saved");
    std::fs::remove_file(&path).expect("archive was just saved");
    assert_eq!(archive.len(), 5, "the 404 and the failed fetch are recorded too");

    let replay = ReplaySender::new(archive);
    let replayed = fetch_all(Box::new(replay), &urls);
    // Successes and the 404 come back exactly as they were.
    assert_eq!(replayed[..4], live[..4]);
    // A failure without a response comes back as a recorded failure that
    // quotes the original.
    assert!(
        replayed[4].contains("failed when the archive was recorded"),
        "got {}",
        replayed[4]
    );
}

/// The server's error page survives recording, so replaying a page that
/// shows a 404 page still shows it.
#[test]
fn replay_keeps_the_body_of_an_http_error() {
    let recorder = RecordingSender::new(site());
    let _ = get(&recorder, "https://example.com/missing");
    let path = temp_archive("http-error");
    recorder.archive().save(&path).expect("temp dir is writable");
    let archive = Archive::load(&path).expect("archive was just saved");
    std::fs::remove_file(&path).expect("archive was just saved");

    let err = get(&ReplaySender::new(archive), "https://example.com/missing")
        .expect_err("the page was a 404");
    match err {
        FetchError::HttpStatus { status, body, .. } => {
            assert_eq!(status, 404);
            assert_eq!(body, NOT_FOUND_PAGE);
        }
        other => panic!("expected HttpStatus, got {other}"),
    }
}

/// Archives written before `status` existed still load.
#[test]
fn load_accepts_version_1() {
    let path = temp_archive("version-1");
    std::fs::write(
        &path,
        r#"{"format":"koala-fetch-archive","version":1,"entries":{
            "https://example.com/":{"body":"aGk=","sha256":"8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4"}}}"#,
    )
    .expect("temp dir is writable");
    let archive = Archive::load(&path).expect("version 1 is still readable");
    std::fs::remove_file(&path).expect("file was just written");
    assert_eq!(archive.len(), 1);
}

/// A URL the recording never saw is an error, not a network fetch.
#[test]
fn replay_miss_is_an_error() {
    let replay = ReplaySender::new(Archive::new());
    let err = get(&replay, "https://example.com/new.js").expect_err("archive is empty");
    assert!(matches!(err, FetchError::NotInArchive { .. }), "got {err}");
}

/// `data:` URLs decode directly and never enter the archive, so replay
/// serves them without an entry.
#[test]
fn data_urls_bypass_the_archive() {
    let recorder = RecordingSender::new(site());
    assert_eq!(get(&recorder, "data:,hi").expect("valid data URL"), b"hi");
    assert_eq!(recorder.archive().len(), 0);

    let replay = ReplaySender::new(Archive::new());
    assert_eq!(get(&replay, "data:,hi").expect("valid data URL"), b"hi");
}

/// Record every URL of `site()` and return the resulting archive.
fn recorded_site() -> Archive {
    let recorder = RecordingSender::new(site());
    for url in ["https://example.com/", "https://example.com/a.css"] {
        let _ = get(&recorder, url);
    }
    let path = temp_archive(&format!("digest-{}", rand_suffix()));
    recorder.archive().save(&path).expect("temp dir is writable");
    let archive = Archive::load(&path).expect("archive was just saved");
    std::fs::remove_file(&path).expect("archive was just saved");
    archive
}

/// Distinguishes temp files created by one test calling `recorded_site`
/// more than once.
fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The digest depends on what was served, not on fetch order or on
/// archive entries the load never used.
#[test]
fn input_digest_tracks_served_inputs() {
    let forward = ReplaySender::new(recorded_site());
    let _ = get(&forward, "https://example.com/");
    let _ = get(&forward, "https://example.com/a.css");

    let backward = ReplaySender::new(recorded_site());
    let _ = get(&backward, "https://example.com/a.css");
    let _ = get(&backward, "https://example.com/");
    assert_eq!(forward.input_digest(), backward.input_digest());

    // Serving less changes the digest.
    let partial = ReplaySender::new(recorded_site());
    let _ = get(&partial, "https://example.com/");
    assert_ne!(forward.input_digest(), partial.input_digest());

    // A miss is an input too: a build that requests something new must
    // not look like it consumed the same inputs.
    let _ = get(&backward, "https://example.com/new.js");
    assert_ne!(forward.input_digest(), backward.input_digest());
}

/// A body that no longer matches its recorded hash is rejected on load.
#[test]
fn load_rejects_tampered_body() {
    let path = temp_archive("tampered");
    std::fs::write(
        &path,
        r#"{"format":"koala-fetch-archive","version":1,"entries":{
            "https://example.com/":{"body":"aGVsbG8=","sha256":"0000"}}}"#,
    )
    .expect("temp dir is writable");
    let err = Archive::load(&path).expect_err("hash does not match body");
    std::fs::remove_file(&path).expect("file was just written");
    assert!(matches!(err, ArchiveError::Corrupt { .. }), "got {err}");
}

/// Files from another format or version are refused with a message that
/// says to re-record.
#[test]
fn load_rejects_other_versions() {
    let path = temp_archive("version");
    std::fs::write(&path, r#"{"format":"koala-fetch-archive","version":99,"entries":{}}"#)
        .expect("temp dir is writable");
    let err = Archive::load(&path).expect_err("version 99 is unsupported");
    std::fs::remove_file(&path).expect("file was just written");
    assert!(matches!(err, ArchiveError::Unsupported { version: 99, .. }), "got {err}");
    assert!(err.to_string().contains("re-record"), "got {err}");
}
