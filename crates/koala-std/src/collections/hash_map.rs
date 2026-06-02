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
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::iter::{FromIterator, FusedIterator};
use core::mem;
use core::slice;

use crate::hash::FxBuildHasher;
use crate::raw::capacity_overflow;

use super::raw_table::{Bucket, RawTable};

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
    /// displacement, starting the walk at `(index, probe_length)`, and
    /// return the slot the originally-passed entry settled into.
    ///
    /// Does no equality checks — the caller has already established the key
    /// is not present. Does not adjust `len` (the caller does). Shared by
    /// `insert` and `resize_to`'s rehash (which both ignore the returned
    /// index) and by `insert_known_absent` (which uses it to hand the entry
    /// API a reference to the value just inserted).
    ///
    /// The passed entry settles at the *first* slot it is written to; every
    /// later write in the loop moves a displaced resident, not it. That
    /// first slot is the returned landing index.
    fn place_from(
        &mut self,
        mut index: usize,
        mut probe_length: usize,
        mut fragment: u32,
        mut entry: (K, V),
    ) -> usize {
        let mask = self.table.capacity() - 1;
        let mut landing = None;
        loop {
            // SAFETY: `index` is always masked with `& mask`, so it stays
            // `< capacity`.
            let bucket = unsafe { self.table.bucket_mut(index) };
            if bucket.is_empty() {
                bucket.init(probe_length, fragment, entry);
                return landing.unwrap_or(index);
            }
            let resident_pl = bucket.probe_length(index, mask);
            if resident_pl < probe_length {
                // SAFETY: reached only past the `is_empty()` check above, so
                // the slot is live and its entry is initialized.
                let (rich_fragment, rich_entry) = unsafe { bucket.take_occupied() };
                bucket.init(probe_length, fragment, entry); // incoming settles here
                if landing.is_none() {
                    landing = Some(index); // the incoming entry's final home
                }
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
                let _ = self.place_from(index, probe_length, fragment, (key, value));
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
        self.resize_to(new_capacity);
    }

    /// Reallocate the backing to exactly `new_capacity` buckets and re-home
    /// every entry into the fresh table.
    ///
    /// `new_capacity` must be a power of two `>= 8` (a `RawTable::grow_to`
    /// debug-assert enforces this) and large enough to hold the current
    /// entries under the load factor — every caller derives it from
    /// [`RawTable::buckets_for`], so this holds for both growth and
    /// shrinkage.
    ///
    /// Shared by [`grow`](Self::grow) (doubling), [`reserve`](Self::reserve),
    /// and [`shrink_to_fit`](Self::shrink_to_fit); they differ only in how
    /// they choose `new_capacity`. The rehash runs no user code — each home
    /// is recomputed from the cached `u32` fragment and displacement does no
    /// `K::eq` — so there is no panic point mid-loop and no drop guard is
    /// needed.
    fn resize_to(&mut self, new_capacity: usize) {
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
            let _ = self.place_from(home, 0, fragment, entry);
        }
        self.table.set_len(old.len());
    }

    /// Locate the bucket holding `key`, or `None` if it is absent.
    ///
    /// The shared lookup walk behind [`get`](Self::get),
    /// [`get_mut`](Self::get_mut), and [`contains_key`](Self::contains_key).
    /// Returns a bucket index rather than a reference so the three callers
    /// can re-borrow `self.table` at whatever mutability each needs — the
    /// immutable borrow taken here ends with the returned `usize`.
    ///
    /// This is the *search* half of [`insert`](Self::insert)'s loop with the
    /// displacement removed: it probes from the key's home and stops on one
    /// of three conditions — empty slot, a resident poorer than the current
    /// probe length (Robin Hood's absence guarantee), or a fragment-and-key
    /// match.
    fn find_index<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // STEP 1: an unallocated table holds nothing — answer before
        // computing `capacity - 1` (which would underflow at capacity 0).
        if self.table.is_empty() {
            return None;
        }
        // STEP 2: locate the home slot via `self.hash(key)` + `split_hash`.
        let mask = self.table.capacity() - 1;
        let (fragment, mut index) = Self::split_hash(self.hash(key), mask);
        let mut probe_length = 0;

        // STEP 3: probe forward. On each step decide between the three
        // terminating conditions (empty / richer resident / match) and the
        // "keep walking" case, masking the index and bumping the probe
        // length exactly as `insert`'s search loop does.
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
                        && unsafe { bucket.key() }.borrow() == key;
                    (false, resident_pl, matches)
                }
            };

            // cases 1 + 2: empty slot, or a richer resident → key absent,
            if is_empty || resident_pl < probe_length {
                return None;
            }

            // case 3: key already present → return the index.
            if key_matches {
                return Some(index);
            }

            // case 4: occupied, not ours, resident not richer → keep walking.
            index = (index + 1) & mask;
            probe_length += 1;
        }
    }

    /// Returns a reference to the value for `key`, or `None` if the key is
    /// not present.
    ///
    /// The key may be any borrowed form of the map's key type, as long as
    /// the borrowed form [`Hash`](core::hash::Hash) and [`Eq`](Eq) match
    /// those of the key type — so a `HashMap<String, _>` can be queried with
    /// a `&str` without allocating.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// assert_eq!(map.get("a"), Some(&1));
    /// assert_eq!(map.get("b"), None);
    /// ```
    ///
    /// # Time complexity
    ///
    /// Average *O*(1). Robin Hood hashing bounds the probe-length variance,
    /// so even the worst-case walk stays short for a well-distributed hash.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // SAFETY: `find_index` only returns indices of occupied buckets, so
        // the entry behind it is initialized.
        self.find_index(key)
            .map(|index| unsafe { self.table.bucket(index).value() })
    }

    /// Returns the stored key and its value for `key`, or `None` if not
    /// present.
    ///
    /// Like [`get`](Self::get) but also hands back the key as actually
    /// stored — the basis for [`HashSet::get`](super::HashSet::get) and
    /// for string interning, where the caller wants the canonical stored
    /// copy of an equal query.
    ///
    /// # Time complexity
    ///
    /// Average *O*(1).
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index(key).map(|index| {
            // SAFETY: `find_index` only returns indices of occupied buckets,
            // so the entry behind it is initialized.
            let bucket = unsafe { self.table.bucket(index) };
            unsafe { (bucket.key(), bucket.value()) }
        })
    }

    /// Returns a mutable reference to the value for `key`, or `None` if the
    /// key is not present.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// if let Some(v) = map.get_mut("a") {
    ///     *v += 10;
    /// }
    /// assert_eq!(map.get("a"), Some(&11));
    /// ```
    ///
    /// # Time complexity
    ///
    /// Average *O*(1), as for [`get`](Self::get).
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let index = self.find_index(key)?;
        // SAFETY: `find_index` only returns indices of occupied buckets, so
        // the entry behind it is initialized.
        Some(unsafe { self.table.bucket_mut(index).value_mut() })
    }

    /// Returns `true` if the map contains a value for `key`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// assert!(map.contains_key("a"));
    /// assert!(!map.contains_key("b"));
    /// ```
    ///
    /// # Time complexity
    ///
    /// Average *O*(1), as for [`get`](Self::get).
    #[must_use]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index(key).is_some()
    }

    /// Removes a key from the map, returning its value if the key was
    /// present.
    ///
    /// The key may be any borrowed form of the map's key type, with the
    /// same `Hash`/`Eq` correspondence as [`get`](Self::get).
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// assert_eq!(map.remove("a"), Some(1));
    /// assert_eq!(map.remove("a"), None);
    /// assert!(map.is_empty());
    /// ```
    ///
    /// # Time complexity
    ///
    /// Average *O*(1): the lookup plus a backshift over the deleted
    /// entry's displaced run, whose length Robin Hood keeps short.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // Locate the slot; a miss is `None`. The actual removal — move out,
        // backshift, account — lives in `remove_at`, shared with the entry
        // API's `OccupiedEntry::remove`.
        let index = self.find_index(key)?;
        Some(self.remove_at(index).1)
    }

    /// Remove the entry at a *known-occupied* `index`, returning the owned
    /// `(key, value)` pair. Shared by [`remove`](Self::remove) (which has
    /// just located the slot) and the entry API's `OccupiedEntry::remove`
    /// (which captured it at `entry` time).
    ///
    /// The entry is moved out *before* the backshift overwrites the slot:
    /// `take_occupied` leaves `raw_state` reading "occupied" but the bytes
    /// stale, and the backshift's first copy or the final `set_empty` is
    /// what makes the table consistent again. The moved-out pair is handed
    /// back to the caller only once the table is whole (backshift done, len
    /// decremented), so a `K`/`V` destructor that later panics never
    /// observes a stale-occupied slot.
    fn remove_at(&mut self, index: usize) -> (K, V) {
        // SAFETY: the caller guarantees `index` is occupied, so the slot is
        // live and its entry can be moved out exactly once.
        let (_frag, entry) = unsafe { self.table.bucket_mut(index).take_occupied() };
        self.backshift_from(index);
        self.table.set_len(self.table.len() - 1);
        entry
    }

    /// Insert a *known-absent* `(key, value)`, growing first if the table is
    /// at its load factor, and return the slot the value settled into.
    ///
    /// The caller must have already established the key is absent (the entry
    /// API does so when it returns a [`VacantEntry`]). Used by
    /// `VacantEntry::insert` to place the entry and then borrow the value
    /// out of its landing slot.
    fn insert_known_absent(&mut self, key: K, value: V) -> usize {
        if self.table.len() >= self.capacity() {
            self.grow();
        }
        let mask = self.table.capacity() - 1;
        let (fragment, index) = Self::split_hash(self.hash(&key), mask);
        let landed = self.place_from(index, 0, fragment, (key, value));
        self.table.set_len(self.table.len() + 1);
        landed
    }

    /// Gets the given key's corresponding entry for in-place manipulation.
    ///
    /// Probes once and returns a typed [`Entry`] capturing the result: an
    /// [`OccupiedEntry`] holding the located slot, or a [`VacantEntry`]
    /// holding the owned key ready to insert.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// // Insert-or-default, then accumulate in place.
    /// for word in ["a", "b", "a", "a"] {
    ///     *map.entry(word).or_insert(0) += 1;
    /// }
    /// assert_eq!(map.get("a"), Some(&3));
    /// assert_eq!(map.get("b"), Some(&1));
    /// ```
    ///
    /// # Time complexity
    ///
    /// Average *O*(1) to probe; a subsequent vacant insert is amortized
    /// *O*(1) (it may trigger a resize).
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, S> {
        // The lookup borrow ends with the returned `Option<usize>`, freeing
        // `self` to be moved into whichever entry variant we build.
        match self.find_index(&key) {
            Some(index) => Entry::Occupied(OccupiedEntry { map: self, index }),
            None => Entry::Vacant(VacantEntry { map: self, key }),
        }
    }

    /// Restore the Robin Hood invariant after the entry at `from` has been
    /// moved out, leaving its slot stale-occupied.
    ///
    /// Walks forward pulling each displaced resident (probe length > 0)
    /// one slot back toward its home, then marks the final vacated slot
    /// empty. This is decision #4's backshift: it stops at the first slot
    /// that is empty or already at home (probe length 0), because nothing
    /// past such a slot probed *through* `from`.
    ///
    /// The probe-length subtlety: an inline-encoded slot stores its probe
    /// length absolutely (independent of position), so moving it one slot
    /// back must *decrement* the stored value; the recompute-sentinel
    /// encoding derives the length from the slot index instead. Going
    /// through [`set_probe_length`](super::raw_table) with the
    /// freshly-read length keeps both encodings correct without a special
    /// case — read the moved resident's length, then re-encode it minus one
    /// at the destination.
    fn backshift_from(&mut self, mut from: usize) {
        let mask = self.table.capacity() - 1;
        // `from` is in bounds on entry (`find_index` returned it) and every
        // reassignment sets it to a masked `next`, so `from < capacity` holds
        // for the whole walk; every `next` is masked too. Both indices feeding
        // the `unsafe` accessors below are therefore always `< capacity`.
        loop {
            // STEP 1: walk forward from `from`. At each step look at `next`:
            //   - empty slot               → stop (nothing displaced past here)
            //   - resident at home (pl 0)  → stop (it never probed through us)
            //   - otherwise                → copy it back into the current slot,
            //     set its probe length to (its length − 1), advance.
            let next = (from + 1) & mask;
            // SAFETY: `next = (from + 1) & mask`, so `next <= mask < capacity`.
            if unsafe { self.table.bucket(next).is_empty() } {
                break;
            }
            // SAFETY: `next < capacity` (masked), and the slot is non-empty —
            // the `is_empty` check above would have broken otherwise — so
            // `probe_length` reads a live slot.
            if unsafe { self.table.bucket(next).probe_length(next, mask) } == 0 {
                break;
            }
            // SAFETY: both indices are `< capacity`; they differ because the
            // table holds at least 8 buckets, so `(from + 1) & mask != from`;
            // and `from` holds no live entry — it is either the slot `remove`
            // moved out of, or one already copied a step back earlier in this
            // walk — so the raw overwrite drops nothing.
            unsafe {
                self.table.copy_bucket(next, from);
            }
            // The entry now lives at `from`, one slot closer to home, so its
            // probe length drops by one. `next` is untouched by the copy, so
            // re-reading it yields the pre-move length; re-encoding it at
            // `from` is correct for both the inline and recompute encodings.
            // SAFETY: `next < capacity` (masked) and still live.
            let new_len = unsafe { self.table.bucket(next).probe_length(next, mask) } - 1;
            // SAFETY: `from < capacity` (loop invariant above).
            unsafe { self.table.bucket_mut(from).set_probe_length(new_len) }
            from = next;
        }
        // STEP 2: the slot the walk ended on is a stale duplicate of the entry
        // that moved back (or the original moved-out slot, if nothing shifted)
        // — mark it empty so it is not re-read as live.
        // SAFETY: `from < capacity` (loop invariant); the slot holds no live
        // entry, so emptying it drops nothing.
        unsafe { self.table.bucket_mut(from).set_empty() };
    }

    /// Reserves room for at least `additional` more entries to be inserted
    /// without reallocating.
    ///
    /// After this call the map can hold `len() + additional` entries before
    /// the next resize. If it already can, this is a no-op.
    ///
    /// # Panics
    ///
    /// Panics if the new entry count overflows `usize`, or if the derived
    /// bucket count exceeds what the allocator can serve.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map: HashMap<i32, i32> = HashMap::new();
    /// map.reserve(100);
    /// let cap = map.capacity();
    /// assert!(cap >= 100);
    /// // Filling up to the reserved capacity does not resize again.
    /// for i in 0..cap as i32 {
    ///     map.insert(i, i);
    /// }
    /// assert_eq!(map.capacity(), cap);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) when a resize happens (every current entry is re-homed),
    /// *O*(1) when the capacity already suffices.
    pub fn reserve(&mut self, additional: usize) {
        // STEP 1: the entry count we must be able to hold without resizing.
        // A `usize` overflow here means an unserviceable request, so it
        // routes through the same panic path as the rest of the capacity math.
        let Some(required) = self.len().checked_add(additional) else {
            capacity_overflow();
        };

        // STEP 2: if the current *entry* capacity already covers that, there
        // is nothing to do — `reserve` never shrinks.
        if self.capacity() >= required {
            return;
        }

        // STEP 3: otherwise resize to the bucket count `RawTable::buckets_for`
        // derives for the target entry count.
        self.resize_to(RawTable::<K, V>::buckets_for(required));
    }

    /// Shrinks the capacity of the map as much as possible while still
    /// holding its current entries under the load factor.
    ///
    /// The backing is reduced to the smallest power-of-two bucket count
    /// (minimum 8) that holds [`len`](Self::len) entries. A map already at
    /// that size is left untouched.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map: HashMap<i32, i32> = HashMap::with_capacity(1000);
    /// map.insert(1, 1);
    /// let before = map.capacity();
    /// map.shrink_to_fit();
    /// assert!(map.capacity() <= before);
    /// assert_eq!(map.get(&1), Some(&1));
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) when a resize happens, *O*(1) otherwise.
    pub fn shrink_to_fit(&mut self) {
        // STEP 1: the smallest backing that still holds `len()` entries.
        let target_buckets = RawTable::<K, V>::buckets_for(self.table.len());

        // STEP 2: resize only if that is strictly smaller than the current
        // bucket count (`self.table.capacity()`, the bucket count — not the
        // entry capacity `self.capacity()`).
        if target_buckets < self.table.capacity() {
            self.resize_to(target_buckets);
        }
    }
}

