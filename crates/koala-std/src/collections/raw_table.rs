//! Private storage primitive for the Robin Hood hash table.
//!
//! This module owns the three types that back
//! `koala_std::collections::HashMap`:
//!
//! - `BucketState` — a typed view of a bucket's encoded state
//!   byte. Three variants: `Empty`, `OccupiedInline` (with the
//!   decoded probe length), `OccupiedRecompute` (sentinel for
//!   pathologically long probe chains whose lengths exceed the
//!   inline encoding's byte budget).
//! - `Bucket<K, V>` — one slot in the bucket array. Holds the
//!   encoded state byte, a cached `u32` hash fragment, and a
//!   `MaybeUninit<(K, V)>` for the entry. No `Drop` impl — the
//!   wrapping `RawTable` is responsible for destructing every
//!   live entry when the table itself is dropped.
//! - `RawTable<K, V>` — the allocation-owning container. Knows
//!   how to allocate, grow, and drop a bucket array. Does NOT
//!   know about hashing, probe sequences, Robin Hood
//!   displacement, or load factors — those all live on
//!   `HashMap`, which reaches into a `RawTable` via
//!   `bucket` / `bucket_mut` for slot access.
//!
//! The storage-vs-probing split is deliberate and mirrors the
//! `RawVec<T>` / `Vec<T>` relationship in `alloc::raw_vec`. The
//! full design — field semantics, invariants, drop contract,
//! panic-safety rationale, and the exact probe-length recovery
//! formula used when `BucketState::OccupiedRecompute` is hit —
//! lives in `project-memory/koala-std-hashmap-design.md` under
//! the "Phase 2 struct design" section. That document is the
//! source of truth; this module implements it.

use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::{self, NonNull};
use core::slice;

use crate::raw::{alloc_array, capacity_overflow, dealloc_array};

/// Typed view of a bucket's state byte.
///
/// A bucket's `raw_state: u8` is a compact
/// encoding: `0` for an empty slot, `1..=254` for an occupied
/// slot with the probe length stored inline (`raw_state - 1`),
/// and `255` as a sentinel for the rare case where the probe
/// length exceeds the inline encoding's byte budget. This
/// enum is the decoded form — every caller that needs to
/// reason about "is this slot live, and if so how far has it
/// been displaced" goes through `Bucket::state` rather than
/// matching on the raw byte.
pub(super) enum BucketState {
    /// This slot holds no entry. The `entry` field is
    /// uninitialized and must not be read. Either the slot is
    /// fresh from allocation and has never been used, or it
    /// was vacated by a delete whose backshift chain
    /// terminated here. The probe loop treats `Empty` as a
    /// hard stop — a lookup that hits an empty slot concludes
    /// that the key is not in the table.
    Empty,

    /// This slot holds a live `(K, V)` entry, and its probe
    /// length fits in the inline 0..=253 range. The wrapped
    /// `usize` is the probe length: the number of slots this
    /// entry sits past its home position (`hash & mask`). A
    /// probe length of 0 means the entry is exactly at its
    /// home; 1 means one slot past its home; etc. This is the
    /// common case — at 70% load factor with `FxHash`,
    /// essentially every live bucket is `OccupiedInline`.
    OccupiedInline(usize),

    /// This slot holds a live `(K, V)` entry, but its probe
    /// length exceeds 253 and cannot be encoded in the inline
    /// byte range. Callers that need the probe length must
    /// recompute it from the bucket index and the cached
    /// `hash_fragment`:
    ///
    /// ```text
    /// home_index   = (hash_fragment as usize) & mask
    /// probe_length = bucket_index.wrapping_sub(home_index) & mask
    /// ```
    ///
    /// `wrapping_sub` is load-bearing: when an entry has
    /// wrapped around the end of the bucket array, plain
    /// unsigned subtraction (`bucket_index - home_index`)
    /// panics in debug builds on underflow. `wrapping_sub`
    /// is a no-op in release and wraps explicitly in debug;
    /// the subsequent `& mask` folds the wrapped value back
    /// into the `0..capacity` probe-length range.
    ///
    /// Using the 32-bit fragment is sufficient because `mask`
    /// is always less than `2^32` in practice — a
    /// `usize::MAX`-capacity `HashMap` is not a thing we
    /// support.
    ///
    /// This variant is pathological: a Robin Hood table at
    /// 70% load with `FxHash` essentially never produces probe
    /// chains longer than 253. The sentinel exists to keep
    /// the inline encoding's byte budget honest under
    /// degenerate input, not as an expected branch.
    OccupiedRecompute,
}

/// One slot in the Robin Hood bucket array.
///
/// Holds three pieces of information:
///
/// - `raw_state` — the encoded state byte whose decoded
///   form is `BucketState`. A value of `0` means the slot
///   is empty and `entry` must not be read; any other value
///   means the slot is live.
/// - `hash_fragment` — the low 32 bits of the full hash of
///   the stored key, cached so the probe loop can filter
///   candidates before falling back to `K::eq`. Only read
///   when `raw_state != 0`; contents are undefined on empty
///   slots.
/// - `entry` — a `MaybeUninit<(K, V)>` that holds the stored
///   key-value pair when the slot is live. Always written
///   and read as a tuple; never as independent `K` and `V`
///   halves.
///
/// `Bucket` has no `Drop` impl. Construction and destruction
/// of the `(K, V)` pair is the wrapping `RawTable`'s
/// responsibility — on drop, `RawTable` walks every live
/// bucket and calls `ptr::drop_in_place` on its entry before
/// deallocating the backing. Leaving `Bucket` trivially
/// droppable keeps the array allocation / deallocation path
/// simple and puts the panic-safety story in exactly one
/// place.
pub(super) struct Bucket<K, V> {
    raw_state: u8,
    hash_fragment: u32,
    entry: MaybeUninit<(K, V)>,
}

