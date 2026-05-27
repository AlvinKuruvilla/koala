//! WPT protocol mode for `koala-cli`.
//!
//! When invoked with `--wpt-protocol`, koala-cli reads JSON-line
//! commands from stdin and emits JSON-line events on stdout. The
//! protocol is consumed by the wptrunner browser + executor plugin
//! under `wpt-tools/wptrunner-plugins/` to drive koala as a
//! subprocess under upstream WPT.
//!
//! Diagnostics MUST be written to stderr; stdout is reserved
//! exclusively for protocol events, one JSON object per line. A
//! single stray `println!` anywhere in the rendering stack will
//! corrupt the protocol and break the wptrunner handshake.
//!
//! See `project-memory/wpt-integration-spec.md` § "Phase 1.1
//! `koala-cli --wpt-protocol` mode" for the canonical schema.

use anyhow::{Context, Result};
use koala_browser::{FontProvider, JsHooks, load_document, load_document_with_hooks};
use koala_browser::js::JsRuntime;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::render::render_document_to_path;

/// WPT reftests render at 800×600 by default. Tests that need a
/// different viewport set it via `<meta name="viewport">` or via
/// the manifest, in which case wptrunner will pass it through as
/// the `viewport` field of the render command.
const DEFAULT_VIEWPORT_WIDTH: u32 = 800;
const DEFAULT_VIEWPORT_HEIGHT: u32 = 600;

/// One command line read from stdin.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Command {
    /// Load a URL, render it, and emit the screenshot path.
    Render {
        /// Absolute URL or local file path to load.
        url: String,
        /// Optional override of `[width, height]` in CSS pixels.
        #[serde(default)]
        viewport: Option<[u32; 2]>,
    },
    /// Load a URL, drive its scripts through the koala-wpt
    /// testharness bridge, and emit the captured per-test
    /// results plus the harness-level completion payload.
    Testharness {
        /// Absolute URL or local file path to load.
        url: String,
    },
    /// Exit the loop cleanly.
    Shutdown,
}

/// A single test result frame emitted as part of
/// [`Event::TestharnessComplete`]. Mirrors
/// [`koala_wpt::TestharnessResult`] but flattened into a serde
/// type so the JSON-line protocol stays decoupled from the
/// underlying Rust struct.
#[derive(Debug, Serialize)]
struct TestharnessResultPayload {
    /// Test name (the string passed to `test()` / `async_test()`).
    name: String,
    /// Numeric WPT status code: 0 = PASS, 1 = FAIL, 2 = TIMEOUT,
    /// 3 = NOTRUN, 4 = PRECONDITION_FAILED.
    status: u32,
    /// Assertion failure detail or empty when the test passed.
    message: String,
    /// JS stack at the failure point, when available.
    stack: String,
}

/// Harness-level completion payload mirroring
/// [`koala_wpt::TestharnessCompletion`]. `None` when the harness
/// never reached its completion callback (i.e. the document
/// loaded but didn't run testharness.js).
#[derive(Debug, Serialize)]
struct TestharnessCompletionPayload {
    /// Numeric harness status: 0 = OK, 1 = ERROR, 2 = TIMEOUT,
    /// 3 = PRECONDITION_FAILED.
    status: u32,
    /// Diagnostic message; empty in the clean OK case.
    message: String,
}

/// One event written to stdout.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event {
    /// Sent once on startup before any commands are read.
    Ready,
    /// Render completed; the PNG is on disk at `screenshot`.
    Rendered {
        /// Echo of the requested URL so wptrunner can correlate.
        url: String,
        /// Absolute path to the PNG. Caller owns cleanup.
        screenshot: String,
    },
    /// The document could not be loaded or rendered. The loop
    /// continues to read further commands.
    LoadFailed {
        /// Echo of the requested URL so wptrunner can correlate.
        url: String,
        /// Human-readable error chain (anyhow `{:#}` formatting).
        error: String,
    },
    /// A testharness run finished — the document loaded and its
    /// scripts (including testharness.js if present) ran to
    /// completion. Emitted once per `testharness` command.
    TestharnessComplete {
        /// Echo of the requested URL so wptrunner can correlate.
        url: String,
        /// Captured per-test results in emission order.
        results: Vec<TestharnessResultPayload>,
        /// Harness-level completion payload, `None` when the
        /// harness completion callback never fired.
        completion: Option<TestharnessCompletionPayload>,
    },
    /// A malformed command was received. The loop continues to
    /// read further commands.
    ProtocolError {
        /// Why the command could not be parsed.
        message: String,
    },
}