/// Iteration accessors and bulk operations that walk the backing without
/// hashing, so they need no bound on `K` or `S`.
impl<K, V, S> HashMap<K, V, S> {
    /// Removes all entries, dropping each one, but keeps the allocated
    /// capacity for reuse.
    ///
    /// After `clear`, [`len`](Self::len) is `0` while
    /// [`capacity`](Self::capacity) is unchanged — matching `std`, so a
    /// cleared map can be refilled without reallocating.
    ///
    /// Needs no bound on `K` or `S`: clearing drops entries and resets
    /// slots by reading each bucket's state, never by hashing a key. That
    /// is what lets `HashSet::clear` — which has no `Hash`/`BuildHasher`
    /// bounds either — delegate straight to it.
    ///
    /// # Time complexity
    ///
    /// *O*(*capacity*): every bucket is visited to drop the live entries
    /// and reset its state, regardless of how many were occupied.
    pub fn clear(&mut self) {
        self.table.clear();
    }

    /// An iterator visiting every entry as `(&K, &V)`, in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// // Order is unspecified, so check order-independent facts.
    /// assert_eq!(map.iter().count(), 2);
    /// let sum: i32 = map.iter().map(|(_, v)| *v).sum();
    /// assert_eq!(sum, 3);
    /// ```
    ///
    /// # Time complexity
    ///
    /// Construction is *O*(1); a full walk is *O*(*capacity*) — every
    /// bucket is visited, empty ones skipped.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            buckets: self.table.as_slice().iter(),
            remaining: self.table.len(),
        }
    }

    /// An iterator visiting every entry as `(&K, &mut V)`, in arbitrary
    /// order, allowing the values to be modified in place.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// for (_, v) in map.iter_mut() {
    ///     *v *= 10;
    /// }
    /// assert_eq!(map.get("a"), Some(&10));
    /// assert_eq!(map.get("b"), Some(&20));
    /// ```
    ///
    /// # Time complexity
    ///
    /// Construction is *O*(1); a full walk is *O*(*capacity*).
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        let remaining = self.table.len();
        IterMut {
            buckets: self.table.as_mut_slice().iter_mut(),
            remaining,
        }
    }

    /// An iterator visiting every key as `&K`, in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// let sum: usize = map.keys().map(|k| k.len()).sum();
    /// assert_eq!(sum, 2);
    /// ```
    ///
    /// # Time complexity
    ///
    /// Construction is *O*(1); a full walk is *O*(*capacity*).
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys { inner: self.iter() }
    }

    /// An iterator visiting every value as `&V`, in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// let sum: i32 = map.values().sum();
    /// assert_eq!(sum, 3);
    /// ```
    ///
    /// # Time complexity
    ///
    /// Construction is *O*(1); a full walk is *O*(*capacity*).
    pub fn values(&self) -> Values<'_, K, V> {
        Values { inner: self.iter() }
    }

    /// An iterator visiting every value as `&mut V`, in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// for v in map.values_mut() {
    ///     *v += 100;
    /// }
    /// assert_eq!(map.get("a"), Some(&101));
    /// ```
    ///
    /// # Time complexity
    ///
    /// Construction is *O*(1); a full walk is *O*(*capacity*).
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut {
            inner: self.iter_mut(),
        }
    }
}

