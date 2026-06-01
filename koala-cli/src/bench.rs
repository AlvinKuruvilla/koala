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
use koala_common::alloc_count::{reset_peak, snapshot};
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

    // Bracket the load with an allocation snapshot. `reset_peak`
    // pins the high-water baseline to whatever is live now (subscriber
    // + counters), so `setup_alloc.peak_live_bytes` reflects only the
    // load itself.
    let alloc_before_setup = snapshot();
    reset_peak();
    let setup_start = Instant::now();
    let doc = load_document(url).with_context(|| format!("loading {url}"))?;
    let setup_us = setup_start.elapsed().as_micros() as u64;
    let setup_alloc = AllocDelta::between(alloc_before_setup, snapshot());

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
    // One allocation delta per render iteration, transposed into
    // per-metric sample vectors below. Render of the same document is
    // near-deterministic in its allocation behavior, so these usually
    // show tiny variance — but we aggregate like the timings so an
    // outlier (e.g. a resize that only trips on some iterations) is
    // visible rather than averaged away.
    let mut alloc_samples: Vec<AllocDelta> = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let alloc_before = snapshot();
        reset_peak();
        let _ = render_document_once(&doc, width, height, &font_provider)?;
        // Snapshot before draining timing events so the drain's own
        // allocations don't land in this iteration's render delta.
        alloc_samples.push(AllocDelta::between(alloc_before, snapshot()));
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
        setup_alloc,
        render,
        render_alloc: RenderAlloc::aggregate(&alloc_samples),
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
    /// Heap activity attributable to the single `load_document` call
    /// (the `setup_us` region). See [`AllocDelta`].
    setup_alloc: AllocDelta,
    /// Per-stage aggregated samples for the render loop, keyed by
    /// span name. `BTreeMap` so JSON output is alphabetically
    /// stable across runs.
    render: BTreeMap<String, StageStats>,
    /// Heap activity per render iteration, aggregated across the
    /// sample loop. See [`RenderAlloc`].
    render_alloc: RenderAlloc,
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

/// Heap activity over one measured region, in *requested* bytes (see
/// `koala_common::alloc_count`). Computed as the delta between two
/// snapshots; never negative because the counters are monotonic and
/// `peak` is reset to baseline before the region.
#[derive(Serialize, Clone, Copy)]
struct AllocDelta {
    /// Bytes requested during the region (allocation churn).
    bytes_allocated: u64,
    /// Bytes returned during the region.
    bytes_freed: u64,
    /// Number of allocation calls during the region.
    alloc_calls: u64,
    /// Net live-byte change (`bytes_allocated − bytes_freed`); can be
    /// negative if the region frees more than it allocates, so it is
    /// signed.
    net_live_bytes: i64,
    /// Maximum live bytes reached during the region, measured above
    /// the footprint that was live at its start.
    peak_live_bytes: u64,
}

impl AllocDelta {
    /// Difference between a starting and ending snapshot. `reset_peak`
    /// is expected to have run at the `before` point so `end.peak`
    /// reflects this region's high-water mark.
    fn between(
        before: koala_common::alloc_count::AllocSnapshot,
        end: koala_common::alloc_count::AllocSnapshot,
    ) -> Self {
        let bytes_allocated = (end.total_allocated - before.total_allocated) as u64;
        let bytes_freed = (end.total_freed - before.total_freed) as u64;
        AllocDelta {
            bytes_allocated,
            bytes_freed,
            alloc_calls: (end.alloc_calls - before.alloc_calls) as u64,
            net_live_bytes: bytes_allocated as i64 - bytes_freed as i64,
            // `peak` was reset to the live baseline before the region,
            // so subtracting that baseline yields the extra heap held
            // at the worst moment. `saturating_sub` guards the
            // degenerate case where nothing allocated.
            peak_live_bytes: (end.peak.saturating_sub(before.live)) as u64,
        }
    }
}

/// Render-loop heap activity, aggregated across all sample
/// iterations. Each field summarizes one [`AllocDelta`] metric the
/// same way [`StageStats`] summarizes timings, so an iteration that
/// allocates anomalously (a capacity resize that only some renders
/// trip) is visible rather than averaged away.
#[derive(Serialize)]
struct RenderAlloc {
    bytes_allocated: Summary,
    alloc_calls: Summary,
    peak_live_bytes: Summary,
}

impl RenderAlloc {
    fn aggregate(samples: &[AllocDelta]) -> Self {
        let bytes: Vec<u64> = samples.iter().map(|d| d.bytes_allocated).collect();
        let calls: Vec<u64> = samples.iter().map(|d| d.alloc_calls).collect();
        let peak: Vec<u64> = samples.iter().map(|d| d.peak_live_bytes).collect();
        RenderAlloc {
            bytes_allocated: summarize(&bytes),
            alloc_calls: summarize(&calls),
            peak_live_bytes: summarize(&peak),
        }
    }
}

/// Unit-agnostic summary of a sample vector. Mirrors [`StageStats`]'s
/// statistics but with neutral field names, since these bins hold
/// bytes and counts rather than microseconds.
#[derive(Serialize)]
struct Summary {
    samples: usize,
    mean: u64,
    p50: u64,
    p95: u64,
    min: u64,
    max: u64,
}

/// Summary statistics for a non-time sample vector. Same percentile
/// convention as [`stats`].
fn summarize(samples: &[u64]) -> Summary {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let sum: u64 = sorted.iter().sum();
    Summary {
        samples: n,
        mean: sum / n as u64,
        p50: sorted[n / 2],
        p95: sorted[(n * 95 / 100).min(n - 1)],
        min: sorted[0],
        max: sorted[n - 1],
    }
}
