//! Browser warnings with colored terminal output.
//!
//! Provides deduplication to avoid spamming the same warning multiple times.
//! Used by HTML, CSS, and DOM components to report unsupported features.

use std::collections::HashSet;
use std::sync::Mutex;

/// ANSI color codes for terminal output
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Global set of warnings we've already printed (to deduplicate)
static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

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