/// A borrowed iterator over a [`HashMap`]'s entries, yielding `(&K, &V)`.
///
/// Created by [`HashMap::iter`] (and by `&HashMap`'s [`IntoIterator`]).
/// The order is arbitrary and must not be relied upon.
#[derive(Clone)]
pub struct Iter<'a, K, V> {
    /// Cursor over the whole backing; `next` skips the empty buckets.
    buckets: slice::Iter<'a, Bucket<K, V>>,
    /// Live entries not yet yielded — drives the exact `size_hint`.
    remaining: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        // Advance the slice cursor until a live bucket appears; `by_ref` keeps
        // the cursor's position across calls. The empties between entries are
        // skipped here, not by the caller.
        for bucket in self.buckets.by_ref() {
            if !bucket.is_empty() {
                self.remaining -= 1;
                // SAFETY: the slot is live (the `is_empty` check above), so its
                // entry is initialized and `key`/`value` read valid data. Each
                // `&Bucket` the slice cursor yields carries the `'a` lifetime, so
                // the returned `(&'a K, &'a V)` cannot outlive the borrowed map.
                return Some(unsafe { (bucket.key(), bucket.value()) });
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> FusedIterator for Iter<'_, K, V> {}

/// A borrowed iterator over a [`HashMap`]'s entries, yielding
/// `(&K, &mut V)`.
///
/// Created by [`HashMap::iter_mut`] (and by `&mut HashMap`'s
/// [`IntoIterator`]). The order is arbitrary.
pub struct IterMut<'a, K, V> {
    buckets: slice::IterMut<'a, Bucket<K, V>>,
    remaining: usize,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        // STEP: the same skip-empty walk as `Iter::next`, but materialize the
        // pair through `Bucket::key_value_mut` — `key()` + `value_mut()`
        // can't both borrow the bucket at once, which is the whole reason
        // that split primitive exists.
        for bucket in self.buckets.by_ref() {
            if !bucket.is_empty() {
                self.remaining -= 1;
                // SAFETY: the slot is live (the `is_empty` check above), so its
                // entry is initialized. `key_value_mut` splits the one
                // `&mut (K, V)` into references to disjoint fields, so the `&K`
                // and `&mut V` do not alias. The slice cursor yields
                // `&'a mut Bucket`, so the pair carries `'a` and cannot outlive
                // the borrowed map.
                return Some(unsafe { bucket.key_value_mut() });
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}
impl<K, V> FusedIterator for IterMut<'_, K, V> {}

/// A consuming iterator over a [`HashMap`], yielding owned `(K, V)` pairs.
///
/// Created by `HashMap`'s [`IntoIterator`]. It owns the backing table and
/// yields entries by moving them out; any entries left unyielded when the
/// `IntoIter` drops are destroyed by the table's own `Drop` — so this type
/// needs no `Drop` of its own, provided each yielded slot is marked empty
/// as it is taken.
pub struct IntoIter<K, V> {
    /// The moved-out backing; `next` drains it slot by slot.
    table: RawTable<K, V>,
    /// Next bucket index to inspect.
    index: usize,
    /// Live entries not yet yielded — drives the exact `size_hint`.
    remaining: usize,
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.table.capacity() {
            let i = self.index;
            self.index += 1;
            // SAFETY: `i < self.table.capacity()` (the loop condition).
            let bucket = unsafe { self.table.bucket_mut(i) };
            if bucket.is_empty() {
                continue;
            }
            // SAFETY: the slot is live (the `is_empty` check above), so its
            // entry is initialized and can be moved out exactly once.
            let (_fragment, entry) = unsafe { bucket.take_occupied() };
            // Mark the slot empty so the table's `Drop` does not re-drop the
            // entry we just moved out — `take_occupied` leaves `raw_state`
            // reading "occupied", and `IntoIter` has no `Drop` of its own.
            bucket.set_empty();
            self.remaining -= 1;
            return Some(entry);
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V> ExactSizeIterator for IntoIter<K, V> {}
impl<K, V> FusedIterator for IntoIter<K, V> {}

/// Consuming iteration moves every entry out of the map.
impl<K, V, S> IntoIterator for HashMap<K, V, S> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> IntoIter<K, V> {
        // Move the backing out; the hasher `S` is dropped here. `HashMap` has
        // no `Drop`, so this destructuring move is allowed.
        let Self { table, .. } = self;
        let remaining = table.len();
        IntoIter {
            table,
            index: 0,
            remaining,
        }
    }
}

/// `for (k, v) in &map` — borrowed iteration, equivalent to [`HashMap::iter`].
impl<'a, K, V, S> IntoIterator for &'a HashMap<K, V, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}

/// `for (k, v) in &mut map` — mutable iteration, equivalent to
/// [`HashMap::iter_mut`].
impl<'a, K, V, S> IntoIterator for &'a mut HashMap<K, V, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> IterMut<'a, K, V> {
        self.iter_mut()
    }
}

/// An iterator over a [`HashMap`]'s keys, yielding `&K`.
///
/// Created by [`HashMap::keys`]. A thin projection of [`Iter`] onto the
/// key half; order is arbitrary.
#[derive(Clone)]
pub struct Keys<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        // Delegate to the entry iterator and keep the key half.
        self.inner.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Keys<'_, K, V> {}
impl<K, V> FusedIterator for Keys<'_, K, V> {}

/// An iterator over a [`HashMap`]'s values, yielding `&V`.
///
/// Created by [`HashMap::values`]. A thin projection of [`Iter`] onto the
/// value half; order is arbitrary.
#[derive(Clone)]
pub struct Values<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        // Delegate to the entry iterator and keep the value half.
        self.inner.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Values<'_, K, V> {}
impl<K, V> FusedIterator for Values<'_, K, V> {}

/// A mutable iterator over a [`HashMap`]'s values, yielding `&mut V`.
///
/// Created by [`HashMap::values_mut`]. A thin projection of [`IterMut`]
/// onto the value half; order is arbitrary.
pub struct ValuesMut<'a, K, V> {
    inner: IterMut<'a, K, V>,
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        // Delegate to the mutable entry iterator and keep the value half.
        self.inner.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {}
impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}