// Tripwire against future layout drift. The design doc's
// "Phase 2 struct design" section cites 24 bytes for
// `Bucket<u64, u64>` as the motivating case for the AoS
// layout decision; pinning it here means any future rustc
// field-reordering change that breaks that claim fires a
// compile error rather than silently shifting the doc out
// of sync with reality.
const _: () = assert!(size_of::<Bucket<u64, u64>>() == 24);
impl<K, V> Bucket<K, V> {
    /// Typed view of this bucket's state. Every caller that
    /// needs to distinguish empty / inline / recompute goes
    /// through this accessor rather than matching on
    /// `raw_state` directly.
    #[inline]
    pub(super) const fn state(&self) -> BucketState {
        match self.raw_state {
            0 => BucketState::Empty,
            255 => BucketState::OccupiedRecompute,
            n => BucketState::OccupiedInline((n - 1) as usize),
        }
    }

    /// Fast path for the probe loop: returns `true` iff this
    /// slot holds no entry. Reads `raw_state` directly so it
    /// compiles to a single compare-and-branch instead of
    /// constructing a full `BucketState` value just to match
    /// on the `Empty` variant.
    #[inline]
    pub(super) const fn is_empty(&self) -> bool {
        self.raw_state == 0
    }

    // Probe-driving primitives for `HashMap`.
    //
    // `HashMap` owns the Robin Hood probe loops, displacement, and
    // backshift deletion, but it drives them through these methods
    // rather than touching `Bucket`'s fields directly. The point of the
    // boundary is that the two error-prone things in this type — the
    // `raw_state` probe-length encoding (the `+1` offset and the `255`
    // recompute sentinel) and the `MaybeUninit` entry access — each live
    // in exactly one reviewed place here instead of being duplicated
    // across `insert` / `get` / `remove`.

    /// The cached low-32-bits hash fragment for this slot.
    ///
    /// Only meaningful when the slot is live (`!is_empty()`); the value
    /// on an empty slot is whatever the zeroed allocation left behind.
    /// Reading it is never UB regardless — `u32` accepts any bit
    /// pattern — so this is a plain accessor.
    #[inline]
    pub(super) const fn hash_fragment(&self) -> u32 {
        self.hash_fragment
    }

    /// Encode a probe length into the `raw_state` byte.
    ///
    /// The inverse of the `BucketState` decode: probe lengths `0..=253`
    /// store inline as `probe_length + 1` (so `0` stays the `Empty`
    /// marker), and anything longer collapses to the `255`
    /// `OccupiedRecompute` sentinel, whose true length is recovered from
    /// the bucket index and `hash_fragment` on demand.
    ///
    /// Can be marked `const fn` once implemented — the range match is
    /// const-compatible; it is non-const here only so the `todo!()`
    /// placeholder compiles.
    #[inline]
    fn encode_state(probe_length: usize) -> u8 {
        match u8::try_from(probe_length) {
            // Inline: store `probe_length + 1` so 0 stays the `Empty`
            // marker. `pl <= 253` ⇒ `pl + 1` is in 1..=254 — valid u8
            // arithmetic, no cast, no overflow.
            Ok(pl) if pl <= 253 => pl + 1,
            // 254, 255, or anything too large for a u8 → the recompute
            // sentinel. The byte-budget overflow *is* the sentinel
            // condition, so it collapses to one branch.
            _ => 255,
        }
    }

    /// The probe length of this (live) slot: how many positions past its
    /// home (`hash & mask`) the entry sits.
    ///
    /// For an inline slot this is just `raw_state - 1`. For the `255`
    /// recompute sentinel it is derived from the slot's own index and
    /// cached fragment:
    ///
    /// ```text
    /// home         = (hash_fragment as usize) & mask
    /// probe_length = index.wrapping_sub(home) & mask
    /// ```
    ///
    /// `index` is this bucket's position in the array and `mask` is
    /// `capacity - 1`. Calling this on an empty slot is meaningless;
    /// callers must check `is_empty()` first (debug-asserted).
    #[inline]
    pub(super) fn probe_length(&self, index: usize, mask: usize) -> usize {
        debug_assert!(self.raw_state != 0, "probe_length on an empty slot");
        match self.state() {
            BucketState::Empty => unreachable!(),
            BucketState::OccupiedInline(pl) => pl,
            BucketState::OccupiedRecompute => {
                let home = (self.hash_fragment as usize) & mask;
                index.wrapping_sub(home) & mask
            }
        }
    }

    /// Shared reference to this slot's key.
    ///
    /// # Safety
    ///
    /// The slot must be live (`!is_empty()`); reading `entry` on an
    /// empty slot is undefined behavior because it is uninitialized.
    #[inline]
    pub(super) unsafe fn key(&self) -> &K {
        debug_assert!(self.raw_state != 0, "key() on an empty slot");
        // SAFETY: The `debug_assert!` above validates that it is not being called
        //         on an empty state, therefore it is safe to retrieve the key
        unsafe { &self.entry.assume_init_ref().0 }
    }

    /// Shared reference to this slot's value.
    ///
    /// # Safety
    ///
    /// The slot must be live (`!is_empty()`).
    #[inline]
    pub(super) unsafe fn value(&self) -> &V {
        debug_assert!(self.raw_state != 0, "value() on an empty slot");
        // SAFETY: The `debug_assert!` above validates that it is not being called
        //         on an empty state, therefore it is safe to retrieve the value
        unsafe { &self.entry.assume_init_ref().1 }
    }

