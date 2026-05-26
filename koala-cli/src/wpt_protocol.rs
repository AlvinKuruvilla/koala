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
use koala_browser::{FontProvider, load_document};
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
    /// Exit the loop cleanly.
    Shutdown,
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
    use super::{Command, Event};

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