/// Clones the map, preserving its bucket layout — the backing
/// [`RawTable`] is copied structurally (no rehash), so only `K` and `V`
/// need be `Clone` (plus the hasher).
impl<K: Clone, V: Clone, S: Clone> Clone for HashMap<K, V, S> {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
            hasher: self.hasher.clone(),
        }
    }
}

/// Formats the map as `{k: v, …}` in arbitrary order, like `std`'s
/// `HashMap`.
impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for HashMap<K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// Two maps are equal when they hold the same set of key/value pairs,
/// regardless of insertion order or internal layout.
impl<K: Eq + Hash, V: PartialEq, S: BuildHasher> PartialEq for HashMap<K, V, S> {
    fn eq(&self, other: &Self) -> bool {
        // Equal length plus "every entry of `self` is in `other` with the
        // same value" implies set equality — no key can be in `other` but
        // not `self` once the counts match.
        self.len() == other.len()
            && self.iter().all(|(key, value)| {
                other
                    .get(key)
                    .is_some_and(|other_value| *value == *other_value)
            })
    }
}

impl<K: Eq + Hash, V: Eq, S: BuildHasher> Eq for HashMap<K, V, S> {}

/// Inserts every pair from the iterator, overwriting existing keys.
impl<K: Eq + Hash, V, S: BuildHasher> Extend<(K, V)> for HashMap<K, V, S> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        let iter = iter.into_iter();
        // Pre-reserve for the iterator's lower bound to avoid repeated grows.
        self.reserve(iter.size_hint().0);
        for (key, value) in iter {
            let _ = self.insert(key, value);
        }
    }
}