    /// Mutable reference to this slot's value (for `get_mut` and the
    /// existing-key overwrite path of `insert`).
    ///
    /// # Safety
    ///
    /// The slot must be live (`!is_empty()`).
    #[inline]
    pub(super) unsafe fn value_mut(&mut self) -> &mut V {
        debug_assert!(self.raw_state != 0, "value_mut() on an empty slot");
        // SAFETY: The `debug_assert!` above validates that it is not being called
        //         on an empty state, therefore it is safe to retrieve the value
        unsafe { &mut self.entry.assume_init_mut().1 }
    }

    /// A shared reference to the key paired with a mutable reference to the
    /// value — the split `IterMut` needs, which neither `key` nor
    /// `value_mut` can give alone (one borrows the bucket shared, the other
    /// mutably). Splitting the single `&mut (K, V)` into its disjoint fields
    /// hands out both at once.
    ///
    /// # Safety
    ///
    /// The slot must be live (`!is_empty()`).
    #[inline]
    pub(super) unsafe fn key_value_mut(&mut self) -> (&K, &mut V) {
        debug_assert!(self.raw_state != 0, "key_value_mut() on an empty slot");
        // SAFETY: live slot (debug_assert above), so `entry` is initialized.
        // The `&mut (K, V)` is split into `&.0` and `&mut .1`, two references
        // to disjoint fields, which the borrow checker permits simultaneously.
        let pair = unsafe { self.entry.assume_init_mut() };
        (&pair.0, &mut pair.1)
    }

    /// Write a fresh entry into this slot, encoding `probe_length` into
    /// `raw_state` and caching `fragment`.
    ///
    /// This overwrites `entry` *without dropping* whatever was there, so
    /// the caller must guarantee the slot holds no live value — either
    /// it is `Empty`, or its previous entry was already moved out with
    /// [`take_occupied`](Self::take_occupied) (the displacement path).
    #[inline]
    pub(super) fn init(&mut self, probe_length: usize, fragment: u32, entry: (K, V)) {
        self.raw_state = Self::encode_state(probe_length);
        self.hash_fragment = fragment;
        self.entry = MaybeUninit::new(entry);
    }

    /// Re-encode this (live) slot's probe length, e.g. after a backshift
    /// move pulls the entry one position closer to home.
    ///
    /// Does not touch the entry or fragment — only `raw_state`.
    #[inline]
    pub(super) fn set_probe_length(&mut self, probe_length: usize) {
        debug_assert!(self.raw_state != 0, "set_probe_length on an empty slot");
        self.raw_state = Self::encode_state(probe_length);
    }

    /// Move the entry out of this slot, returning its cached fragment and
    /// the `(K, V)` pair.
    ///
    /// Leaves `raw_state` unchanged — the slot's bytes are now stale, and
    /// the caller is responsible for either overwriting it
    /// ([`init`](Self::init), or a `RawTable::copy_bucket` over it) or
    /// marking it [`set_empty`](Self::set_empty). Reading the slot's
    /// entry again before one of those happens is a use-after-move.
    ///
    /// # Safety
    ///
    /// The slot must be live (`!is_empty()`).
    #[inline]
    pub(super) unsafe fn take_occupied(&mut self) -> (u32, (K, V)) {
        debug_assert!(self.raw_state != 0, "take_occupied on an empty slot");
        let fragment = self.hash_fragment;
        // SAFETY: caller guarantees the slot is live (debug_assert above), so
        // `entry` is initialized and can be moved out exactly once.
        unsafe { (fragment, self.entry.assume_init_read()) }
    }

    /// Mark this slot empty. Does not drop or move the entry — the caller
    /// must have already taken ownership of any live value (the final
    /// step of a backshift, where the vacated tail slot is a stale copy).
    #[inline]
    pub(super) fn set_empty(&mut self) {
        self.raw_state = 0;
    }
}

