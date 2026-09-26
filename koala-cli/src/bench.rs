//! Per-render timing harness for `--bench` mode.
//!
//! Loads the requested document `setup_iterations` times (aggregating
//! the per-load setup stages so they are as comparable as the render
//! numbers), then warms the engine and runs N sample iterations of
//! [`render_document_once`]. A `tracing_subscriber::Layer` installed at
//! startup collects each span's close-time elapsed into a thread-local
//! event log; the harness drains the log between loads/renders and bins
//! durations by span name. The output is a JSON report with every
//! measured load and render, plus heap accounting and fingerprints of
//! the input and the rendered frame. `koala-lab` (tools/koala-lab)
//! compares reports from different builds; the layout it relies on is
//! versioned by [`SCHEMA_VERSION`].
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
use koala_common::alloc_count::{SIZE_BUCKET_BOUNDS, reset_peak, size_histogram, snapshot};
use crate::archive::ReplaySender;
use sha2::{Digest, Sha256};
use serde::Serialize;
use tracing::span;
use tracing_subscriber::layer::{Context as LayerContext, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::{LookupSpan, Registry};

use crate::render::render_document_once;

/// What one `--bench` run measures and how.
pub(crate) struct BenchConfig<'a> {
    /// File path or HTTP URL of the page.
    pub(crate) url: &'a str,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Sample count whose render timings get aggregated.
    pub(crate) iterations: u32,
    /// Discard renders run beforehand (lets the OS page in glyph
    /// atlases, lazy caches warm, etc.). Zero is supported but pollutes
    /// the first sample with cold-cache outliers; the `just bench`
    /// default of 3 keeps the noise floor below ~5 % on the landing page.
    pub(crate) warmup: u32,
    /// Measured document loads aggregated into the setup stats.
    pub(crate) setup_iterations: u32,
    /// Discard loads run before the measured ones.
    pub(crate) setup_warmup: u32,
    /// The sender serving fetches under `--replay`, read afterwards for
    /// the report's input digest.
    pub(crate) replay: Option<&'a ReplaySender>,
}

