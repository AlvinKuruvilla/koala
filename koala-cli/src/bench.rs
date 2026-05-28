//! Per-render timing harness for `--bench` mode.
//!
//! Loads the requested document once (timed as a coarse "setup"
//! number), warms the engine with a few discard-iterations, then
//! runs N sample iterations of [`render_document_once`]. A
//! `tracing_subscriber::Layer` installed at startup collects each
//! span's close-time elapsed into a thread-local event log; the
//! harness drains the log between samples and bins durations by
//! span name. The output is a JSON report with per-stage mean /
//! p50 / p95 / min / max, plus the setup cost.
//!
//! Only compiled when the `bench` feature is enabled. The
//! `tracing` spans themselves live in `koala-browser` and
//! `koala-cli::render` and are always emitted; without a
//! subscriber registered, dispatch is a few atomic loads and a
//! function-pointer call — so non-bench builds carry the spans
//! but pay no measurable cost.
//!
//! The subscriber is process-global. Bench mode is single-threaded
//! (load once, render N times in a loop), so the thread-local
//! event log is sufficient — no cross-thread aggregation needed.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{Context, Result};
use koala_browser::{FontProvider, load_document, warning};
use serde::Serialize;
use tracing::span;
use tracing_subscriber::layer::{Context as LayerContext, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::{LookupSpan, Registry};

use crate::render::render_document_once;

/// Run the bench harness against `url` (file path or HTTP URL).
/// Emits a single JSON document to stdout — schema is the
/// [`BenchReport`] struct below.
///
/// `iterations` is the sample count whose timings get aggregated.
/// `warmup` is the discard-iteration count run beforehand (lets
/// the OS page in glyph atlases, JIT caches warm, etc.). Setting
/// `warmup = 0` is supported but pollutes the first sample with
/// cold-cache outliers — the `just bench` default of 3 keeps the
/// noise floor below ~5 % on the landing page.
///
/// # Errors
///
/// Propagates errors from [`load_document`] and
/// [`render_document_once`]. A bench run failing partway through
/// emits no JSON.
#[allow(clippy::cast_possible_truncation)] // µs durations comfortably fit u64
pub(crate) fn run(
    url: &str,
    width: u32,
    height: u32,
    iterations: u32,
    warmup: u32,
) -> Result<()> {
    // Suppress informational stderr noise — font-load lines,
    // image-decode warnings, CSS parser warn_once messages. These
    // are useful for diagnosing real-world rendering, but during a
    // bench they pollute the report and would corrupt downstream
    // tooling that captures stderr alongside stdout.
    warning::set_quiet(true);

    install_subscriber();

    let setup_start = Instant::now();
    let doc = load_document(url).with_context(|| format!("loading {url}"))?;
    let setup_us = setup_start.elapsed().as_micros() as u64;

    // Setup spans (html_parse, css_extract, css_cascade,
    // image_loading, layout_tree_build, script_loading,
    // js_execute, possibly post_js_relayout). Aggregated by name
    // and summed because a stage like image_loading may fire once
    // overall but cover many images — the sum is what matters,
    // not per-call stats.
    let setup_events = take_events();
    let mut setup_stages: BTreeMap<String, u64> = BTreeMap::new();
    for ev in setup_events {
        *setup_stages.entry(ev.name.to_string()).or_insert(0) += ev.duration_us;
    }

    let font_provider = FontProvider::load();

    // Drain any spans from font loading so they don't pollute the
    // render samples below. In the cached-fonts path this is a
    // no-op past the first invocation, but it's a defensive drain.
    let _ = take_events();

    for _ in 0..warmup {
        let _ = render_document_once(&doc, width, height, &font_provider)?;
        let _ = take_events();
    }

    let mut per_stage: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
    for _ in 0..iterations {
        let _ = render_document_once(&doc, width, height, &font_provider)?;
        for ev in take_events() {
            per_stage.entry(ev.name).or_default().push(ev.duration_us);
        }
    }

    let render: BTreeMap<String, StageStats> = per_stage
        .into_iter()
        .map(|(name, samples)| (name.to_string(), stats(&samples)))
        .collect();

    let report = BenchReport {
        url: url.to_string(),
        viewport: Viewport { width, height },
        iterations,
        warmup,
        setup_us,
        setup_stages,
        render,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

/// One closed span: which named site, how long it took.
///
/// `name` is the static string from a `#[tracing::instrument(name = "…")]`
/// attribute or an `info_span!("…")` call. Kept as `&'static str`
/// so the per-iteration log doesn't allocate.
struct TimingEvent {
    name: &'static str,
    duration_us: u64,
}

thread_local! {
    // Drained by `take_events()` between iterations. Const init so
    // the cell doesn't allocate when bench mode never runs on a
    // given thread (defensive — bench mode is single-threaded today,
    // but the subscriber is global so spans on any thread would
    // route here).
    static EVENTS: RefCell<Vec<TimingEvent>> = const { RefCell::new(Vec::new()) };
}

fn take_events() -> Vec<TimingEvent> {
    EVENTS.with(|e| std::mem::take(&mut *e.borrow_mut()))
}

/// `tracing_subscriber::Layer` that records each span's
/// enter-to-close duration into the thread-local `EVENTS` log.
///
/// Uses the registry's span extensions to stash an `Instant` on
/// `on_enter` and read it back on `on_close`. This is the
/// canonical pattern from the `tracing-subscriber` docs and the
/// reason we depend on the `registry` feature (it provides the
/// `LookupSpan` impl + extensions storage).
///
/// `on_close` runs once per span lifecycle — after the last drop
/// of the `Span` handle. Inline `info_span!().in_scope()` and
/// `#[instrument]` both produce a single span instance per call
/// site, so the count maps directly to "this stage was timed".
struct StageRecorder;

impl<S> Layer<S> for StageRecorder
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_enter(&self, id: &span::Id, ctx: LayerContext<'_, S>) {
        if let Some(span) = ctx.span(id) {
            // Replace any prior Instant (a re-enter of the same
            // span) — only the most recent enter→close pair is
            // interesting for our per-render aggregation. In
            // practice our spans are entered exactly once each.
            let _ = span.extensions_mut().replace(Instant::now());
        }
    }

    #[allow(clippy::cast_possible_truncation)] // µs durations comfortably fit u64
    fn on_close(&self, id: span::Id, ctx: LayerContext<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        let extensions = span.extensions();
        let Some(start) = extensions.get::<Instant>() else { return };
        let duration_us = start.elapsed().as_micros() as u64;
        let name = span.metadata().name();
        EVENTS.with(|e| {
            e.borrow_mut().push(TimingEvent {
                name,
                duration_us,
            });
        });
    }
}

/// Register the global subscriber. Called once at the start of
/// `run`. Idempotent in spirit — `set_global_default` only succeeds
/// the first time, and a re-call would error if any earlier code
/// in the same process already set one. The bench binary doesn't
/// install a subscriber anywhere else, so this is safe.
fn install_subscriber() {
    let subscriber = Registry::default().with(StageRecorder);
    // `try_init` returns Err if a default is already set; we
    // ignore that case so back-to-back bench invocations in the
    // same process (tests, repeated CLI loops) don't panic.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[derive(Serialize)]
struct BenchReport {
    /// Verbatim path/URL passed on the command line. Useful when
    /// multiple report JSONs are pooled and a downstream tool
    /// needs to attribute timings.
    url: String,
    viewport: Viewport,
    iterations: u32,
    warmup: u32,
    /// Wall-clock cost of one `load_document` call — fetch, parse,
    /// cascade, layout-tree build, JS execution. Single end-to-end
    /// number; see `setup_stages` for the breakdown.
    setup_us: u64,
    /// Per-stage breakdown of the one-time setup, keyed by span
    /// name (`html_parse`, `css_extract`, `css_cascade`,
    /// `image_loading`, `layout_tree_build`, `script_loading`,
    /// `js_execute`, optionally `post_js_relayout`). Values are
    /// total microseconds for that stage; a stage that fires
    /// multiple times (image_loading fetching N images) has its
    /// per-call durations summed.
    setup_stages: BTreeMap<String, u64>,
    /// Per-stage aggregated samples for the render loop, keyed by
    /// span name. `BTreeMap` so JSON output is alphabetically
    /// stable across runs.
    render: BTreeMap<String, StageStats>,
}

#[derive(Serialize)]
struct Viewport {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct StageStats {
    /// Number of samples in this bin. Equal to `iterations` for
    /// every span that fires exactly once per render. Spans that
    /// fire multiple times per render will have a higher count,
    /// which is the signal that they're called more than once.
    samples: usize,
    mean_us: u64,
    p50_us: u64,
    p95_us: u64,
    min_us: u64,
    max_us: u64,
}

/// Compute summary statistics from a sample vector. Sorts in
/// place (well, on a copy) for the percentiles. `mean_us` is
/// floor-rounded — sub-microsecond precision isn't meaningful at
/// the scales we're benching.
#[allow(clippy::cast_possible_truncation)] // sample count comfortably fits u64
fn stats(samples: &[u64]) -> StageStats {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let sum: u64 = sorted.iter().sum();
    let mean_us = sum / n as u64;
    let p50_us = sorted[n / 2];
    let p95_us = sorted[(n * 95 / 100).min(n - 1)];
    StageStats {
        samples: n,
        mean_us,
        p50_us,
        p95_us,
        min_us: sorted[0],
        max_us: sorted[n - 1],
    }
}
