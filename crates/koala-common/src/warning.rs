//! The process-wide quiet flag.
//!
//! When set, [`crate::diagnostics::report`] records nothing and other
//! diagnostic output in the engine gates itself on [`is_quiet`]. Set by
//! `--quiet` on the binaries, by `koala --wpt-protocol` so per-test stderr
//! stays empty unless a real error fires, and by the bench harness.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set once at process startup; never toggled mid-run.
static QUIET: AtomicBool = AtomicBool::new(false);

/// Enable or disable quiet mode for this process.
///
/// Intended to be called exactly once, early in startup (before any
/// document load). Callers that want partial silencing should branch
/// on [`is_quiet`] at their own sites rather than flipping the flag
/// repeatedly.
pub fn set_quiet(value: bool) {
    QUIET.store(value, Ordering::Relaxed);
}

/// Returns true when the process is in quiet mode. Cheap to call from
/// hot paths.
#[must_use]
pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}
