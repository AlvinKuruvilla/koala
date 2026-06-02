//! Per-render timing harness for `--bench` mode.
//!
//! Loads the requested document `setup_iterations` times (aggregating
//! the per-load setup stages so they are as comparable as the render
//! numbers), then warms the engine and runs N sample iterations of
//! [`render_document_once`]. A `tracing_subscriber::Layer` installed at
//! startup collects each span's close-time elapsed into a thread-local
//! event log; the harness drains the log between loads/renders and bins
//! durations by span name. The output is a JSON report with per-stage
//! mean / p50 / p95 / min / max for both setup and render, plus heap
//! accounting. The schema is consumed by `--bench-diff` (see
//! `bench_diff.rs`).
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
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use koala_browser::{FontProvider, load_document, warning};
use koala_common::alloc_count::{reset_peak, snapshot};
use serde::{Deserialize, Serialize};
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
    setup_iterations: u32,
    setup_warmup: u32,
) -> Result<()> {
    // At least one measured load is required — we keep its document for
    // the render loop and need a non-empty sample set for `stats`.
    let setup_iterations = setup_iterations.max(1);
    // Suppress informational stderr noise — font-load lines,
    // image-decode warnings, CSS parser warn_once messages. These
    // are useful for diagnosing real-world rendering, but during a
    // bench they pollute the report and would corrupt downstream
    // tooling that captures stderr alongside stdout.
    warning::set_quiet(true);

    install_subscriber();

    // Setup phase. A single load is too noisy to compare across builds —
    // its stages are measured once, so a 20% run-to-run swing reads as a
    // regression. Instead we load the document `setup_iterations` times
    // and aggregate per-stage timings into the same `StageStats` the
    // render loop produces, making setup numbers diff-worthy. The
    // `setup_warmup` discard-loads first do double duty: they let lazy
    // statics (notably the named-entity table) initialize so their
    // one-time cost stays out of the samples, AND they ramp the CPU /
    // warm OS caches before measurement. The latter matters more than it
    // sounds — the measured loads run at process start, so too little
    // warmup samples the frequency ramp and adds ~20% cross-process
    // variance, which is exactly the noise `--bench-diff` must not show.
    //
    // NOTE: each load re-runs `load_document`, which re-fetches the
    // source. For the cached-file path `just bench` uses that is a cheap
    // local read; for a live URL it is a real network round-trip per
    // iteration, so live benching should pass `--setup-iterations 1`.
    for _ in 0..setup_warmup {
        let _ = load_document(url).with_context(|| format!("loading {url}"))?;
        let _ = take_events();
    }

    let mut setup_us_samples: Vec<u64> = Vec::with_capacity(setup_iterations as usize);
    let mut setup_stage_samples: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let mut setup_alloc: Option<AllocDelta> = None;
    let mut doc = None;
    for _ in 0..setup_iterations {
        let alloc_before = snapshot();
        reset_peak();
        let start = Instant::now();
        let loaded = load_document(url).with_context(|| format!("loading {url}"))?;
        setup_us_samples.push(start.elapsed().as_micros() as u64);

        // Allocation per load is deterministic on a fixed source, so one
        // representative sample (the first, post-warmup) is enough.
        if setup_alloc.is_none() {
            setup_alloc = Some(AllocDelta::between(alloc_before, snapshot()));
        }

        // A stage may fire more than once per load (e.g. image_loading
        // per image), so sum within the load, then record that per-load
        // total as one sample for the stage.
        let mut this_load: BTreeMap<String, u64> = BTreeMap::new();
        for ev in take_events() {
            *this_load.entry(ev.name.to_string()).or_insert(0) += ev.duration_us;
        }
        for (name, total) in this_load {
            setup_stage_samples.entry(name).or_default().push(total);
        }

        doc = Some(loaded);
    }

    let doc = doc.expect("setup_iterations clamped to >= 1, so the loop ran");
    let setup_us = stats(&setup_us_samples);
    let setup_stages: BTreeMap<String, StageStats> = setup_stage_samples
        .into_iter()
        .map(|(name, samples)| (name, stats(&samples)))
        .collect();
    let setup_alloc = setup_alloc.expect("at least one setup iteration ran");

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
        setup_iterations,
        setup_warmup,
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

#[derive(Serialize, Deserialize)]
struct BenchReport {
    /// Verbatim path/URL passed on the command line. Useful when
    /// multiple report JSONs are pooled and a downstream tool
    /// needs to attribute timings.
    url: String,
    viewport: Viewport,
    iterations: u32,
    warmup: u32,
    /// Number of measured setup loads aggregated into `setup_us` /
    /// `setup_stages`, and the discard-loads run before them.
    setup_iterations: u32,
    setup_warmup: u32,
    /// Wall-clock cost of one `load_document` call — fetch, parse,
    /// cascade, layout-tree build, JS execution — aggregated across
    /// `setup_iterations` loads. See `setup_stages` for the breakdown.
    setup_us: StageStats,
    /// Per-stage breakdown of setup, keyed by span name (`html_parse`,
    /// `css_extract`, `css_cascade`, `image_loading`,
    /// `layout_tree_build`, `script_loading`, `js_execute`, optionally
    /// `post_js_relayout`). Each value aggregates one per-load total per
    /// measured iteration; a stage that fires multiple times within a
    /// load (image_loading per image) is summed within that load first.
    setup_stages: BTreeMap<String, StageStats>,
    /// Heap activity attributable to a single `load_document` call,
    /// sampled on the first measured load (deterministic on a fixed
    /// source). See [`AllocDelta`].
    setup_alloc: AllocDelta,
    /// Per-stage aggregated samples for the render loop, keyed by
    /// span name. `BTreeMap` so JSON output is alphabetically
    /// stable across runs.
    render: BTreeMap<String, StageStats>,
    /// Heap activity per render iteration, aggregated across the
    /// sample loop. See [`RenderAlloc`].
    render_alloc: RenderAlloc,
}

#[derive(Serialize, Deserialize)]
struct Viewport {
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize, Clone, Copy)]
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
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
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