/// Inserts every pair from an iterator of references, for `map.extend(&other)`.
impl<'a, K: Eq + Hash + Copy, V: Copy, S: BuildHasher> Extend<(&'a K, &'a V)> for HashMap<K, V, S> {
    fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
        self.extend(iter.into_iter().map(|(&key, &value)| (key, value)));
    }
}

/// Builds a map from an iterator of pairs, using the default hasher.
impl<K: Eq + Hash, V, S: BuildHasher + Default> FromIterator<(K, V)> for HashMap<K, V, S> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut map = Self::with_hasher(S::default());
        map.extend(iter);
        map
    }
}

/// A view into a single entry in a [`HashMap`], which may be occupied or
/// vacant. Constructed by [`HashMap::entry`].
///
/// The `S` parameter rides along from the map the entry borrows; it
/// defaults to [`FxBuildHasher`] like `HashMap` itself.
pub enum Entry<'a, K, V, S = FxBuildHasher> {
    /// The key is present in the map.
    Occupied(OccupiedEntry<'a, K, V, S>),
    /// The key is absent; the entry owns it, ready to insert.
    Vacant(VacantEntry<'a, K, V, S>),
}

/// A view into an occupied entry in a [`HashMap`]. Part of [`Entry`].
pub struct OccupiedEntry<'a, K, V, S = FxBuildHasher> {
    map: &'a mut HashMap<K, V, S>,
    /// The located bucket. It stays valid for the entry's whole lifetime
    /// because the exclusive `&mut map` borrow prevents any mutation that
    /// could move or vacate the slot.
    index: usize,
}