/// Allocation-owning backing for the Robin Hood hash table.
///
/// A `RawTable` holds a contiguous array of `Bucket<K, V>`
/// slots on the heap, along with the metadata needed to
/// locate a free slot for insertion and to know when the
/// table is full enough to grow. It knows about storage,
/// allocation, and drop — nothing else. Probe sequences,
/// Robin Hood displacement, load-factor checks, and the
/// hasher itself all live on the wrapping `HashMap`, which
/// reaches into a `RawTable` via `bucket` / `bucket_mut` for
/// slot access. The storage-vs-probing split mirrors the
/// `RawVec<T>` / `Vec<T>` relationship and is the reason a
/// separate `RawTable` type exists at all.
///
/// # Fields
///
/// - `buckets` — raw pointer to the start of the bucket
///   array. `NonNull` (not `*mut`) so the type has a
///   non-null niche and so `NonNull::dangling()` gives us a
///   well-aligned sentinel for the `capacity == 0` case.
///   Must never be dereferenced when `capacity == 0`.
/// - `capacity` — number of bucket slots in the backing
///   array. Always either `0` or a power of two `>= 8`.
///   This is the denominator of the load-factor check, not
///   the number of live entries.
/// - `len` — number of live entries (buckets whose
///   `raw_state != 0`). Incremented on insert, decremented
///   on remove. When `len * 100 >= capacity * 70` the next
///   insert triggers a `grow_to(capacity * 2)`. Named `len`
///   so it pairs naturally with `capacity` and matches
///   `std::HashMap::len()` on the public wrapper.
/// - `_marker` — `PhantomData<(K, V)>` so the dropck sees
///   `RawTable` as conceptually owning its `K` and `V`
///   types. Without it, dropping a `RawTable<&'a str, V>`
///   whose `'a` is about to be freed would not be rejected.
///   Variance stays covariant in both parameters via the
///   `NonNull`.
pub(super) struct RawTable<K, V> {
    buckets: NonNull<Bucket<K, V>>,
    capacity: usize,
    len: usize,
    _marker: PhantomData<(K, V)>,
}
impl<K, V> RawTable<K, V> {
    /// Construct an empty `RawTable` with no heap allocation.
    ///
    /// The `buckets` pointer is `NonNull::dangling()` — a
    /// well-aligned non-null sentinel that must not be
    /// dereferenced. No bucket memory is allocated until
    /// `with_capacity` or `grow_to` is called, so the
    /// resulting table has `capacity() == 0` and `len() == 0`.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub(super) const fn new() -> Self {
        Self {
            buckets: NonNull::dangling(),
            capacity: 0,
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Construct a `RawTable` whose backing array is large enough
    /// to hold at least `entries` live entries without growing.
    ///
    /// This is the eager counterpart to [`new`](Self::new): where
    /// `new` allocates nothing and defers the first allocation to
    /// the initial insert, `with_capacity` allocates the backing
    /// up front so a caller who already knows the entry count pays
    /// for one allocation instead of a sequence of regrows.
    ///
    /// `entries` is a count of *entries*, not buckets. The backing
    /// array is deliberately larger than `entries` for two
    /// reasons:
    ///
    /// 1. **Load factor.** The table grows once it is 70% full, so
    ///    holding `entries` entries needs at least `entries / 0.7`
    ///    bucket slots. Sizing to exactly `entries` would leave the
    ///    table 100% full and trip the grow threshold on the
    ///    entry that fills it.
    /// 2. **Power-of-two capacity.** The probe loops derive a
    ///    bucket's home position with `hash & (capacity - 1)`,
    ///    which is only equivalent to `hash % capacity` when
    ///    `capacity` is a power of two. The derived bucket count is
    ///    therefore rounded up to the next power of two, with a
    ///    floor of 8.
    ///
    /// So [`capacity`](Self::capacity) on the returned table
    /// reports the rounded bucket count, which is `>= entries` and
    /// generally strictly greater. Passing `entries == 0` still
    /// allocates the minimum 8-bucket array — callers wanting the
    /// no-allocation state use [`new`](Self::new).
    ///
    /// The backing is obtained with `alloc_zeroed`; because
    /// `raw_state == 0` encodes [`BucketState::Empty`], a zeroed
    /// block is already a valid all-empty table and needs no
    /// per-bucket initialization.
    ///
    /// # Panics
    ///
    /// Panics via [`capacity_overflow`](crate::raw::capacity_overflow)
    /// if the bucket count derived from `entries` overflows `usize`.
    /// Allocation failure is routed through `handle_alloc_error`
    /// rather than a panic.
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) in the derived bucket count, dominated by the
    /// zeroing of the backing array.
    pub(super) fn with_capacity(entries: usize) -> Self {
        let capacity = Self::buckets_for(entries);
        let buckets = alloc_array::<Bucket<K, V>>(capacity, true);
        Self {
            buckets,
            capacity,
            len: 0, // No entries in the table yet.
            _marker: PhantomData,
        }
    }