/// Run the bench harness described by `config`. Emits a single JSON
/// document to stdout — schema is the [`BenchReport`] struct below.
///
/// # Errors
///
/// Propagates errors from [`load_document`] and
/// [`render_document_once`]. A bench run failing partway through
/// emits no JSON.
#[allow(clippy::cast_possible_truncation)] // µs durations comfortably fit u64
pub(crate) fn run(config: &BenchConfig<'_>) -> Result<()> {
    let &BenchConfig {
        url,
        width,
        height,
        iterations,
        warmup,
        setup_iterations,
        setup_warmup,
        replay,
    } = config;
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
    // and keep every load's timings. The
    // `setup_warmup` discard-loads first do double duty: they let lazy
    // statics (notably the named-entity table) initialize so their
    // one-time cost stays out of the samples, AND they ramp the CPU /
    // warm OS caches before measurement. The latter matters more than it
    // sounds — the measured loads run at process start, so too little
    // warmup samples the frequency ramp and adds ~20% cross-process
    // variance, which a comparison between builds must not see.
    //
    // NOTE: each load re-runs `load_document`, which re-fetches the
    // source. Under `--replay` or for a local file that is a cheap read;
    // for a live URL it is a real network round-trip per iteration, so
    // live benching should pass `--setup-iterations 1`.
    for _ in 0..setup_warmup {
        let _ = load_document(url).with_context(|| format!("loading {url}"))?;
        let _ = take_events();
    }

    let mut setup_samples: Vec<LoadSample> = Vec::with_capacity(setup_iterations as usize);
    let mut setup_alloc: Option<AllocDelta> = None;
    let mut setup_histogram: Option<Vec<HistBucket>> = None;
    let mut doc = None;
    for _ in 0..setup_iterations {
        let alloc_before = snapshot();
        let hist_before = size_histogram();
        reset_peak();
        let start = Instant::now();
        let loaded = load_document(url).with_context(|| format!("loading {url}"))?;
        let total_us = start.elapsed().as_micros() as u64;

        // Allocation per load is deterministic on a fixed source, so one
        // representative sample (the first, post-warmup) is enough.
        if setup_alloc.is_none() {
            setup_alloc = Some(AllocDelta::between(alloc_before, snapshot()));
            setup_histogram = Some(hist_delta(hist_before, size_histogram()));
        }

        // A stage may fire more than once per load (e.g. image_loading
        // per image), so sum within the load, then record that per-load
        // total as one sample for the stage.
        let mut this_load: BTreeMap<String, u64> = BTreeMap::new();
        for ev in take_events() {
            *this_load.entry(ev.name.to_string()).or_insert(0) += ev.duration_us;
        }
        setup_samples.push(LoadSample {
            total_us,
            stages_us: this_load,
        });

        doc = Some(loaded);
    }

    let doc = doc.expect("setup_iterations clamped to >= 1, so the loop ran");
    let setup_alloc = setup_alloc.expect("at least one setup iteration ran");
    let setup_size_histogram = setup_histogram.expect("at least one setup iteration ran");

    // Attribute the small-allocation bucket to call sites. One extra load
    // under the armed allocator (kept out of the timing/alloc samples
    // above, since backtrace capture is slow), printing a top-N table to
    // stderr — the bench JSON on stdout stays clean.
    #[cfg(feature = "alloc-attribution")]
    {
        koala_common::alloc_count::attribution::arm(24);
        let _ = load_document(url).with_context(|| format!("loading {url}"))?;
        koala_common::alloc_count::attribution::dump(30);
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

    // One allocation delta per render iteration, transposed into
    // per-metric sample vectors below. Render of the same document is
    // near-deterministic in its allocation behavior, so these usually
    // show tiny variance — but we aggregate like the timings so an
    // outlier (e.g. a resize that only trips on some iterations) is
    // visible rather than averaged away.
    let mut alloc_samples: Vec<AllocDelta> = Vec::with_capacity(iterations as usize);
    let mut render_samples: Vec<BTreeMap<String, u64>> = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let alloc_before = snapshot();
        reset_peak();
        let _ = render_document_once(&doc, width, height, &font_provider)?;
        // Snapshot before draining timing events so the drain's own
        // allocations don't land in this iteration's render delta.
        alloc_samples.push(AllocDelta::between(alloc_before, snapshot()));
        let mut this_render: BTreeMap<String, u64> = BTreeMap::new();
        for ev in take_events() {
            *this_render.entry(ev.name.to_string()).or_insert(0) += ev.duration_us;
        }
        render_samples.push(this_render);
    }

    // One more render, outside the timed loop because hashing a full
    // frame costs milliseconds, to fingerprint what the page looks like.
    let render_hash = hex::encode(Sha256::digest(
        render_document_once(&doc, width, height, &font_provider)?.rgba_bytes(),
    ));

    let report = BenchReport {
        schema_version: SCHEMA_VERSION,
        url: url.to_string(),
        viewport: Viewport { width, height },
        iterations,
        warmup,
        setup_iterations,
        setup_warmup,
        setup_alloc,
        setup_size_histogram,
        render_alloc: RenderAlloc::aggregate(&alloc_samples),
        // Read after every load has run, so it covers everything served.
        input_digest: replay.map(ReplaySender::input_digest),
        render_hash,
        setup_samples,
        render_samples,
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
    /// [`SCHEMA_VERSION`] of the build that wrote this report.
    schema_version: u32,
    /// Verbatim path/URL passed on the command line. Useful when
    /// multiple report JSONs are pooled and a downstream tool
    /// needs to attribute timings.
    url: String,
    viewport: Viewport,
    iterations: u32,
    warmup: u32,
    /// Number of measured setup loads (see `setup_samples`), and the
    /// discard-loads run before them.
    setup_iterations: u32,
    setup_warmup: u32,
    /// Heap activity attributable to a single `load_document` call,
    /// sampled on the first measured load (deterministic on a fixed
    /// source). See [`AllocDelta`].
    setup_alloc: AllocDelta,
    /// Allocation-size distribution of that same setup load, bucketed by
    /// requested size. The low buckets gauge how many allocations a
    /// small-string-optimized string type could keep off the heap.
    setup_size_histogram: Vec<HistBucket>,
    /// Heap activity per render iteration, aggregated across the
    /// sample loop. See [`RenderAlloc`].
    render_alloc: RenderAlloc,
    /// Under `--replay`, the SHA-256 of every URL the loads were served
    /// and what each produced (see `ReplaySender::input_digest`). Two
    /// reports with equal digests measured identical inputs. `None` for
    /// live or local-file loads, whose inputs are not pinned.
    input_digest: Option<String>,
    /// SHA-256 of the RGBA pixels of one render of the final document.
    /// Two builds with equal hashes drew identical frames; a timing
    /// comparison between builds that draw different frames compares
    /// different work.
    render_hash: String,
    /// Every measured load, in order: whole-load time and per-stage time
    /// (`html_parse`, `css_cascade`, `js_execute`, ...). Raw rather than
    /// summarized, so an analysis can use any statistic without
    /// re-running the page.
    setup_samples: Vec<LoadSample>,
    /// Every measured render, in order: per-stage µs, summed within the
    /// render when a stage fires more than once.
    render_samples: Vec<BTreeMap<String, u64>>,
}

/// Version of the [`BenchReport`] layout. Bump when a field is added,
/// removed, or changes meaning, so readers can refuse a report they
/// would misread.
///
/// - 1: first versioned layout.
/// - 2: removed the `setup_us`, `setup_stages`, and `render` timing
///   summaries; `setup_samples` and `render_samples` carry the same data.
const SCHEMA_VERSION: u32 = 2;

/// One measured document load.
#[derive(Serialize)]
struct LoadSample {
    /// Wall-clock µs for the whole `load_document` call.
    total_us: u64,
    /// Per-stage µs, summed within the load when a stage fires more
    /// than once (`image_loading` per image). Stages that did not fire
    /// are absent.
    stages_us: BTreeMap<String, u64>,
}

#[derive(Serialize)]
struct Viewport {
    width: u32,
    height: u32,
}

/// One bucket of the allocation-size histogram: how many allocations of
/// the measured region requested at most `max_bytes` (and more than the
/// previous bucket's bound).
#[derive(Serialize)]
struct HistBucket {
    /// Inclusive upper bound of the bucket in bytes (`u64::MAX` is the
    /// catch-all for large allocations).
    max_bytes: u64,
    count: u64,
}

/// Element-wise difference of two cumulative size histograms, paired
/// with the bucket bounds — the size profile of whatever ran between the
/// two snapshots.
#[allow(clippy::cast_possible_truncation)] // counts fit u64 on any target
fn hist_delta(
    before: [usize; SIZE_BUCKET_BOUNDS.len()],
    after: [usize; SIZE_BUCKET_BOUNDS.len()],
) -> Vec<HistBucket> {
    SIZE_BUCKET_BOUNDS
        .iter()
        .zip(before.iter().zip(after.iter()))
        .map(|(&bound, (&b, &a))| HistBucket {
            max_bytes: bound as u64,
            count: a.saturating_sub(b) as u64,
        })
        .collect()
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
            net_live_bytes: bytes_allocated.cast_signed() - bytes_freed.cast_signed(),
            // `peak` was reset to the live baseline before the region,
            // so subtracting that baseline yields the extra heap held
            // at the worst moment. `saturating_sub` guards the
            // degenerate case where nothing allocated.
            peak_live_bytes: (end.peak.saturating_sub(before.live)) as u64,
        }
    }
}

/// Render-loop heap activity, aggregated across all sample
/// iterations. Each field summarizes one [`AllocDelta`] metric, so an
/// iteration that
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

/// Summary of a sample vector of byte or call counts.
#[derive(Serialize)]
struct Summary {
    samples: usize,
    mean: u64,
    p50: u64,
    p95: u64,
    min: u64,
    max: u64,
}

/// Mean, p50, p95, and range of `samples`. `mean` is floor-rounded.
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
