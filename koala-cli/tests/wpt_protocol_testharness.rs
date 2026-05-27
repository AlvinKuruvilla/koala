//! End-to-end integration test for the koala-cli WPT protocol's
//! `testharness` command (Phase 5 chunk 3).
//!
//! Spawns the real `koala-cli --wpt-protocol` binary as a
//! subprocess, sends a `testharness` command pointing at a
//! local HTML fixture whose inline script calls
//! `__koala_emit_result__` directly, and asserts the
//! corresponding `testharness_complete` event carries the
//! expected results. Bypasses real testharness.js entirely —
//! that integration belongs at the wptrunner layer, not here.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Locate the freshly-built `koala` binary under `target/`. Cargo
/// sets `CARGO_BIN_EXE_<name>` for any binary the test crate
/// declares as a dependency through its package, which we can't
/// rely on across workspace crates, so fall back to deriving it
/// from `CARGO_MANIFEST_DIR`. The integration test runs after
/// `cargo build`, so the binary is guaranteed to exist.
fn koala_binary() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_koala") {
        return PathBuf::from(path);
    }
    let manifest = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest)
        .parent()
        .expect("koala-cli manifest dir has a parent")
        .join("target")
        .join("debug")
        .join("koala")
}

/// Build a unique fixture path under the system temp dir.
fn fixture_path(suffix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "koala-wpt-testharness-{}-{suffix}.html",
        std::process::id(),
    ));
    p
}

#[test]
fn testharness_command_round_trips_emitted_results() {
    // Write a fixture that calls __koala_emit_result__ directly
    // (no real testharness.js — that path is tested at the
    // wptrunner integration layer).
    let html = r#"<!DOCTYPE html>
        <html><body>
          <script>
            __koala_emit_result__({ name: 'check one', status: 0 });
            __koala_emit_result__({ name: 'check two', status: 1, message: 'fail msg' });
            __koala_emit_completion__([], { status: 0, message: '' });
          </script>
        </body></html>"#;
    let fixture = fixture_path("results");
    std::fs::write(&fixture, html).expect("write fixture");

    let binary = koala_binary();
    assert!(
        binary.exists(),
        "koala binary not built: {}. Run `cargo build` first.",
        binary.display(),
    );

    let mut child = Command::new(&binary)
        .arg("--wpt-protocol")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn koala-cli --wpt-protocol");

    let mut stdin = child.stdin.take().expect("take stdin");
    let stdout = child.stdout.take().expect("take stdout");
    let mut reader = BufReader::new(stdout);

    // Read the `ready` event before sending commands.
    let mut line = String::new();
    let _ = reader.read_line(&mut line).expect("read ready event");
    assert!(line.contains("\"event\":\"ready\""), "expected ready, got: {line}");

    // Issue the testharness command.
    let cmd = format!(
        r#"{{"cmd":"testharness","url":"{}"}}"#,
        fixture.to_string_lossy(),
    );
    writeln!(stdin, "{cmd}").expect("write command");

    // Read the response event.
    line.clear();
    let _ = reader
        .read_line(&mut line)
        .expect("read testharness_complete event");

    // Shut down cleanly.
    writeln!(stdin, r#"{{"cmd":"shutdown"}}"#).expect("write shutdown");
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&fixture);

    let event: serde_json::Value =
        serde_json::from_str(line.trim()).expect("parse event JSON");
    assert_eq!(
        event["event"].as_str(),
        Some("testharness_complete"),
        "expected testharness_complete, got: {event}",
    );

    let results = event["results"].as_array().expect("results array");
    assert_eq!(results.len(), 2, "expected 2 captured results: {event}");
    assert_eq!(results[0]["name"].as_str(), Some("check one"));
    assert_eq!(results[0]["status"].as_i64(), Some(0));
    assert_eq!(results[1]["name"].as_str(), Some("check two"));
    assert_eq!(results[1]["status"].as_i64(), Some(1));
    assert_eq!(results[1]["message"].as_str(), Some("fail msg"));

    let completion = event["completion"]
        .as_object()
        .expect("completion object");
    assert_eq!(completion["status"].as_i64(), Some(0));
}