    /// The bucket count needed to hold `entries` live entries under the
    /// 70% load factor: the smallest power of two `>= 8` whose
    /// [`HashMap`](super::hash_map) entry-capacity covers `entries`.
    ///
    /// Shared by [`with_capacity`](Self::with_capacity) and the
    /// `HashMap::{reserve, shrink_to_fit}` resize targets so all three
    /// derive the same backing size from a desired entry count. Panics
    /// (via `capacity_overflow`) if `entries` is large enough that the
    /// load-factor scaling overflows `usize`.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub(super) fn buckets_for(entries: usize) -> usize {
        // Scale by 100 before dividing by 70 so the 70%-load math
        // stays in integers; the `+ 69` rounds the division up so the
        // table can actually hold `entries` before it grows.
        let Some(scaled) = entries.checked_mul(100).and_then(|x| x.checked_add(69)) else {
            capacity_overflow();
        };
        let min_buckets = scaled / 70;
        core::cmp::max(8, min_buckets).next_power_of_two()
    }

    /// Number of bucket slots in the backing array.
    ///
    /// Always either `0` or a power of two `>= 8`. This is the
    /// denominator of the load-factor check, not the count of
    /// live entries — use [`len`](Self::len) for that.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of live entries currently stored in the table.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) const fn len(&self) -> usize {
        self.len
    }

    /// Set the cached live-entry count.
    ///
    /// `RawTable` cannot compute its own length — it has no view of
    /// which slot writes are net additions vs. displacements — so the
    /// probing layer (`HashMap`) is the source of truth and reports the
    /// count through this setter after each `insert` / `remove` / rehash.
    /// The caller must keep `len` in step with the number of live buckets;
    /// a `debug_assert` guards the obvious `len <= capacity` invariant.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.capacity, "len cannot exceed capacity");
        self.len = len;
    }

    /// Returns `true` when the table holds no live entries.
    ///
    /// Note that `is_empty()` can return `true` while
    /// `capacity() > 0` — a freshly-grown table that has not
    /// yet been written to, or a table after every entry has
    /// been removed, is both allocated and empty.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a shared reference to the bucket at index `i`.
    ///
    /// # Safety
    ///
    /// `i` must be less than [`capacity`](Self::capacity).
    /// The caller is responsible for bounds checking; this
    /// function does not validate at runtime in release
    /// builds (it carries only a `debug_assert!` tripwire).
    ///
    /// The returned reference is live for as long as
    /// `&self`. The bucket's `entry` field may be
    /// uninitialized when `raw_state == 0`; reading it is
    /// undefined behavior unless the caller first checks
    /// the state through `Bucket::state` or
    /// `Bucket::is_empty`.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) unsafe fn bucket(&self, i: usize) -> &Bucket<K, V> {
        debug_assert!(i < self.capacity, "bucket index out of bounds");
        // SAFETY: the caller guarantees `i < self.capacity`,
        // so `self.buckets.as_ptr().add(i)` stays inside the
        // single allocation the `buckets` pointer refers to.
        // A zeroed `Bucket` is a valid `Bucket` (u8 and u32
        // accept any bit pattern, and `MaybeUninit` accepts
        // any bit pattern by construction), so dereferencing
        // the resulting pointer produces a well-formed
        // `Bucket<K, V>` value. The returned reference
        // inherits `&self`'s lifetime via elision and cannot
        // outlive the `RawTable`.
        unsafe { &*self.buckets.as_ptr().add(i) }
    }

    /// Returns a mutable reference to the bucket at index
    /// `i`.
    ///
    /// # Safety
    ///
    /// Same as [`bucket`](Self::bucket): `i` must be less
    /// than [`capacity`](Self::capacity), and the caller is
    /// responsible for respecting the `raw_state` liveness
    /// marker when reading or writing the `entry` field.
    /// The returned `&mut Bucket` is the unique mutable
    /// reference to that slot — the `&mut self` borrow
    /// prevents any other bucket accessor from coexisting
    /// with it.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) unsafe fn bucket_mut(&mut self, i: usize) -> &mut Bucket<K, V> {
        debug_assert!(i < self.capacity, "bucket index out of bounds");
        // SAFETY: same reasoning as `bucket`. The caller
        // guarantees `i < self.capacity`, so the pointer
        // offset stays inside the allocation; a zeroed
        // `Bucket` is valid; and the `&mut Bucket` reference
        // inherits `&mut self`'s exclusive borrow, so no
        // other reference to the same or another bucket can
        // alias it for the duration of the returned borrow.
        unsafe { &mut *self.buckets.as_ptr().add(i) }
    }

    /// The whole backing as a bucket slice, empty and occupied alike.
    ///
    /// The iterator types walk this and skip empty slots themselves; a
    /// slice hands them a cursor with a real lifetime, so the borrowed
    /// `&K` / `&V` they hand out are tied to `&self` without the manual
    /// pointer-lifetime juggling a raw walk would need.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) fn as_slice(&self) -> &[Bucket<K, V>] {
        // SAFETY: `buckets` points to `capacity` consecutive initialized
        // `Bucket`s (a zeroed bucket is a valid bucket). At `capacity == 0`
        // the pointer is dangling but well-aligned and the length is 0,
        // which `from_raw_parts` explicitly permits.
        unsafe { slice::from_raw_parts(self.buckets.as_ptr(), self.capacity) }
    }

    /// The whole backing as a mutable bucket slice, for `IterMut`.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    pub(super) fn as_mut_slice(&mut self) -> &mut [Bucket<K, V>] {
        // SAFETY: as in `as_slice`; the `&mut self` borrow makes the
        // returned slice the unique path to every bucket for its duration.
        unsafe { slice::from_raw_parts_mut(self.buckets.as_ptr(), self.capacity) }
    }

    /// Replace the backing array with a fresh, zeroed array of
    /// `new_capacity` buckets, returning the old table intact.
    ///
    /// Does **not** rehash or move entries. The returned table still
    /// owns every live entry it held; the caller is responsible for
    /// draining it — moving each entry into `self` and marking the
    /// drained bucket `Empty` — before the returned table drops.
    /// Whatever entries remain when it drops are destroyed. Draining
    /// is the caller's job because re-homing an entry needs a
    /// `BuildHasher`, which this storage layer does not own.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if `new_capacity` is not a power of two
    /// `>= 8`. Allocation failure routes through `handle_alloc_error`.
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) in `new_capacity`, dominated by zeroing the new backing.
    /// The swap itself is *O*(1); rehashing the old entries is the
    /// caller's cost, not counted here.
    #[must_use = "the returned table still owns its entries; drain it before it drops"]
    pub(super) fn grow_to(&mut self, new_capacity: usize) -> RawTable<K, V> {
        // Trust, don't round. Every caller (`HashMap::resize_to`, reached
        // from grow / reserve / shrink_to_fit) passes a power-of-two target
        // derived from `buckets_for`; rounding here would mask a caller bug.
        debug_assert!(
            new_capacity.is_power_of_two() && new_capacity >= 8,
            "grow_to needs a power-of-two capacity >= 8"
        );
        let buckets = alloc_array::<Bucket<K, V>>(new_capacity, true);
        let new_table = Self {
            buckets,
            capacity: new_capacity,
            len: 0, // No entries in the table yet.
            _marker: PhantomData,
        };
        // Change self to point to the new table and return the old one
        mem::replace(self, new_table)
    }

    /// Bitwise-move the bucket at `from` over the bucket at `to`.
    ///
    /// This is the backshift primitive: during `HashMap::remove`, each
    /// displaced entry is pulled one slot back toward its home by copying
    /// its whole bucket — state byte, fragment, and `(K, V)` — into the
    /// preceding slot. It is a raw move, not a clone: afterwards `to`
    /// owns the entry and `from` holds a stale duplicate that must be
    /// overwritten by the next copy or marked empty. Nothing is dropped.
    ///
    /// Both slots are accessed through the backing pointer directly
    /// rather than two `bucket_mut` borrows, because two simultaneous
    /// `&mut Bucket` into the same array would alias.
    ///
    /// # Safety
    ///
    /// - `from` and `to` must both be `< capacity` and must differ
    ///   (`from != to`).
    /// - `to`'s current contents must not be a live value the caller
    ///   still needs dropped — `remove` guarantees this (the destination
    ///   was either just vacated or a slot already copied forward).
    #[inline]
    pub(super) unsafe fn copy_bucket(&mut self, from: usize, to: usize) {
        debug_assert!(from < self.capacity && to < self.capacity);
        debug_assert!(from != to, "copy_bucket requires distinct slots");
        let src = unsafe { self.buckets.as_ptr().add(from) };
        let dst = unsafe { self.buckets.as_ptr().add(to) };
        // SAFETY: both indices are < capacity (debug_assert) so the offsets stay in
        // the one allocation, the buckets are aligned (alloc_array), and from != to
        // so source and destination don't overlap. This is a raw move,
        // not a clone — the safety contract obliges the caller to ensure `to` holds
        // no value that will be dropped elsewhere, which `remove`'s backshift upholds.
        unsafe { ptr::copy_nonoverlapping(src, dst, 1) };
    }
}

