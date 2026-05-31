//! The public `HashMap<K, V, S>` type.
//!
//! A hand-rolled hash map using Robin Hood hashing over the private
//! [`RawTable`](super::raw_table::RawTable) backing. The full design —
//! the seven locked decisions (algorithm, hash function, load factor,
//! deletion strategy, storage layout, cached fragment, API shape) —
//! lives in `project-memory/koala-std-hashmap-design.md`.
//!
//! `HashMap` owns everything `RawTable` deliberately does not: the
//! hasher, the probe sequences, Robin Hood displacement, the 70% load
//! factor, and backshift deletion. It reaches into the backing through
//! the bucket primitives added in Phase 3a.
//!
//! # Default hasher
//!
//! The default `S` is [`FxBuildHasher`] — fast and good-quality for
//! non-adversarial keys, which is koala's entire workload (internal
//! containers, no untrusted input path). Callers who need `DoS`
//! resistance can supply any [`BuildHasher`](core::hash::BuildHasher)
//! via [`with_hasher`](HashMap::with_hasher).
//!
//! # Current state
//!
//! Phase 3b-i: the struct, the four constructors, and the size
//! accessors (`len` / `is_empty` / `capacity`). The probing methods —
//! `insert` (3b-ii), `get` family (3b-iii), and `remove` (3b-iv) — do
//! not exist yet.

use crate::hash::FxBuildHasher;

use super::raw_table::RawTable;

/// A hash map with Robin Hood probing and a default 70% load factor.
///
/// `K` must be [`Hash`](core::hash::Hash) + [`Eq`](Eq) for the probing
/// methods; the constructors and size accessors impose no bounds. `V`
/// is unconstrained. `S` is the [`BuildHasher`](core::hash::BuildHasher)
/// factory, defaulting to [`FxBuildHasher`].
///
/// The map stores its entries in a single [`RawTable`] allocation and
/// holds the hasher factory directly — there is no
/// `BuildHasherDefault` re-wrapping, because [`FxBuildHasher`] is
/// already a `BuildHasher`.
pub struct HashMap<K, V, S = FxBuildHasher> {
    table: RawTable<K, V>,
    hasher: S,
}

impl<K, V> HashMap<K, V, FxBuildHasher> {
    /// Creates an empty `HashMap` with the default [`FxBuildHasher`].
    ///
    /// Allocates nothing — the first allocation is deferred to the
    /// first insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let map: HashMap<&str, i32> = HashMap::new();
    /// assert!(map.is_empty());
    /// assert_eq!(map.capacity(), 0);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            table: RawTable::new(),
            hasher: FxBuildHasher::new(),
        }
    }

    /// Creates an empty `HashMap` with the default hasher and room for
    /// at least `capacity` entries before the first resize.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let map: HashMap<i32, i32> = HashMap::with_capacity(10);
    /// assert!(map.is_empty());
    /// assert!(map.capacity() >= 10);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) in the derived bucket count, dominated by zeroing the
    /// backing array.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            table: RawTable::with_capacity(capacity),
            hasher: FxBuildHasher::default(),
        }
    }
}

/// Equivalent to [`HashMap::new`] — an empty map with the default
/// [`FxBuildHasher`] and no allocation.
impl<K, V> Default for HashMap<K, V, FxBuildHasher> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, S> HashMap<K, V, S> {
    /// Creates an empty `HashMap` that will use `hasher` to hash keys.
    ///
    /// Allocates nothing until the first insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// # use koala_std::hash::FxBuildHasher;
    /// let map: HashMap<&str, i32> = HashMap::with_hasher(FxBuildHasher::default());
    /// assert!(map.is_empty());
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            table: RawTable::new(),
            hasher,
        }
    }

    /// Creates an empty `HashMap` with room for at least `capacity`
    /// entries, that will use `hasher` to hash keys.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// # use koala_std::hash::FxBuildHasher;
    /// let map: HashMap<i32, i32> =
    ///     HashMap::with_capacity_and_hasher(10, FxBuildHasher::default());
    /// assert!(map.capacity() >= 10);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) in the derived bucket count.
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            table: RawTable::with_capacity(capacity),
            hasher,
        }
    }

    /// Returns the number of entries in the map.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Returns `true` if the map contains no entries.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// Returns the number of entries the map can hold before its next
    /// resize.
    ///
    /// This is an *entry* count, not a bucket count: it is the backing
    /// array's bucket count scaled by the 70% load factor, matching
    /// `std`'s element-capacity semantics. A freshly-[`new`](Self::new)
    /// map reports `0` (it has not allocated).
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.table.capacity() * 7 / 10
    }
}