#[test]
fn subprocess_survives_failed_test_and_serves_subsequent_command() {
    // wptrunner runs every test in a directory through a single
    // koala-cli subprocess. If one test fails — whether from a
    // bogus URL, a JS error, or a Rust-side panic — the
    // subprocess MUST stay alive so the next test in the batch
    // can be served. Otherwise a single bad test cascades into
    // every subsequent test in the batch reporting CRASH.
    //
    // This test exercises the command-loop-continues path with
    // a non-existent URL (the load_document call returns
    // `Err`, which the protocol surfaces as `load_failed`).
    // The matching catch_unwind path for actual Rust panics is
    // covered by the panic_message unit tests; both paths share
    // the same "log + emit + continue" structure in `run`.
    let good_html = r#"<!DOCTYPE html>
        <html><body>
          <script>
            __koala_emit_result__({ name: 'second-command-ran', status: 0 });
          </script>
        </body></html>"#;
    let good_fixture = fixture_path("recovery-good");
    std::fs::write(&good_fixture, good_html).expect("write good fixture");

    let binary = koala_binary();
    let mut child = Command::new(&binary)
        .arg("--wpt-protocol")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn koala-cli --wpt-protocol");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let _ = reader.read_line(&mut line).unwrap(); // ready

    // First command: a path that doesn't exist. Should come back
    // as load_failed without taking the subprocess down.
    let bogus = fixture_path("recovery-bogus-DOES-NOT-EXIST");
    let bad_cmd = format!(
        r#"{{"cmd":"testharness","url":"{}"}}"#,
        bogus.to_string_lossy(),
    );
    writeln!(stdin, "{bad_cmd}").expect("write bad command");
    line.clear();
    let _ = reader.read_line(&mut line).expect("read load_failed event");
    let first: serde_json::Value =
        serde_json::from_str(line.trim()).expect("parse first event");
    assert_eq!(
        first["event"].as_str(),
        Some("load_failed"),
        "expected first response to be load_failed, got: {first}",
    );

    // Second command: a known-good fixture. Subprocess must
    // still be alive and serving.
    let good_cmd = format!(
        r#"{{"cmd":"testharness","url":"{}"}}"#,
        good_fixture.to_string_lossy(),
    );
    writeln!(stdin, "{good_cmd}").expect("write good command");
    line.clear();
    let _ = reader.read_line(&mut line).expect("read testharness_complete event");
    let second: serde_json::Value =
        serde_json::from_str(line.trim()).expect("parse second event");

    writeln!(stdin, r#"{{"cmd":"shutdown"}}"#).unwrap();
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&good_fixture);

    assert_eq!(
        second["event"].as_str(),
        Some("testharness_complete"),
        "expected subprocess to survive and complete second command, got: {second}",
    );
    let results = second["results"].as_array().expect("results array");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"].as_str(), Some("second-command-ran"));
}

#[test]
fn testharness_command_with_no_completion_returns_null_completion() {
    // A document that emits results but never fires the harness
    // completion callback. The frame should still come through
    // with `completion: null`.
    let html = r#"<!DOCTYPE html>
        <html><body>
          <script>
            __koala_emit_result__({ name: 'no completion', status: 0 });
          </script>
        </body></html>"#;
    let fixture = fixture_path("no-completion");
    std::fs::write(&fixture, html).expect("write fixture");

    let binary = koala_binary();
    let mut child = Command::new(&binary)
        .arg("--wpt-protocol")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn koala-cli --wpt-protocol");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let _ = reader.read_line(&mut line).unwrap(); // ready

    let cmd = format!(
        r#"{{"cmd":"testharness","url":"{}"}}"#,
        fixture.to_string_lossy(),
    );
    writeln!(stdin, "{cmd}").unwrap();

    line.clear();
    let _ = reader.read_line(&mut line).unwrap();

    writeln!(stdin, r#"{{"cmd":"shutdown"}}"#).unwrap();
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&fixture);

    let event: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(event["event"].as_str(), Some("testharness_complete"));
    assert_eq!(event["results"].as_array().unwrap().len(), 1);
    assert!(
        event["completion"].is_null(),
        "expected completion: null, got: {}",
        event["completion"],
    );
}