/// A view into a vacant entry in a [`HashMap`]. Part of [`Entry`].
///
/// Holds the owned key rather than a slot: inserting may grow the table
/// and re-home everything, so any pre-located slot would be invalid by
/// the time the value goes in.
pub struct VacantEntry<'a, K, V, S = FxBuildHasher> {
    map: &'a mut HashMap<K, V, S>,
    key: K,
}

impl<'a, K, V, S> Entry<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Ensures a value is in the entry, inserting `default` if vacant, and
    /// returns a mutable reference to the resident value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// *map.entry("a").or_insert(1) += 10;
    /// assert_eq!(map.get("a"), Some(&11));
    /// ```
    ///
    /// # Time complexity
    ///
    /// Amortized *O*(1).
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Like [`or_insert`](Self::or_insert), but the default is computed
    /// lazily, only when the entry is vacant.
    ///
    /// # Time complexity
    ///
    /// Amortized *O*(1) plus the cost of `default`.
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Like [`or_insert_with`](Self::or_insert_with), but the closure is
    /// handed the key being inserted.
    ///
    /// # Time complexity
    ///
    /// Amortized *O*(1) plus the cost of `default`.
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    /// Returns a reference to this entry's key — the resident key if
    /// occupied, the to-be-inserted key if vacant.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub fn key(&self) -> &K {
        match self {
            Entry::Occupied(entry) => entry.key(),
            Entry::Vacant(entry) => entry.key(),
        }
    }

    /// Runs `f` on the value if the entry is occupied, then returns the
    /// entry, so it composes ahead of [`or_insert`](Self::or_insert).
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// map.insert("a", 1);
    /// map.entry("a").and_modify(|v| *v += 100).or_insert(0);
    /// map.entry("b").and_modify(|v| *v += 100).or_insert(0);
    /// assert_eq!(map.get("a"), Some(&101));
    /// assert_eq!(map.get("b"), Some(&0));
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1) plus the cost of `f`.
    #[must_use]
    pub fn and_modify<F: FnOnce(&mut V)>(mut self, f: F) -> Self {
        if let Entry::Occupied(entry) = &mut self {
            f(entry.get_mut());
        }
        self
    }
}

