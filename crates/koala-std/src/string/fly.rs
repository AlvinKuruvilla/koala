//! Small-string-optimized, optionally-interned strings: [`FlyString`].
//!
//! A `FlyString` is a cheap-to-clone, cheap-to-compare string handle for
//! the engine's high-frequency text: tag names, attribute names, CSS
//! idents. It is modeled on Ladybird's `AK::FlyString` but built over
//! [`ecow::EcoString`] as the small-string substrate.
//!
//! This module lives behind the crate's `std` feature: the process-global
//! interner needs `std`'s `Mutex`/`LazyLock`. See the crate-level docs for
//! why that is the one sanctioned `std` dependency. `EcoString` is an
//! interim substrate — once koala-std grows a hand-rolled SSO string, it
//! swaps in behind this same wrapper API, with `ecow` retained as the
//! differential oracle.
//!
//! # Two regimes, one type
//!
//! Construction decides the storage, and the caller never has to think
//! about it:
//!
//! - **Short strings** (fit in `EcoString`'s inline storage) live inline
//!   in the handle itself — no heap allocation, and *not* interned.
//!   Interning a short string would cost a heap slot and a table entry to
//!   deduplicate something that already fits in a register-sized handle;
//!   Ladybird skips the table for short strings for exactly this reason,
//!   and it is why our earlier "intern every tag name" experiment moved
//!   almost no allocations.
//! - **Long strings** are deduplicated through the process-global table.
//!   Equal content resolves to one shared `EcoString` allocation, so a
//!   clone is a refcount bump and equality of two interned handles is a
//!   pointer compare.
//!
//! # Equality and hashing
//!
//! Equality is content equality, implemented to be O(1) on the common
//! path: interned long strings share an allocation, so an equal-pointer
//! check settles them without touching bytes; short strings fall through
//! to a byte compare that is cheap because they are tiny. Hashing is by
//! content (not by pointer) — this is required for correctness, because
//! two equal *short* strings have distinct addresses yet must hash equal.
//!
//! # Lifetime of interned strings
//!
//! The table holds a strong clone of every long string it has seen, so
//! interned strings live for the life of the process. That is the right
//! trade for a bounded vocabulary (tag/attribute names, CSS property
//! names) and a small leak for unbounded transient text — so callers
//! should intern *names*, not arbitrary document text. `ecow` exposes no
//! weak handle, so self-GC (which Ladybird gets from its refcounted
//! `StringData`) is left as future work.

use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{LazyLock, Mutex};

use ecow::EcoString;

/// Strings up to this many bytes are kept inline rather than interned.
///
/// This mirrors `EcoString`'s inline capacity on 64-bit little-endian
/// targets (Koala's platforms). It is a *table-skip heuristic*, not a
/// correctness boundary: interning a string that would have fit inline
/// is merely wasteful, never wrong, so a stale value here can only cost
/// a little memory.
const INLINE_CAPACITY: usize = 15;

/// The one process-wide dedup table for long strings. Behind a `Mutex`
/// because interning mutates it from several threads (per-tab load
/// workers plus the main thread).
static TABLE: LazyLock<Mutex<HashSet<EcoString>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// A small-string-optimized, optionally-interned string handle.
///
/// Cheap to clone (inline copy or refcount bump) and cheap to compare.
/// See the [module docs](self) for the short-vs-long storage split.
#[derive(Clone, Default)]
pub struct FlyString {
    inner: EcoString,
}

impl FlyString {
    /// Creates a `FlyString` from `s`, interning it if it is long enough
    /// to warrant deduplication (see the [module docs](self)).
    ///
    /// Short strings are stored inline and returned without touching the
    /// global table; long strings resolve to the canonical shared handle
    /// for their content.
    ///
    /// # Panics
    ///
    /// Panics if the interner mutex is poisoned (a prior panic while
    /// interning). Failing loudly is preferable to returning a result from
    /// a possibly half-updated table.
    #[must_use]
    pub fn new(s: &str) -> Self {
        if s.len() <= INLINE_CAPACITY {
            return Self {
                inner: EcoString::from(s),
            };
        }

        let mut table = TABLE
            .lock()
            .expect("global FlyString interner mutex poisoned");
        if let Some(existing) = table.get(s) {
            return Self {
                inner: existing.clone(),
            };
        }
        let canonical = EcoString::from(s);
        let _ = table.insert(canonical.clone());
        Self { inner: canonical }
    }