// `--bench-diff`: compare two reports
//
// Reads two [`BenchReport`] JSONs and prints a per-metric before→after
// table. Lower is better for every metric (less time, fewer bytes,
// fewer allocations), so improvements render green and regressions red;
// changes within the noise band are dimmed so a wall of ±1% noise does
// not read as signal. Lives here, not in a separate module, so it can
// deserialize the private report types directly.

/// Percent-change threshold below which a delta is treated as noise and
/// rendered neutral. Setup stages now aggregate across loads, but a few
/// percent of run-to-run jitter still survives; only color past it.
const NOISE_PCT: f64 = 2.0;

/// Compare two bench reports and print a colored delta table.
///
/// # Errors
///
/// Propagates I/O errors reading either file and `serde_json` errors if
/// a file is not a valid [`BenchReport`].
pub(crate) fn diff(before_path: &Path, after_path: &Path) -> Result<()> {
    let before = read_report(before_path)?;
    let after = read_report(after_path)?;

    println!("{}", "bench-diff (before → after, lower is better)".bold());
    println!(
        "  before: {}  (setup ×{}, render ×{})",
        before.url, before.setup_iterations, before.iterations
    );
    println!(
        "  after:  {}  (setup ×{}, render ×{})",
        after.url, after.setup_iterations, after.iterations
    );

    println!("\n{}", "SETUP — per-stage mean µs".underline());
    for stage in stage_union(&before.setup_stages, &after.setup_stages) {
        print_metric(
            &stage,
            before.setup_stages.get(&stage).map(|s| s.mean_us),
            after.setup_stages.get(&stage).map(|s| s.mean_us),
        );
    }
    print_metric(
        "(total load)",
        Some(before.setup_us.mean_us),
        Some(after.setup_us.mean_us),
    );

    println!("\n{}", "RENDER — per-stage mean µs".underline());
    for stage in stage_union(&before.render, &after.render) {
        print_metric(
            &stage,
            before.render.get(&stage).map(|s| s.mean_us),
            after.render.get(&stage).map(|s| s.mean_us),
        );
    }

    println!("\n{}", "ALLOCATION — bytes / calls".underline());
    print_metric(
        "setup bytes",
        Some(before.setup_alloc.bytes_allocated),
        Some(after.setup_alloc.bytes_allocated),
    );
    print_metric(
        "setup alloc calls",
        Some(before.setup_alloc.alloc_calls),
        Some(after.setup_alloc.alloc_calls),
    );
    print_metric(
        "setup peak live",
        Some(before.setup_alloc.peak_live_bytes),
        Some(after.setup_alloc.peak_live_bytes),
    );
    print_metric(
        "render bytes (mean)",
        Some(before.render_alloc.bytes_allocated.mean),
        Some(after.render_alloc.bytes_allocated.mean),
    );
    print_metric(
        "render alloc calls (mean)",
        Some(before.render_alloc.alloc_calls.mean),
        Some(after.render_alloc.alloc_calls.mean),
    );
    print_metric(
        "render peak live (mean)",
        Some(before.render_alloc.peak_live_bytes.mean),
        Some(after.render_alloc.peak_live_bytes.mean),
    );

    Ok(())
}

fn read_report(path: &Path) -> Result<BenchReport> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading bench report '{}'", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parsing bench report '{}'", path.display()))
}

/// Sorted union of stage names present in either report, so a stage that
/// appears in only one side (e.g. `post_js_relayout`) is still shown.
fn stage_union(a: &BTreeMap<String, StageStats>, b: &BTreeMap<String, StageStats>) -> Vec<String> {
    a.keys()
        .chain(b.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Print one `before → after` row, colored by direction once the change
/// clears the noise band. `None` on either side marks a stage that only
/// one report has.
#[allow(clippy::cast_precision_loss)] // counts/durations fit f64 for a ratio
fn print_metric(label: &str, before: Option<u64>, after: Option<u64>) {
    let cell = match (before, after) {
        (Some(b), Some(a)) => {
            let pct = if b == 0 {
                if a == 0 { 0.0 } else { 100.0 }
            } else {
                (a as f64 - b as f64) / b as f64 * 100.0
            };
            let arrow = if a < b {
                "↓"
            } else if a > b {
                "↑"
            } else {
                "="
            };
            let body = format!("{:>14} → {:>14}  {arrow}{pct:+6.1}%", commas(b), commas(a));
            if pct.abs() < NOISE_PCT {
                body.dimmed().to_string()
            } else if a < b {
                body.green().to_string()
            } else {
                body.red().to_string()
            }
        }
        (None, Some(a)) => format!("{:>14} → {:>14}  (new)", "—", commas(a)),
        (Some(b), None) => format!("{:>14} → {:>14}  (gone)", commas(b), "—"),
        (None, None) => return,
    };
    println!("  {label:<26} {cell}");
}

/// Format an integer with thousands separators (`1234567` → `1,234,567`).
fn commas(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
