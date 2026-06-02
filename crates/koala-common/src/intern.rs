//! Process-global string interning (FlyString Phase 1).
//!
//! The single global [`Interner`] the whole engine shares, so equal
//! strings resolve to the *same* `Arc` no matter which thread interned
//! them. That cross-thread identity is what makes [`FlyString`] equality
//! an O(1) pointer compare in practice — a per-thread interner would hand
//! out distinct `Arc`s for the same text and break it.
//!
//! The interner type and its dedup logic live in koala-std (`no_std`,
//! using `alloc`'s `Arc`); only this global — which needs `std`'s
//! `Mutex` / `LazyLock` — lives here, matching koala-std's "OS/sync bits
//! sit above us" layering.

use std::sync::{LazyLock, Mutex};

use koala_std::string::Interner;

pub use koala_std::string::FlyString;

/// The one process-wide interner. Behind a `Mutex` because interning
/// mutates the table and runs from several threads (per-tab load workers
/// plus the main thread).
static INTERNER: LazyLock<Mutex<Interner>> = LazyLock::new(|| Mutex::new(Interner::new()));

/// Interns `s` in the global table, returning a shared [`FlyString`].
///
/// The same content always yields a pointer-equal handle, across every
/// thread — the guarantee `FlyString`'s pointer-based equality and
/// hashing rely on.
///
/// # Panics
///
/// Panics if the interner mutex is poisoned (a prior panic while
/// interning). That should never happen on valid input; failing loudly
/// is preferable to handing back results from a possibly half-updated
/// table.
pub fn intern(s: &str) -> FlyString {
    INTERNER
        .lock()
        .expect("global string interner mutex poisoned")
        .intern(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_interns_to_equal_handles() {
        assert_eq!(intern("color"), intern("color"));
        assert_ne!(intern("color"), intern("background"));
    }

    #[test]
    fn shared_identity_across_threads() {
        // The global table hands out one canonical Arc per string to every
        // thread — the cross-thread identity a per-thread interner can't
        // give, and the whole reason equality can be a pointer compare.
        let handles: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| intern("shared-token")))
            .collect();
        let from_main = intern("shared-token");
        for handle in handles {
            let from_worker = handle.join().expect("worker thread panicked");
            assert_eq!(from_worker, from_main);
        }
    }
}
