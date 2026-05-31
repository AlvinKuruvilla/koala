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

use core::borrow::Borrow;
use core::hash::{BuildHasher, Hash};
use core::mem;

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
        Self::load_capacity(self.table.capacity())
    }

    /// The 70%-load entry threshold for a backing of `buckets` buckets:
    /// the most entries the map holds before the next insert resizes it.
    ///
    /// Shared by [`capacity`](Self::capacity) (what it reports) and the
    /// `insert` resize check (when it grows), so the two can never
    /// disagree. For power-of-two bucket counts `buckets * 7 / 10` is
    /// always strictly below `buckets * 0.7` (it is never an exact
    /// multiple of 10), so it is exactly the largest `len` satisfying the
    /// canonical `len * 100 < buckets * 70`.
    #[inline]
    fn load_capacity(buckets: usize) -> usize {
        buckets * 7 / 10
    }
}

impl<K, V, S> HashMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// The full 64-bit hash of a key, or any borrowed form of it, using a
    /// fresh hasher from the build-hasher factory.
    ///
    /// Generic over `Q` so the lookup methods can hash a borrowed query
    /// — e.g. a `&str` against a `HashMap<String, V>` — and get the same
    /// hash the owned key produced. The `Borrow` contract guarantees that
    /// equality of hashes.
    fn hash<Q>(&self, key: &Q) -> u64
    where
        K: Borrow<Q>,
        Q: Hash + ?Sized,
    {
        self.hasher.hash_one(key)
    }

    /// Split a full hash into the cached `u32` fragment and the home bucket
    /// index for a backing of `mask + 1` buckets.
    ///
    /// Both casts truncate by design: the fragment *is* the low 32 bits of
    /// the hash, and the home is `hash & mask` where `mask < 2^32`, so the
    /// masked value always fits the destination. There is no checked
    /// conversion that expresses "the low bits," so the truncation is
    /// documented here once rather than re-justified at every probe site.
    #[allow(clippy::cast_possible_truncation)]
    fn split_hash(hash: u64, mask: usize) -> (u32, usize) {
        (hash as u32, (hash as usize) & mask)
    }

    /// Place a *known-absent* entry into the table by Robin Hood
    /// displacement, starting the walk at `(index, probe_length)`.
    ///
    /// Does no equality checks — the caller has already established the key
    /// is not present. Does not adjust `len` (the caller does). Shared by
    /// `insert` (which hands off here once it has located the insertion
    /// point) and `grow`'s rehash (which calls it at each entry's home).
    fn place_from(
        &mut self,
        mut index: usize,
        mut probe_length: usize,
        mut fragment: u32,
        mut entry: (K, V),
    ) {
        let mask = self.table.capacity() - 1;
        loop {
            // SAFETY: `index` is always masked with `& mask`, so it stays
            // `< capacity`.
            let bucket = unsafe { self.table.bucket_mut(index) };
            if bucket.is_empty() {
                bucket.init(probe_length, fragment, entry);
                return;
            }
            let resident_pl = bucket.probe_length(index, mask);
            if resident_pl < probe_length {
                // SAFETY: reached only past the `is_empty()` check above, so
                // the slot is live and its entry is initialized.
                let (rich_fragment, rich_entry) = unsafe { bucket.take_occupied() };
                bucket.init(probe_length, fragment, entry); // incoming settles here
                // carry the evicted (richer) resident onward; it sat at
                // probe length resident_pl, so it resumes from there.
                fragment = rich_fragment;
                entry = rich_entry;
                probe_length = resident_pl;
            }
            index = (index + 1) & mask;
            probe_length += 1;
        }
    }

    /// Inserts a key/value pair, returning the previous value for the key
    /// if it was already present.
    ///
    /// On an existing key the value is overwritten and the old value
    /// returned; the original key is kept (matching `std`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// assert_eq!(map.insert("a", 1), None);
    /// assert_eq!(map.insert("a", 2), Some(1));
    /// assert_eq!(map.len(), 1);
    /// ```
    ///
    /// # Time complexity
    ///
    /// Amortized *O*(1). A resize (triggered when the table reaches its
    /// 70% load factor) is *O*(*n*) but happens on at most a `1/n`
    /// fraction of inserts, so the amortized cost stays constant.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // STEP 1: grow if full (also handles the empty-table first insert: 0 >= 0).
        if self.table.len() >= self.capacity() {
            self.grow();
        }

        // STEP 2: locate the home slot.
        let mask = self.table.capacity() - 1;
        let (fragment, mut index) = Self::split_hash(self.hash(&key), mask);
        let mut probe_length = 0;

        // STEP 3: search-or-insert walk.
        loop {
            let (is_empty, resident_pl, key_matches) = {
                // SAFETY: `index` is always masked with `& mask`, so `< capacity`.
                let bucket = unsafe { self.table.bucket(index) };
                if bucket.is_empty() {
                    (true, 0, false)
                } else {
                    let resident_pl = bucket.probe_length(index, mask);
                    // Only do the key compare when the
                    // resident is poor enough to *be* our key AND the cheap
                    // fragment matches.
                    let matches = resident_pl >= probe_length
                        && bucket.hash_fragment() == fragment
                        && unsafe { bucket.key() } == &key;
                    (false, resident_pl, matches)
                }
            };

            // cases 1 + 2: empty slot, or a richer resident → key absent,
            // insert here.
            if is_empty || resident_pl < probe_length {
                self.place_from(index, probe_length, fragment, (key, value));
                self.table.set_len(self.table.len() + 1);
                return None;
            }

            // case 3: key already present → overwrite value, keep key, return old.
            if key_matches {
                return Some(mem::replace(
                    unsafe { self.table.bucket_mut(index).value_mut() },
                    value,
                ));
            }

            // case 4: occupied, not ours, resident not richer → keep walking.
            index = (index + 1) & mask;
            probe_length += 1;
        }
    }

    /// Double the backing (or allocate the initial 8 buckets) and re-home
    /// every entry into it.
    fn grow(&mut self) {
        let new_capacity = if self.table.capacity() == 0 {
            8
        } else {
            self.table.capacity() * 2
        };
        let mut old = self.table.grow_to(new_capacity);
        // Now `self.table` is the new, empty backing; `old` still owns every entry.
        let new_mask = new_capacity - 1;
        for i in 0..old.capacity() {
            // SAFETY: `i` is in `0..old.capacity()`, so it is a valid index.
            if unsafe { old.bucket(i) }.is_empty() {
                continue;
            }
            // SAFETY: the slot is live (checked just above), so its entry is
            // initialized and can be moved out exactly once.
            let (fragment, entry) = unsafe { old.bucket_mut(i).take_occupied() };
            // Mark the old slot empty so `old`'s Drop does not re-drop the
            // entry we just moved out (use-after-free otherwise).
            // SAFETY: `i` is a valid index.
            unsafe { old.bucket_mut(i) }.set_empty();
            // `fragment as usize` is a widening cast (no truncation). With
            // `new_mask < 2^32`, the home derived from the fragment equals the
            // one the full hash would give, so no re-hashing is needed.
            let home = (fragment as usize) & new_mask;
            self.place_from(home, 0, fragment, entry);
        }
        self.table.set_len(old.len());
    }
}