impl<'a, K, V, S> Entry<'a, K, V, S>
where
    K: Hash + Eq,
    V: Default,
    S: BuildHasher,
{
    /// Ensures a value is in the entry, inserting `V::default()` if vacant.
    ///
    /// # Time complexity
    ///
    /// Amortized *O*(1).
    pub fn or_default(self) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(V::default()),
        }
    }
}

impl<'a, K, V, S> OccupiedEntry<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// A reference to the key in the map for this entry.
    #[must_use]
    pub fn key(&self) -> &K {
        // SAFETY: `index` was located by `entry` and the exclusive `&mut map`
        // borrow has prevented any mutation that could vacate it, so the slot
        // is still live.
        unsafe { self.map.table.bucket(self.index).key() }
    }

    /// A reference to the value.
    #[must_use]
    pub fn get(&self) -> &V {
        // SAFETY: as in `key` — the captured slot is still occupied.
        unsafe { self.map.table.bucket(self.index).value() }
    }

    /// A mutable reference to the value, borrowed for the entry.
    pub fn get_mut(&mut self) -> &mut V {
        // SAFETY: as in `key` — the captured slot is still occupied.
        unsafe { self.map.table.bucket_mut(self.index).value_mut() }
    }

    /// Converts the entry into a mutable reference to the value, with the
    /// map's lifetime rather than the entry's.
    #[must_use]
    pub fn into_mut(self) -> &'a mut V {
        // SAFETY: as in `key` — the captured slot is still occupied.
        unsafe { self.map.table.bucket_mut(self.index).value_mut() }
    }

    /// Sets the value of the entry, returning the previous value.
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Removes the entry, returning the value.
    #[must_use]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Removes the entry, returning the owned key and value.
    #[must_use]
    pub fn remove_entry(self) -> (K, V) {
        self.map.remove_at(self.index)
    }
}

impl<'a, K, V, S> VacantEntry<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// A reference to the key that would be inserted.
    #[must_use]
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Takes ownership of the key, leaving the map unchanged.
    #[must_use]
    pub fn into_key(self) -> K {
        self.key
    }

    /// Inserts the entry's key with `value`, returning a mutable reference
    /// to the value in its landing slot.
    pub fn insert(self, value: V) -> &'a mut V {
        let index = self.map.insert_known_absent(self.key, value);
        // SAFETY: `insert_known_absent` just placed the entry and returned the
        // occupied slot it settled into, so the value there is initialized;
        // `self.map` is `&'a mut`, so the reference carries the map's lifetime.
        unsafe { self.map.table.bucket_mut(index).value_mut() }
    }
}