/// Unwind-safe finisher for `RawTable::drop`.
///
/// Carries the index of the next bucket to process and owns the
/// completion of cleanup: its own `Drop` finishes dropping any live
/// entries the main path didn't reach, then frees the backing. This
/// guarantees the backing is deallocated exactly once whether the
/// drop walk completes normally or a `K`/`V` destructor panics partway
/// through. `PhantomData<(K, V)>` matches `RawTable`'s marker so dropck
/// sees the guard as owning the `K` and `V` it destroys.
struct DropGuard<K, V> {
    buckets: NonNull<Bucket<K, V>>,
    capacity: usize,
    next: usize,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> DropGuard<K, V> {
    /// Drop every live entry in `next..capacity`, advancing `next`
    /// *before* each `drop_in_place`.
    ///
    /// The advance-before-drop order is the panic-safety invariant: if
    /// an entry's destructor panics, the resumed walk (from the guard's
    /// own `Drop`) starts past that bucket, so the panicking entry is
    /// never dropped a second time. Safe to call more than once — once
    /// `next == capacity` it is a no-op.
    fn drop_remaining(&mut self) {
        while self.next < self.capacity {
            let i = self.next;
            // Advance before the drop: if `drop_in_place` panics, the
            // guard's own Drop resumes at `i + 1`, so bucket `i` is never
            // dropped twice.
            self.next += 1;

            // SAFETY: `i < self.capacity` (the loop condition held when we
            // captured `i = self.next`). `self.capacity` is the exact bucket
            // count `alloc_array` allocated, so offsetting by `i` stays inside
            // that one allocation, and `Layout::array` already proved the byte
            // offset fits in `isize`. The guard is only constructed when
            // capacity != 0 (RawTable::drop returns early otherwise), so
            // `buckets` is a real, aligned base; `&mut self` makes the
            // resulting borrow unique.
            let bucket = unsafe { &mut *self.buckets.as_ptr().add(i) };

            if bucket.raw_state != 0 {
                // SAFETY: raw_state != 0 means the slot is live, so `entry`
                // is initialized and `as_mut_ptr()` points at a valid
                // `(K, V)`. Dropped exactly once — advancing `next` above
                // means a re-entrant Drop skips this bucket.
                unsafe {
                    ptr::drop_in_place(bucket.entry.as_mut_ptr());
                }
            }
        }
    }
}

impl<K, V> Drop for DropGuard<K, V> {
    fn drop(&mut self) {
        // Finish any entries the main path didn't reach (the panic path),
        // then free the backing. On the normal path this is a no-op walk
        // followed by the dealloc.
        self.drop_remaining();

        // SAFETY: `self.buckets` came from
        // `alloc_array::<Bucket<K, V>>(self.capacity, _)` and is freed
        // exactly once — `Drop` runs once, and `capacity != 0` is
        // guaranteed by `RawTable::drop`'s early return before the guard
        // is ever constructed.
        unsafe {
            dealloc_array(self.buckets, self.capacity);
        }
    }
}

impl<K, V> Drop for RawTable<K, V> {
    fn drop(&mut self) {
        // A capacity-0 table (`new()`, or the old table handed back by
        // `grow_to` on a fresh map) has a dangling pointer and no
        // allocation — there is nothing to drop or free.
        if self.capacity == 0 {
            return;
        }

        // Hand cleanup to the guard so a panicking entry destructor still
        // frees the backing. The guard's own `Drop` runs when `guard`
        // leaves scope below; on the normal path `drop_remaining` has
        // already drained every entry, so that `Drop` only deallocs.
        let mut guard = DropGuard {
            buckets: self.buckets,
            capacity: self.capacity,
            next: 0,
            _marker: PhantomData,
        };
        guard.drop_remaining();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;

    /// Drop-observing entry fixture.
    ///
    /// There is no public `insert` yet (that lands in Phase 3), so the
    /// tests can't drive entries through the real API. `Probe` is the
    /// stand-in: every variant bumps a shared counter on drop, so a test
    /// can assert "this many entries were destroyed". The `Poison`
    /// variant additionally panics *after* bumping the counter, which is
    /// how the panic-safety test forces a destructor to unwind partway
    /// through `RawTable`'s drop walk.
    enum Probe {
        Plain(Rc<Cell<usize>>),
        Poison(Rc<Cell<usize>>),
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            match self {
                Probe::Plain(counter) => {
                    counter.set(counter.get() + 1);
                }
                Probe::Poison(counter) => {
                    // Count first, then unwind — so the panic-safety test
                    // can confirm the poisoned entry was dropped exactly
                    // once even though its destructor panicked.
                    counter.set(counter.get() + 1);
                    panic!("Probe::Poison dropped — exercising panic-during-drop");
                }
            }
        }
    }

    /// Write a live entry into bucket `i`, marking it `OccupiedInline(0)`
    /// and bumping `len`. Stands in for the real insert. Callers must
    /// only target empty slots — overwriting a live bucket would leak its
    /// existing entry, since this clobbers the `MaybeUninit` without
    /// dropping what was there.
    fn put<K, V>(table: &mut RawTable<K, V>, i: usize, entry: (K, V)) {
        // SAFETY: tests pass `i < capacity` (enforced by `bucket_mut`'s
        // debug_assert) into a slot known to be empty.
        let bucket = unsafe { table.bucket_mut(i) };
        bucket.raw_state = 1; // OccupiedInline(0)
        bucket.entry = MaybeUninit::new(entry);
        table.len += 1;
    }

