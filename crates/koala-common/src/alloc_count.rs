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
