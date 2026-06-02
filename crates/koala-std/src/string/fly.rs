//! Interned strings: [`FlyString`] and its backing [`Interner`].
//!
//! SCAFFOLD (Phase 0): structure, signatures, and wiring are in place;
//! the bodies are `todo!()` for the learning-partner pass. The contract
//! is pinned by `tests/fly_string.rs`; implement until green
//! (`cargo test -p koala-std --test fly_string`). Design lives in
//! `project-memory/koala-std-flystring-design.md`.

use alloc::sync::Arc;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Deref;

use crate::collections::HashSet;

/// An interned string — a cheap, clonable handle to a uniquely-stored
/// `str`.
///
/// Equality and hashing are by pointer (O(1)), which is correct only
/// among `FlyString`s minted by the **same** [`Interner`] (in
/// production, the one process-global interner). There is no public
/// constructor: a `FlyString` can only be born from [`Interner::intern`],
/// so the single-interner invariant cannot be bypassed.
#[derive(Clone)]
pub struct FlyString {
    inner: Arc<str>,
}

impl FlyString {
    /// Crate-private mint point. Only an [`Interner`] calls this, after
    /// guaranteeing `inner` is the canonical `Arc` for its contents.
    pub(crate) fn from_arc(inner: Arc<str>) -> Self {
        Self { inner }
    }

    /// The interned string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl Deref for FlyString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.inner
    }
}

impl PartialEq for FlyString {
    fn eq(&self, other: &Self) -> bool {
        let equality = Arc::ptr_eq(&self.inner, &other.inner);
        debug_assert!(
            equality || self.as_str() != other.as_str(),
            "FlyString: equal content with different pointers — \
           strings from two different interners were compared",
        );
        equality
    }
}

impl Eq for FlyString {}

impl Hash for FlyString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.inner).cast::<()>() as usize).hash(state);
    }
}

impl fmt::Debug for FlyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for FlyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A string-interning table. Hands out [`FlyString`]s, guaranteeing that
/// equal content shares a single `Arc<str>`.
///
/// Backed by a `koala_std` [`HashSet`] of `Arc<str>` — the first real
/// internal consumer of milestone 1.
pub struct Interner {
    table: HashSet<Arc<str>>,
}

impl Interner {
    /// Creates an empty interner.
    #[must_use]
    pub fn new() -> Self {
        // IMPLEMENT: wrap an empty HashSet.
        Self {
            table: HashSet::new(),
        }
    }

    /// Creates an interner pre-sized for `capacity` distinct strings.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            table: HashSet::with_capacity(capacity),
        }
    }

    /// Number of distinct interned strings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Whether the interner holds no strings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// Interns `s`: returns the existing handle if it has been seen
    /// before, otherwise stores it and returns a fresh handle. Either
    /// way the returned `FlyString` shares the canonical `Arc` for `s`.
    pub fn intern(&mut self, s: &str) -> FlyString {
        if let Some(existing) = self.table.get(s) {
            return FlyString::from_arc(existing.clone());
        }
        let arc: Arc<str> = Arc::from(s);
        let _ = self.table.insert(arc.clone());
        FlyString::from_arc(arc)
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}
