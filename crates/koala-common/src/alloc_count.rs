//! Process-wide heap accounting via a counting global allocator.
//!
//! [`CountingAllocator`] is a thin wrapper around the system
//! allocator that tallies every allocation and deallocation into a
//! set of process-global atomic counters. Registering it as the
//! `#[global_allocator]` of a binary turns those counters on; until
//! a binary does so the type is inert and the counters stay at zero,
//! so non-instrumented builds (the shipping renderer, the GUI in its
//! normal configuration) pay nothing.
//!
//! # Why this lives in `koala-common`
//!
//! Two binaries want the same accounting: `koala-cli`'s `--bench`
//! harness (reproducible, scripted A/B of engine changes) and a
//! future `koala-ui` developer HUD (live heap watch during a real
//! browsing session). Both depend on `koala-common`, and a
//! `#[global_allocator]` must be declared in the final binary crate,
//! so the *type* lives here while each binary owns the one-line
//! registration.
//!
//! # What the numbers mean
//!
//! - **total allocated / freed** — monotonic byte counters. Their
//!   difference over a window is *churn*: how much the code under
//!   test moved through the allocator, regardless of net growth.
//!   This is the signal for "does the new data structure allocate
//!   more on insert / resize?"
//! - **live** — bytes currently allocated but not yet freed
//!   (`allocated − freed`). The instantaneous footprint.
//! - **peak** — the high-water mark of `live`. Combined with
//!   [`reset_peak`], this isolates the maximum footprint of one
//!   measured region (e.g. a single render), which is the signal for
//!   "does this hold more heap at its worst moment?"
//!
//! # Accuracy caveats
//!
//! Accounting is keyed on [`Layout::size`], so it tracks *requested*
//! bytes, not the allocator's rounded-up real footprint or its own
//! metadata. It is therefore a faithful measure of what Koala's code
//! asks for, not of RSS as the OS sees it. For comparing two Koala
//! builds under an identical workload — the use case this exists for
//! — requested bytes is the right unit: it is deterministic and
//! attributable, where RSS is dominated by allocator slack, the JS
//! heap, and framebuffers.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::System;

// Process-global tallies. `Relaxed` ordering is sufficient: these
// are independent counters with no happens-before relationship to
// protect, and `snapshot` does not need a consistent cut across all
// six — a few allocations of skew between counters is immaterial at
// the scales we measure.
static TOTAL_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static TOTAL_FREED: AtomicUsize = AtomicUsize::new(0);
static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static FREE_CALLS: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// Inclusive upper bounds (bytes) of the allocation-size histogram
/// buckets; the last is `usize::MAX` (catch-all).
///
/// The low buckets straddle the small-string-optimization range — most
/// SSO string types inline up to ~22–24 bytes — so `≤16` + `≤24`
/// together answer "what fraction of allocations could an SSO string
/// have kept off the heap?".
pub const SIZE_BUCKET_BOUNDS: [usize; 8] = [16, 24, 32, 64, 128, 256, 1024, usize::MAX];

/// Per-bucket allocation counts, parallel to [`SIZE_BUCKET_BOUNDS`].
static SIZE_BUCKETS: [AtomicUsize; 8] = [const { AtomicUsize::new(0) }; 8];

/// Index of the first bucket whose upper bound covers `size`.
fn bucket_index(size: usize) -> usize {
    SIZE_BUCKET_BOUNDS
        .iter()
        .position(|&bound| size <= bound)
        .unwrap_or(SIZE_BUCKET_BOUNDS.len() - 1)
}

/// A `#[global_allocator]` that forwards every request to the system
/// allocator and records the byte counts.
///
/// Stateless and zero-sized — all accounting lives in module-level
/// statics, so the allocator itself can be a unit value in a `static`
/// slot.
///
/// # Examples
///
/// Registering it in a binary's crate root:
///
/// ```ignore
/// #[global_allocator]
/// static GLOBAL: koala_common::alloc_count::CountingAllocator =
///     koala_common::alloc_count::CountingAllocator;
/// ```
pub struct CountingAllocator;