/// Run the WPT protocol loop until stdin closes or a `shutdown`
/// command is received.
///
/// # Errors
///
/// Returns an error only when stdin/stdout I/O fails fatally.
/// Document load, parse, and render failures are reported as
/// [`Event::LoadFailed`] and do not abort the loop.
pub(crate) fn run() -> Result<()> {
    // Silence engine-internal diagnostics ("Loaded regular font: …",
    // image-load warnings, CSS warn_once feature notices) so stderr
    // stays empty per test. wptrunner already captures real failures
    // via the protocol; the noise here just slows large batches.
    koala_browser::warning::set_quiet(true);

    let font_provider = FontProvider::load();
    let mut counter: u64 = 0;

    emit(&Event::Ready)?;

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .context("while attempting to read command from stdin")?;
        if bytes == 0 {
            // EOF — wptrunner closed stdin. Exit cleanly.
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let command: Command = match serde_json::from_str(trimmed) {
            Ok(c) => c,
            Err(e) => {
                emit(&Event::ProtocolError {
                    message: format!("invalid command JSON: {e}"),
                })?;
                continue;
            }
        };

        match command {
            Command::Testharness { url } => {
                let event = match run_testharness(&url) {
                    Ok((results, completion)) => Event::TestharnessComplete {
                        url,
                        results,
                        completion,
                    },
                    Err(e) => Event::LoadFailed {
                        url,
                        error: format!("{e:#}"),
                    },
                };
                emit(&event)?;
            }
            Command::Render { url, viewport } => {
                counter = counter.saturating_add(1);
                let path = screenshot_path(counter);
                let [w, h] =
                    viewport.unwrap_or([DEFAULT_VIEWPORT_WIDTH, DEFAULT_VIEWPORT_HEIGHT]);

                let event = match render_url(&url, &path, w, h, &font_provider) {
                    Ok(()) => Event::Rendered {
                        url,
                        screenshot: path.to_string_lossy().into_owned(),
                    },
                    Err(e) => Event::LoadFailed {
                        url,
                        error: format!("{e:#}"),
                    },
                };
                emit(&event)?;
            }
            Command::Shutdown => return Ok(()),
        }
    }
}

/// Build a unique screenshot path. Composed of pid + monotonic
/// counter so concurrent koala-cli processes (e.g. wptrunner
/// chunked runs) don't collide on the same temp file.
fn screenshot_path(counter: u64) -> PathBuf {
    let pid = std::process::id();
    std::env::temp_dir().join(format!("koala-wpt-{pid}-{counter}.png"))
}

/// Load a document by URL or local path and render it.
fn render_url(
    url: &str,
    output_path: &Path,
    width: u32,
    height: u32,
    font_provider: &FontProvider,
) -> Result<()> {
    let doc = load_document(url).context("while attempting to load document")?;
    render_document_to_path(&doc, output_path, width, height, font_provider)
}

/// Load `url`, run its scripts through the koala-wpt testharness
/// bridge, and return the captured results.
///
/// Returns `(results, completion)` where `results` is the list
/// of per-test outcomes in emission order and `completion` is
/// the harness-level completion payload (or `None` when the
/// harness never reached its completion callback).
fn run_testharness(
    url: &str,
) -> Result<(Vec<TestharnessResultPayload>, Option<TestharnessCompletionPayload>)> {
    let mut hook = TestharnessHook::default();
    let doc = load_document_with_hooks(url, &mut hook)
        .context("while attempting to load testharness document")?;
    // Surface JS parse / runtime errors so debugging real
    // testharness.js failures isn't blind. stderr is fine —
    // wptrunner captures it via KoalaErrorsPart and threads
    // it into the wptrunner log on EXTERNAL-TIMEOUT / ERROR.
    for issue in &doc.parse_issues {
        eprintln!("[koala-wpt] parse_issue: {issue}");
    }
    Ok((hook.results, hook.completion))
}

/// JS-runtime hook that installs the koala-wpt testharness
/// bridge before any document script runs and drains the
/// captured buffers once the post-`load` pump returns. Lives in
/// koala-cli rather than koala-wpt itself because the hook trait
/// is a koala-browser concept and would otherwise leak that
/// dependency upstream.
#[derive(Default)]
struct TestharnessHook {
    results: Vec<TestharnessResultPayload>,
    completion: Option<TestharnessCompletionPayload>,
}

impl JsHooks for TestharnessHook {
    fn before_scripts(&mut self, rt: &mut JsRuntime) {
        koala_wpt::install(rt);
    }

    fn should_stop_pumping(&mut self, rt: &mut JsRuntime) -> bool {
        // Once the harness has emitted its completion payload,
        // there's no reason for the pump to keep sleeping on
        // testharness.js's watchdog `setTimeout`. Read failures
        // (only possible if a script has clobbered the hidden
        // buffer slot) fall back to "keep pumping" so the budget
        // path still terminates the loop.
        koala_wpt::has_test_completion(rt).unwrap_or(false)
    }

    fn after_settled(&mut self, rt: &mut JsRuntime) {
        // Drain results. Errors here can only come from a
        // malicious script clobbering the hidden buffer slot,
        // which we don't try to recover from — log via stderr
        // (the protocol channel reserves stdout) and surface an
        // empty result set so the caller still gets a frame.
        match koala_wpt::take_test_results(rt) {
            Ok(results) => {
                self.results = results
                    .into_iter()
                    .map(|r| TestharnessResultPayload {
                        name: r.name,
                        status: r.status,
                        message: r.message,
                        stack: r.stack,
                    })
                    .collect();
            }
            Err(e) => {
                eprintln!(
                    "[koala-cli] testharness drain failed: {e}; continuing with empty result set"
                );
            }
        }
        match koala_wpt::take_test_completion(rt) {
            Ok(Some(c)) => {
                self.completion = Some(TestharnessCompletionPayload {
                    status: c.status,
                    message: c.message,
                });
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("[koala-cli] testharness completion drain failed: {e}");
            }
        }
    }
}