    #[test]
    fn new_is_empty_and_unallocated() {
        let table = RawTable::<i32, i32>::new();
        assert_eq!(table.capacity(), 0);
        assert_eq!(table.len(), 0);
        assert!(table.is_empty());
    }

    #[test]
    fn with_capacity_rounds_up_to_power_of_two_floor_eight() {
        // 0 and small requests floor at 8 buckets; the first request whose
        // 70%-load bucket count exceeds 8 jumps to the next power of two.
        assert_eq!(RawTable::<i32, i32>::with_capacity(0).capacity(), 8);
        assert_eq!(RawTable::<i32, i32>::with_capacity(5).capacity(), 8);
        // 6 entries need ceil(6 / 0.7) = 9 buckets → next power of two is 16.
        assert_eq!(RawTable::<i32, i32>::with_capacity(6).capacity(), 16);
        // 100 entries need ceil(100 / 0.7) = 143 buckets → 256.
        assert_eq!(RawTable::<i32, i32>::with_capacity(100).capacity(), 256);

        let table = RawTable::<i32, i32>::with_capacity(50);
        assert_eq!(table.len(), 0);
        assert!(table.is_empty());
    }

    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn with_capacity_overflow_panics() {
        // `usize::MAX` entries overflow the 70%-load scaling math long
        // before any allocation is attempted, routing through
        // `capacity_overflow`.
        let _table = RawTable::<u64, u64>::with_capacity(usize::MAX);
    }

    #[test]
    fn grow_to_from_zero_swaps_in_new_backing() {
        // The `HashMap::new()` path: a capacity-0 table grows to its first
        // real allocation. `*self` becomes the new empty table; the
        // returned old table carries the (empty, dangling) prior state and
        // drops harmlessly via the capacity-0 early return.
        let mut table = RawTable::<i32, i32>::new();
        let old = table.grow_to(8);
        assert_eq!(table.capacity(), 8);
        assert_eq!(table.len(), 0);
        assert_eq!(old.capacity(), 0);
        assert_eq!(old.len(), 0);
    }