/// A consistent-enough read of the global counters at one instant.
///
/// Field values are *requested* bytes / call counts (see the module
/// docs on accuracy). Take two snapshots around a region of interest
/// and subtract to attribute heap activity to that region.
#[derive(Clone, Copy, Debug)]
pub struct AllocSnapshot {
    /// Cumulative bytes ever requested from the allocator.
    pub total_allocated: usize,
    /// Cumulative bytes ever returned to the allocator.
    pub total_freed: usize,
    /// Cumulative number of allocation calls.
    pub alloc_calls: usize,
    /// Cumulative number of deallocation calls.
    pub free_calls: usize,
    /// Bytes currently allocated and not yet freed.
    pub live: usize,
    /// High-water mark of `live` since the last [`reset_peak`] (or
    /// process start, if never reset).
    pub peak: usize,
}

/// Read the current values of all global counters.
#[must_use]
pub fn snapshot() -> AllocSnapshot {
    AllocSnapshot {
        total_allocated: TOTAL_ALLOCATED.load(Ordering::Relaxed),
        total_freed: TOTAL_FREED.load(Ordering::Relaxed),
        alloc_calls: ALLOC_CALLS.load(Ordering::Relaxed),
        free_calls: FREE_CALLS.load(Ordering::Relaxed),
        live: LIVE.load(Ordering::Relaxed),
        peak: PEAK.load(Ordering::Relaxed),
    }
}

/// Read the cumulative per-bucket allocation counts, parallel to
/// [`SIZE_BUCKET_BOUNDS`]. Snapshot before and after a region and
/// subtract element-wise to get that region's allocation-size profile.
#[must_use]
pub fn size_histogram() -> [usize; SIZE_BUCKET_BOUNDS.len()] {
    let mut out = [0usize; SIZE_BUCKET_BOUNDS.len()];
    for (slot, bucket) in out.iter_mut().zip(SIZE_BUCKETS.iter()) {
        *slot = bucket.load(Ordering::Relaxed);
    }
    out
}

/// Reset the peak high-water mark down to the current live footprint.
///
/// Call this immediately before a region you want a clean peak for;
/// the `peak` field of the snapshot taken afterward then reflects the
/// maximum footprint reached *during* that region, measured above the
/// baseline that was live when this was called.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

// Bump the allocation counters and raise `PEAK` if this allocation
// set a new high-water mark. The compare-exchange loop is the
// standard lock-free max update: retry until we either win the race
// or observe a peak already at least as high as ours.
fn record_alloc(size: usize) {
    let _ = TOTAL_ALLOCATED.fetch_add(size, Ordering::Relaxed);
    let _ = ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = SIZE_BUCKETS[bucket_index(size)].fetch_add(1, Ordering::Relaxed);

    // Opt-in call-site attribution. Cheap (one relaxed load) when the
    // feature is built but disarmed; compiled out entirely otherwise.
    #[cfg(feature = "alloc-attribution")]
    attribution::record(size);

    let new_live = LIVE.fetch_add(size, Ordering::Relaxed) + size;

    let mut peak = PEAK.load(Ordering::Relaxed);
    while new_live > peak {
        match PEAK.compare_exchange_weak(peak, new_live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => peak = observed,
        }
    }
}

// Mirror of `record_alloc` for the free path. `live` cannot
// legitimately underflow — every dealloc corresponds to a prior
// alloc of the same size — so a plain `fetch_sub` is correct.
fn record_free(size: usize) {
    let _ = TOTAL_FREED.fetch_add(size, Ordering::Relaxed);
    let _ = FREE_CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = LIVE.fetch_sub(size, Ordering::Relaxed);
}

