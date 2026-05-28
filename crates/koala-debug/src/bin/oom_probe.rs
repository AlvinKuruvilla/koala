//! Per-stage allocation probe for `koala_browser::load_document`.
//!
//! Installs a `tracing` `Layer` that, on every span enter and
//! exit, reports current peak resident-set size (via
//! `getrusage`) and the delta since the previous event. The
//! result is a single time-ordered log of which pipeline stage
//! added how much memory — anything growing by GB between
//! enter and exit is the culprit. Originally built to localize
//! the overleaf for-in OOM (since fixed by the Boa bump); kept
//! as the standing tool for any future "where is the memory
//! going?" question.
//!
//! Optional `--map URL=PATH` overrides install a
//! [`MappedSender`] so debug copies of third-party JS / CSS can
//! be served from disk while the rest of the page still loads
//! normally. Stackable.
//!
//! Run with:
//!
//! ```sh
//! just probe-oom <url>                # via the justfile recipe
//! cargo run --release --bin oom-probe -- <url>
//! cargo run --release --bin oom-probe -- --map URL=/tmp/x.js <url>
//! ```
//!
//! Output goes to stderr so a normal stdout pipeline doesn't
//! get polluted.

use std::sync::Mutex;
use std::time::Instant;

use koala_browser::{load_document, warning};
use koala_browser::net::{DefaultSender, MappedSender, install_sender};
use tracing::span;
use tracing_subscriber::layer::{Context as LayerContext, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::{LookupSpan, Registry};

/// Current resident-set size in bytes via `getrusage(RUSAGE_SELF)`.
///
/// On Darwin `ru_maxrss` is the peak resident size in bytes; on
/// Linux it is in kilobytes. Both are monotonic for our purposes
/// (we want to see when the high-water mark jumps), but we
/// normalize to bytes for printing.
#[allow(unsafe_code)]
fn peak_rss_bytes() -> u64 {
    // SAFETY: getrusage is a plain syscall wrapper; passing
    // RUSAGE_SELF and a properly-typed `rusage` struct out-pointer
    // is the documented contract.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &raw mut ru) };
    if rc != 0 {
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        ru.ru_maxrss as u64
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux: ru_maxrss is KB.
        (ru.ru_maxrss as u64).saturating_mul(1024)
    }
}

fn format_mb(bytes: u64) -> String {
    format!("{:>8.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// State shared by enter/exit reporting. Kept inside a `Mutex` so
/// we don't need to thread anything through the `Layer` impl.
struct ProbeState {
    started_at: Instant,
    last_rss: u64,
}

static STATE: Mutex<Option<ProbeState>> = Mutex::new(None);

fn report(kind: &str, name: &str) {
    let rss = peak_rss_bytes();
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(|| ProbeState {
        started_at: Instant::now(),
        last_rss: rss,
    });
    let delta_signed = rss as i64 - st.last_rss as i64;
    let delta_str = if delta_signed >= 0 {
        format!("+{}", format_mb(delta_signed as u64))
    } else {
        format!("-{}", format_mb((-delta_signed) as u64))
    };
    let elapsed_ms = st.started_at.elapsed().as_millis();
    eprintln!(
        "[{elapsed_ms:>6} ms] {kind:<5} {name:<24} rss={} Δ={}",
        format_mb(rss),
        delta_str
    );
    st.last_rss = rss;
}

struct ProbeLayer;

impl<S> Layer<S> for ProbeLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_enter(&self, id: &span::Id, ctx: LayerContext<'_, S>) {
        if let Some(span) = ctx.span(id) {
            report("enter", span.name());
        }
    }

    fn on_close(&self, id: span::Id, ctx: LayerContext<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            report("close", span.name());
        }
    }
}

fn install_subscriber() {
    let subscriber = Registry::default().with(ProbeLayer);
    tracing::subscriber::set_global_default(subscriber)
        .expect("global tracing subscriber should not already be set");
}

fn main() {
    // Tiny CLI: zero or more `--map URL=PATH` pairs followed by the
    // page URL. Mappings stack onto a `MappedSender` that overlays
    // [`DefaultSender`], so the loader fetches override targets
    // from disk and everything else from the real network.
    let mut mappings: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut page_url: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--map" {
            let pair = args.next().unwrap_or_else(|| {
                eprintln!("--map needs URL=PATH");
                std::process::exit(2);
            });
            let (url, path) = pair.split_once('=').unwrap_or_else(|| {
                eprintln!("--map expects URL=PATH, got {pair:?}");
                std::process::exit(2);
            });
            mappings.push((url.to_string(), path.into()));
        } else {
            page_url = Some(a);
        }
    }
    let Some(url) = page_url else {
        eprintln!(
            "usage: oom-probe [--map URL=PATH …] <url-or-file-path>\n  --map swaps a fetched URL with a local file; useful for instrumented copies of third-party JS"
        );
        std::process::exit(2);
    };

    warning::set_quiet(true);
    install_subscriber();

    let _sender_guard = if mappings.is_empty() {
        None
    } else {
        let mut sender = MappedSender::new(DefaultSender);
        for (url, path) in &mappings {
            eprintln!("[boa map ] {url} -> {}", path.display());
            sender = sender.map(url.clone(), path.clone());
        }
        Some(install_sender(Box::new(sender)))
    };

    eprintln!("[  0 ms] start loading {url}");
    let result = load_document(&url);
    let final_rss = peak_rss_bytes();
    eprintln!("[final ] load_document returned, rss={}", format_mb(final_rss));

    match result {
        Ok(doc) => {
            eprintln!(
                "[final ] doc: dom_nodes? html_len={} parse_issues={}",
                doc.html_source.len(),
                doc.parse_issues.len()
            );
        }
        Err(e) => eprintln!("[final ] load_document error: {e}"),
    }
}
