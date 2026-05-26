//! Browser warnings with colored terminal output.
//!
//! Provides deduplication to avoid spamming the same warning multiple times.
//! Used by HTML, CSS, and DOM components to report unsupported features.
//!
//! Also hosts the process-wide quiet flag (see [`set_quiet`]). When set,
//! [`warn_once`] is a no-op and other diagnostic call sites in the engine
//! gate themselves on [`is_quiet`]. Used by `koala-cli --wpt-protocol`
//! so per-test stderr stays empty unless a real error fires.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// ANSI color codes for terminal output
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Global set of warnings we've already printed (to deduplicate)
static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// When true, [`warn_once`] is a no-op and engine internals are expected
/// to skip their own informational `eprintln`s. Set once at process
/// startup; never toggled mid-run.
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

/// Warn about an unsupported feature (prints once per unique message)
///
/// # Example
/// ```ignore
/// warn_once("CSS", "unsupported unit 'em' in font-size: 1.5em");
/// ```
///
/// # Panics
/// Panics if the global warning set mutex is poisoned.
pub fn warn_once(component: &str, message: &str) {
    if is_quiet() {
        return;
    }
    let key = format!("[{component}] {message}");
    let should_print = WARNED
        .lock()
        .unwrap()
        .get_or_insert_with(HashSet::new)
        .insert(key);

    if should_print {
        eprintln!("{YELLOW}[Koala {component}] ⚠ {message}{RESET}");
    }
}

/// Clear all recorded warnings (call when loading a new page)
///
/// # Panics
/// Panics if the global warning set mutex is poisoned.
pub fn clear_warnings() {
    let mut guard = WARNED.lock().unwrap();
    if let Some(set) = guard.as_mut() {
        set.clear();
    }
}