// SAFETY: every method forwards verbatim to `System`, whose
// `GlobalAlloc` impl already upholds the trait's contract; the only
// added work is reading `layout.size()` (always valid) and touching
// atomics (always sound). Accounting happens only on the success
// path so a failed/null allocation is not counted.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is the caller's valid layout, forwarded
        // unchanged to the system allocator.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as `alloc`; delegating preserves `System`'s
        // zeroing fast path (calloc) rather than the slower trait
        // default of alloc-then-write_bytes.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`/`layout` are a matched pair from a prior
        // allocation through this allocator, per the trait contract.
        unsafe { System.dealloc(ptr, layout) };
        record_free(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Delegate to `System::realloc` so a genuine in-place grow is
        // preserved, then book it as freeing the old size and
        // allocating the new one. Only account on success — on null
        // the original block is left intact and untouched.
        // SAFETY: `ptr`/`layout` are a matched pair and `new_size`
        // satisfies the trait's size constraints, forwarded unchanged.
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            record_free(layout.size());
            record_alloc(new_size);
        }
        new_ptr
    }
}

/// Call-site attribution for small allocations — a lightweight stand-in
/// for dhat that answers "which code allocates all the tiny blocks?"
/// without dhat's giant `dhat-heap.json`.
///
/// dhat serializes the full backtrace tree of every live allocation,
/// which is what makes its artifact enormous. We only want a ranked list
/// of the hottest call sites, so instead of writing a file we aggregate
/// captures in memory keyed by (unresolved) call stack and print a top-N
/// table to stderr. Symbolization is deferred to [`attribution::dump`],
/// so the hot path only walks the stack.
///
/// The whole module is gated behind the `alloc-attribution` feature and,
/// even when built, does nothing until [`attribution::arm`] is called —
/// so it is inert outside an explicit measurement window. Capture is
/// bounded to allocations of at most the armed size, which is the only
/// range we care about (the SSO bucket).
///
/// # Backtrace fidelity
///
/// Release builds inline aggressively, so captured stacks can be
/// collapsed; build the probe target with debuginfo (the bench profile
/// already does) and read the top frames as "near here", not gospel.
#[cfg(feature = "alloc-attribution")]
pub mod attribution {
    use core::ffi::c_void;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    /// Stack frames captured per allocation — enough to step past the
    /// allocator / `Vec` / `String` / `HashMap` plumbing into engine code.
    const MAX_FRAMES: usize = 32;

    /// Engine frames to show per call site (the first one plus a little
    /// caller context).
    const FRAMES_SHOWN: usize = 3;

    static ARMED: AtomicBool = AtomicBool::new(false);
    static MAX_SIZE: AtomicUsize = AtomicUsize::new(0);

    // Distinct call stacks (frame instruction pointers, unresolved) → how
    // many in-range allocations came from each. Boxed slice so the key is
    // compact; symbolization is deferred to `dump`.
    static SITES: LazyLock<Mutex<HashMap<Box<[usize]>, u64>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    thread_local! {
        // Re-entrancy guard: capturing a backtrace and inserting into the
        // map both allocate, re-entering the global allocator. Without
        // this we would recurse forever; with it, those inner allocations
        // are still counted globally but not attributed.
        static IN_HOOK: Cell<bool> = const { Cell::new(false) };
    }

    /// Begin attributing allocations of at most `max_size` bytes,
    /// discarding any prior capture. Pair with [`dump`].
    pub fn arm(max_size: usize) {
        if let Ok(mut sites) = SITES.lock() {
            sites.clear();
        }
        MAX_SIZE.store(max_size, Ordering::Relaxed);
        ARMED.store(true, Ordering::Relaxed);
    }

    /// Hook from the allocator's record path. One relaxed load when
    /// disarmed; full capture only inside an armed window for in-range
    /// sizes.
    #[inline]
    pub(super) fn record(size: usize) {
        if !ARMED.load(Ordering::Relaxed) || size > MAX_SIZE.load(Ordering::Relaxed) {
            return;
        }
        IN_HOOK.with(|guard| {
            if guard.get() {
                return;
            }
            guard.set(true);

            let mut frames: Vec<usize> = Vec::with_capacity(MAX_FRAMES);
            backtrace::trace(|frame| {
                frames.push(frame.ip() as usize);
                frames.len() < MAX_FRAMES
            });
            if let Ok(mut sites) = SITES.lock() {
                *sites.entry(frames.into_boxed_slice()).or_insert(0) += 1;
            }

            guard.set(false);
        });
    }