    /// The string slice this handle refers to.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Number of bytes in the string.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the string is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// ASCII-case-insensitive comparison against a string slice.
    ///
    /// HTML tag and attribute names and many CSS idents match
    /// case-insensitively, so this is a common operation on names.
    #[must_use]
    pub fn equals_ignoring_ascii_case(&self, other: &str) -> bool {
        self.as_str().eq_ignore_ascii_case(other)
    }

    /// Whether the string equals any of `candidates`. Ergonomic for
    /// dispatching on a known set of tag or keyword names.
    #[must_use]
    pub fn is_one_of(&self, candidates: &[&str]) -> bool {
        candidates.iter().any(|c| self.as_str() == *c)
    }

    /// Number of distinct long strings currently interned. Primarily of
    /// interest to tests and diagnostics.
    ///
    /// # Panics
    ///
    /// Panics if the interner mutex is poisoned.
    #[must_use]
    pub fn interned_count() -> usize {
        TABLE
            .lock()
            .expect("global FlyString interner mutex poisoned")
            .len()
    }
}

impl Deref for FlyString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.inner
    }
}

impl Borrow<str> for FlyString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for FlyString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for FlyString {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

impl From<&String> for FlyString {
    fn from(s: &String) -> Self {
        Self::new(s)
    }
}

impl PartialEq for FlyString {
    fn eq(&self, other: &Self) -> bool {
        // Canonical interned (heap) strings share one allocation, so an
        // equal data pointer plus equal length settles them in O(1).
        // Inline strings have distinct addresses and fall through to a
        // byte compare — cheap, since they are at most INLINE_CAPACITY
        // bytes. A non-canonical long string (never produced by `new`)
        // would also take the byte-compare path and stay correct.
        if self.inner.as_ptr() == other.inner.as_ptr() && self.inner.len() == other.inner.len() {
            return true;
        }
        self.as_str() == other.as_str()
    }
}

impl Eq for FlyString {}

impl Hash for FlyString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // By content, so equal handles hash equally regardless of whether
        // they are inline or interned. Pointer hashing would break the
        // Hash/Eq contract for equal short strings.
        self.as_str().hash(state);
    }
}

impl PartialEq<str> for FlyString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for FlyString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_strings_are_not_interned() {
        let before = FlyString::interned_count();
        let _div = FlyString::new("div");
        let _span = FlyString::new("span");
        // Short strings live inline; the table should be untouched.
        assert_eq!(FlyString::interned_count(), before);
    }

    #[test]
    fn equal_content_compares_equal() {
        assert_eq!(FlyString::new("color"), FlyString::new("color"));
        assert_ne!(FlyString::new("color"), FlyString::new("background"));
    }

    #[test]
    fn long_strings_share_one_allocation() {
        // Two handles for the same long content must resolve to the same
        // ecow allocation, so the equal-pointer fast path fires.
        let long = "a-rather-long-attribute-value-that-spills-to-the-heap";
        let a = FlyString::new(long);
        let b = FlyString::new(long);
        assert_eq!(a, b);
        assert_eq!(a.as_ptr(), b.as_ptr());
    }

    #[test]
    fn eq_and_hash_agree_across_regimes() {
        use std::collections::hash_map::DefaultHasher;

        let hash_of = |s: &FlyString| {
            let mut h = DefaultHasher::new();
            s.hash(&mut h);
            h.finish()
        };

        // The same short content built twice must hash the same and
        // compare equal, even though their inline addresses differ.
        let a = FlyString::new("id");
        let b = FlyString::new("id");
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    #[test]
    fn shared_identity_across_threads() {
        // Long strings interned on different threads resolve to the same
        // canonical handle — the cross-thread identity the fast-path
        // pointer compare relies on.
        let long = "shared-long-token-that-definitely-spills-onto-the-heap";
        let handles: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(move || FlyString::new(long)))
            .collect();
        let from_main = FlyString::new(long);
        for handle in handles {
            let from_worker = handle.join().expect("worker thread panicked");
            assert_eq!(from_worker, from_main);
            assert_eq!(from_worker.as_ptr(), from_main.as_ptr());
        }
    }

    #[test]
    fn deref_and_helpers() {
        let tag = FlyString::new("DIV");
        // Deref gives str methods for free.
        assert_eq!(tag.to_ascii_lowercase(), "div");
        assert!(tag.equals_ignoring_ascii_case("div"));
        assert!(tag.is_one_of(&["span", "DIV", "p"]));
        assert!(!tag.is_one_of(&["span", "p"]));
    }
}
