//! Record/replay through `koala_common::archive`.
//!
//! The inner sender is a fixed URL → response table, so every test runs
//! without the network and knows exactly what "the page" returned.

use std::collections::HashMap;
use std::path::PathBuf;

use koala_common::archive::{Archive, ArchiveError, RecordingSender, ReplaySender};
use koala_common::net::{FetchError, RequestSender, fetch_bytes, install_sender};

/// Stand-in for the network: known URLs return their body, anything else
/// fails with a 404.
struct FakeSite(HashMap<&'static str, &'static [u8]>);

impl RequestSender for FakeSite {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        self.0.get(url).map(|body| body.to_vec()).ok_or_else(|| FetchError::HttpStatus {
            url: url.to_string(),
            status: 404,
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
        .map(|url| match fetch_bytes(url) {
            Ok(body) => format!("ok {body:?}"),
            Err(e) => format!("err {e}"),
        })
        .collect()
}

/// Record a load, then replay it from the saved file: every fetch returns
/// what it returned while recording, including the failure.
#[test]
fn replay_reproduces_recorded_load() {
    let urls = [
        "https://example.com/",
        "https://example.com/a.css",
        "https://example.com/logo.png",
        "https://example.com/missing.js",
    ];
    let recorder = RecordingSender::new(site());
    let live = fetch_all(Box::new(recorder.clone()), &urls);

    let path = temp_archive("round-trip");
    recorder.archive().save(&path).expect("temp dir is writable");
    let archive = Archive::load(&path).expect("archive was just saved");
    std::fs::remove_file(&path).expect("archive was just saved");
    assert_eq!(archive.len(), 4, "the failed fetch is recorded too");

    let replayed = fetch_all(Box::new(ReplaySender::new(archive)), &urls);
    assert_eq!(replayed[..3], live[..3]);
    // The 404 comes back as a recorded failure that quotes the original.
    assert!(
        replayed[3].contains("failed when the archive was recorded")
            && replayed[3].contains("HTTP 404"),
        "got {}",
        replayed[3]
    );
}

/// A URL the recording never saw is an error, not a network fetch.
#[test]
fn replay_miss_is_an_error() {
    let replay = ReplaySender::new(Archive::new());
    let err = replay.fetch("https://example.com/new.js").expect_err("archive is empty");
    assert!(matches!(err, FetchError::NotInArchive { .. }), "got {err}");
}

/// `data:` URLs decode directly and never enter the archive, so replay
/// serves them without an entry.
#[test]
fn data_urls_bypass_the_archive() {
    let recorder = RecordingSender::new(site());
    assert_eq!(recorder.fetch("data:,hi").expect("valid data URL"), b"hi");
    assert!(recorder.archive().is_empty());

    let replay = ReplaySender::new(Archive::new());
    assert_eq!(replay.fetch("data:,hi").expect("valid data URL"), b"hi");
}

/// Record every URL of `site()` and return the resulting archive.
fn recorded_site() -> Archive {
    let recorder = RecordingSender::new(site());
    for url in ["https://example.com/", "https://example.com/a.css"] {
        let _ = recorder.fetch(url);
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
    let _ = forward.fetch("https://example.com/");
    let _ = forward.fetch("https://example.com/a.css");

    let backward = ReplaySender::new(recorded_site());
    let _ = backward.fetch("https://example.com/a.css");
    let _ = backward.fetch("https://example.com/");
    assert_eq!(forward.input_digest(), backward.input_digest());

    // Serving less changes the digest.
    let partial = ReplaySender::new(recorded_site());
    let _ = partial.fetch("https://example.com/");
    assert_ne!(forward.input_digest(), partial.input_digest());

    // A miss is an input too: a build that requests something new must
    // not look like it consumed the same inputs.
    let _ = backward.fetch("https://example.com/new.js");
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