    /// Stop attributing and print the `top_n` hottest call sites to
    /// stderr, most-frequent first. Each line shows the count and the
    /// first few engine frames, skipping allocator and std plumbing.
    ///
    /// # Panics
    ///
    /// Panics if the attribution map mutex is poisoned.
    pub fn dump(top_n: usize) {
        ARMED.store(false, Ordering::Relaxed);

        let mut ranked: Vec<(Box<[usize]>, u64)> = {
            let sites = SITES.lock().expect("attribution map poisoned");
            sites.iter().map(|(k, v)| (k.clone(), *v)).collect()
        };
        ranked.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        let total: u64 = ranked.iter().map(|(_, c)| c).sum();
        eprintln!(
            "alloc-attribution: {total} allocations <= {} B across {} call sites; top {}:",
            MAX_SIZE.load(Ordering::Relaxed),
            ranked.len(),
            top_n.min(ranked.len()),
        );
        for (frames, count) in ranked.into_iter().take(top_n) {
            eprintln!("  {count:>8}  {}", describe(&frames));
        }
    }

    // Symbolize the captured frames into a one-line description: the first
    // few frames that look like engine code, with the allocator/std
    // plumbing skipped as noise.
    fn describe(frames: &[usize]) -> String {
        let mut picked: Vec<String> = Vec::new();
        for &ip in frames {
            let mut name = String::new();
            backtrace::resolve(ip as *mut c_void, |sym| {
                // Keep the first resolvable (innermost inlined) name for
                // this address; later inline frames are caller context we
                // pick up from subsequent stack frames instead.
                if name.is_empty() {
                    name = sym.name().map(|n| n.to_string()).unwrap_or_default();
                }
            });
            if name.is_empty() || is_noise(&name) {
                continue;
            }
            picked.push(trim_hash(&name));
            if picked.len() == FRAMES_SHOWN {
                break;
            }
        }
        if picked.is_empty() {
            "<unresolved>".to_string()
        } else {
            picked.join("  <-  ")
        }
    }

    // Frames that describe *how* something allocated rather than *what*
    // asked for it. We want the engine call site, not the capture hook,
    // the allocator shim, or the Vec/HashMap/String machinery on top.
    //
    // Two name shapes reach here: fully-qualified paths from the symbol
    // table (`alloc::vec::...`) and bare leaf names from DWARF inline
    // records (`record`, `{closure#0}`) — so we match both prefixes and
    // exact leaves, plus a few substrings for the allocator glue.
    fn is_noise(name: &str) -> bool {
        const PREFIXES: [&str; 8] = [
            "alloc::",
            "<alloc::",
            "core::",
            "std::",
            "hashbrown::",
            "backtrace::",
            "koala_common::alloc_count",
            "__rust",
        ];
        // Our own capture hook and the allocator entry points, as they
        // appear when resolved to bare inlined names.
        const LEAVES: [&str; 8] = [
            "trace",
            "record",
            "record_alloc",
            "alloc",
            "alloc_zeroed",
            "realloc",
            "malloc",
            "calloc",
        ];
        PREFIXES.iter().any(|prefix| name.starts_with(prefix))
            || LEAVES.contains(&name)
            || name.contains("{closure")
            || name.contains("GlobalAlloc")
            || name.contains("Allocator")
    }

    // Drop the trailing `::h<hex>` disambiguator rustc appends to symbol
    // names so the output reads cleanly.
    fn trim_hash(name: &str) -> String {
        match name.rfind("::h") {
            Some(idx) if name[idx + 3..].bytes().all(|b| b.is_ascii_hexdigit()) => {
                name[..idx].to_string()
            }
            _ => name.to_string(),
        }
    }
}