    #[test]
    fn grow_to_hands_back_the_old_table_without_dropping_entries() {
        let counter = Rc::new(Cell::new(0));
        let mut table = RawTable::<Probe, ()>::with_capacity(4);
        let old_capacity = table.capacity();

        put(&mut table, 0, (Probe::Plain(counter.clone()), ()));
        put(&mut table, 2, (Probe::Plain(counter.clone()), ()));

        let old = table.grow_to(old_capacity * 2);

        // `*self` is the new, larger, empty table.
        assert_eq!(table.capacity(), old_capacity * 2);
        assert_eq!(table.len(), 0);
        // The old table still owns the two entries, untouched.
        assert_eq!(old.capacity(), old_capacity);
        assert_eq!(old.len(), 2);
        assert_eq!(counter.get(), 0, "grow_to must not drop any entries");

        // Dropping the handed-back table is what destroys the entries —
        // exactly once each. (Phase 4's rehash will instead drain them
        // into the new table; here we verify the ownership/drop contract.)
        drop(old);
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn drop_destroys_each_live_entry_exactly_once() {
        let counter = Rc::new(Cell::new(0));
        {
            let mut table = RawTable::<Probe, ()>::with_capacity(8);
            // Non-contiguous occupancy with empty gaps between live slots,
            // so this also proves the walk skips `Empty` buckets rather
            // than reading their uninitialized `entry` field.
            put(&mut table, 0, (Probe::Plain(counter.clone()), ()));
            put(&mut table, 3, (Probe::Plain(counter.clone()), ()));
            put(&mut table, 7, (Probe::Plain(counter.clone()), ()));
            assert_eq!(table.len(), 3);
        }
        assert_eq!(counter.get(), 3);
    }

    #[test]
    fn drop_of_allocated_but_empty_table_touches_no_entries() {
        let counter = Rc::new(Cell::new(0));
        {
            let _table = RawTable::<Probe, ()>::with_capacity(8);
            // No entries written; every bucket is `Empty`.
        }
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn drop_is_panic_safe() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        // A poisoned entry sits in the middle of the occupied run, so its
        // panic interrupts the drop walk with live entries on both sides.
        // The guard must still (a) not re-drop the poisoned entry, (b)
        // finish dropping the entries after it, and (c) free the backing
        // (the latter verified by miri in CI). All five entries counting
        // exactly once is the observable proof of (a) and (b).
        let counter = Rc::new(Cell::new(0));
        let inner = counter.clone();

        // NOTE: the poisoned destructor prints a panic message to stderr;
        // that is expected test noise, not a failure.
        let result = catch_unwind(AssertUnwindSafe(move || {
            let mut table = RawTable::<Probe, ()>::with_capacity(8);
            put(&mut table, 0, (Probe::Plain(inner.clone()), ()));
            put(&mut table, 1, (Probe::Plain(inner.clone()), ()));
            put(&mut table, 2, (Probe::Poison(inner.clone()), ()));
            put(&mut table, 3, (Probe::Plain(inner.clone()), ()));
            put(&mut table, 4, (Probe::Plain(inner.clone()), ()));
            drop(table);
        }));

        assert!(result.is_err(), "the poisoned drop must propagate a panic");
        assert_eq!(
            counter.get(),
            5,
            "every entry dropped exactly once despite the mid-walk panic"
        );
    }

    #[test]
    fn zero_sized_kv_still_allocates_and_drops() {
        // `Bucket<(), ()>` is 8 bytes, not a ZST, so the table allocates
        // normally and there is no infinite-capacity special case. This
        // mainly exists as a miri exercise of the `MaybeUninit<((), ()))>`
        // drop path.
        // 8 entries need ceil(8 / 0.7) = 12 buckets → next power of two 16.
        let mut table = RawTable::<(), ()>::with_capacity(8);
        assert_eq!(table.capacity(), 16);
        put(&mut table, 0, ((), ()));
        assert_eq!(table.len(), 1);
    }

    // 3a primitive-surface tests: the probe-length encode/decode and the
    // take+init / copy_bucket mechanics, exercised in isolation before
    // `HashMap` drives them for real.

    #[test]
    fn probe_length_inline_round_trips() {
        // Inline encoding covers 0..=253; decode ignores index/mask for it.
        let mut table = RawTable::<u32, u32>::with_capacity(8);
        let bucket = unsafe { table.bucket_mut(0) };
        for pl in [0usize, 1, 2, 100, 253] {
            bucket.init(pl, 0xABCD, (1, 2));
            assert!(matches!(bucket.state(), BucketState::OccupiedInline(p) if p == pl));
            assert_eq!(bucket.probe_length(0, 7), pl);
        }
    }

    #[test]
    fn probe_length_boundary_between_inline_and_sentinel() {
        let mut table = RawTable::<u32, u32>::with_capacity(8);
        let bucket = unsafe { table.bucket_mut(0) };
        // 253 is the largest inline probe length.
        bucket.init(253, 0, (1, 2));
        assert!(matches!(bucket.state(), BucketState::OccupiedInline(253)));
        // 254 is the first that overflows into the recompute sentinel.
        bucket.init(254, 0, (1, 2));
        assert!(matches!(bucket.state(), BucketState::OccupiedRecompute));
    }

    #[test]
    fn probe_length_recompute_sentinel_path() {
        // The corner the Phase-2 tests never reached: a probe length too
        // large to store inline, recovered from the slot's index and
        // cached fragment. capacity 512 lets us place an entry 300 slots
        // from its home (300 >= 254 → the sentinel).
        let mut table = RawTable::<u32, u32>::with_capacity(300);
        assert_eq!(table.capacity(), 512);
        let mask = table.capacity() - 1;
        let index = 300;

        // fragment 0 → home = 0 & mask = 0, so geometric distance == index.
        let bucket = unsafe { table.bucket_mut(index) };
        bucket.init(index, 0, (7, 8));
        assert!(matches!(bucket.state(), BucketState::OccupiedRecompute));
        // Decoded from position, not from the value passed to `init`.
        assert_eq!(bucket.probe_length(index, mask), 300);

        // Mark empty again so the table's Drop doesn't read the lone slot
        // as live (the (u32, u32) entry has no destructor, but keep the
        // invariant honest).
        unsafe { table.bucket_mut(index) }.set_empty();
    }

    #[test]
    fn take_then_init_is_a_move_not_a_drop() {
        // The insert displacement step: take the resident out, write the
        // incoming over the slot. Taking must not drop the entry (the
        // caller now owns it), and re-init must not drop the stale bytes.
        let counter = Rc::new(Cell::new(0));
        let mut table = RawTable::<Probe, u32>::with_capacity(8);

        unsafe { table.bucket_mut(0) }.init(2, 0xAB, (Probe::Plain(counter.clone()), 99));

        let (fragment, (key, value)) = unsafe { table.bucket_mut(0).take_occupied() };
        assert_eq!(fragment, 0xAB);
        assert_eq!(value, 99);
        assert_eq!(counter.get(), 0, "take_occupied moves, it must not drop");

        // Write a different entry over the now-stale slot.
        unsafe { table.bucket_mut(0) }.init(5, 0xCD, (Probe::Plain(counter.clone()), 7));
        assert!(matches!(
            unsafe { table.bucket(0) }.state(),
            BucketState::OccupiedInline(5)
        ));

        // The taken-out key is ours; dropping it is the only drop so far.
        drop(key);
        assert_eq!(counter.get(), 1);

        // Table drop destroys the re-init'd slot-0 entry exactly once.
        drop(table);
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn copy_bucket_backshift_does_not_double_drop() {
        // One backshift step in isolation: remove slot 0, shift slot 1's
        // entry back into it, vacate slot 1. The shifted entry must end up
        // owned by exactly one slot — no leak, no double-drop.
        let counter = Rc::new(Cell::new(0));
        let mut table = RawTable::<Probe, u32>::with_capacity(8);
        let mask = table.capacity() - 1;

        unsafe { table.bucket_mut(0) }.init(0, 0x10, (Probe::Plain(counter.clone()), 100));
        unsafe { table.bucket_mut(1) }.init(1, 0x20, (Probe::Plain(counter.clone()), 200));

        // Remove slot 0: move its entry out and drop it.
        let (_frag, entry0) = unsafe { table.bucket_mut(0).take_occupied() };
        drop(entry0);
        assert_eq!(counter.get(), 1);

        // Backshift slot 1 → slot 0, decrementing the moved entry's probe
        // length, then vacate the stale tail slot.
        let pl1 = unsafe { table.bucket(1) }.probe_length(1, mask);
        unsafe { table.copy_bucket(1, 0) };
        unsafe { table.bucket_mut(0) }.set_probe_length(pl1 - 1);
        unsafe { table.bucket_mut(1) }.set_empty();

        assert!(!unsafe { table.bucket(0) }.is_empty());
        assert_eq!(unsafe { table.bucket(0) }.probe_length(0, mask), 0);
        assert_eq!(unsafe { *table.bucket(0).value() }, 200);
        assert!(unsafe { table.bucket(1) }.is_empty());
        assert_eq!(counter.get(), 1, "the bitwise move must not drop anything");

        // Only the shifted entry remains live; it drops exactly once.
        drop(table);
        assert_eq!(counter.get(), 2);
    }
}