/// Serialize one event and write it as a single JSON line.
/// Flushes after every event so wptrunner sees output promptly.
fn emit(event: &Event) -> Result<()> {
    let stdout = io::stdout();
    let mut locked = stdout.lock();
    serde_json::to_writer(&mut locked, event)
        .context("while attempting to serialize protocol event")?;
    locked
        .write_all(b"\n")
        .context("while attempting to write protocol newline")?;
    locked
        .flush()
        .context("while attempting to flush protocol stdout")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Command, Event, TestharnessCompletionPayload, TestharnessResultPayload,
    };

    #[test]
    fn parses_render_command_without_viewport() {
        let cmd: Command =
            serde_json::from_str(r#"{"cmd":"render","url":"http://example.com/"}"#)
                .expect("render command should parse");
        let Command::Render { url, viewport } = cmd else {
            panic!("expected Render, got {cmd:?}");
        };
        assert_eq!(url, "http://example.com/");
        assert!(viewport.is_none());
    }

    #[test]
    fn parses_render_command_with_viewport() {
        let cmd: Command = serde_json::from_str(
            r#"{"cmd":"render","url":"http://web-platform.test:8000/css/CSS2/test.html","viewport":[1024,768]}"#,
        )
        .expect("render command with viewport should parse");
        let Command::Render { url, viewport } = cmd else {
            panic!("expected Render, got {cmd:?}");
        };
        assert_eq!(url, "http://web-platform.test:8000/css/CSS2/test.html");
        assert_eq!(viewport, Some([1024, 768]));
    }

    #[test]
    fn parses_shutdown_command() {
        let cmd: Command =
            serde_json::from_str(r#"{"cmd":"shutdown"}"#).expect("shutdown should parse");
        assert!(matches!(cmd, Command::Shutdown));
    }

    #[test]
    fn unknown_cmd_is_a_parse_error() {
        let result: Result<Command, _> = serde_json::from_str(r#"{"cmd":"unknown"}"#);
        assert!(result.is_err(), "unknown command tag should fail to parse");
    }

    #[test]
    fn serializes_ready_event() {
        let json = serde_json::to_string(&Event::Ready).expect("ready should serialize");
        assert_eq!(json, r#"{"event":"ready"}"#);
    }

    #[test]
    fn serializes_rendered_event() {
        let json = serde_json::to_string(&Event::Rendered {
            url: "http://example.com/".to_owned(),
            screenshot: "/tmp/koala-wpt-1-1.png".to_owned(),
        })
        .expect("rendered should serialize");
        assert_eq!(
            json,
            r#"{"event":"rendered","url":"http://example.com/","screenshot":"/tmp/koala-wpt-1-1.png"}"#
        );
    }

    #[test]
    fn parses_testharness_command() {
        let cmd: Command = serde_json::from_str(
            r#"{"cmd":"testharness","url":"http://web-platform.test:8000/dom/foo.html"}"#,
        )
        .expect("testharness command should parse");
        let Command::Testharness { url } = cmd else {
            panic!("expected Testharness, got {cmd:?}");
        };
        assert_eq!(url, "http://web-platform.test:8000/dom/foo.html");
    }

    #[test]
    fn serializes_testharness_complete_event_with_results() {
        let json = serde_json::to_string(&Event::TestharnessComplete {
            url: "http://example.com/t.html".to_owned(),
            results: vec![TestharnessResultPayload {
                name: "first".to_owned(),
                status: 0,
                message: String::new(),
                stack: String::new(),
            }],
            completion: Some(TestharnessCompletionPayload {
                status: 0,
                message: String::new(),
            }),
        })
        .expect("testharness_complete should serialize");
        assert_eq!(
            json,
            r#"{"event":"testharness_complete","url":"http://example.com/t.html","results":[{"name":"first","status":0,"message":"","stack":""}],"completion":{"status":0,"message":""}}"#
        );
    }

    #[test]
    fn serializes_testharness_complete_event_with_no_completion() {
        let json = serde_json::to_string(&Event::TestharnessComplete {
            url: "http://example.com/t.html".to_owned(),
            results: vec![],
            completion: None,
        })
        .expect("testharness_complete should serialize");
        assert_eq!(
            json,
            r#"{"event":"testharness_complete","url":"http://example.com/t.html","results":[],"completion":null}"#
        );
    }

    #[test]
    fn serializes_load_failed_event() {
        let json = serde_json::to_string(&Event::LoadFailed {
            url: "http://example.com/missing".to_owned(),
            error: "404 Not Found".to_owned(),
        })
        .expect("load_failed should serialize");
        assert_eq!(
            json,
            r#"{"event":"load_failed","url":"http://example.com/missing","error":"404 Not Found"}"#
        );
    }
}
