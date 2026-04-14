// `doc_markdown` is allowed at the module level because the prose
// discusses algorithm names (`FxHash`, `SipHash`), abbreviations
// (`DoS`), and crate names (`rustc-hash`) that read naturally
// without backticks. The pedantic lint flags them as "looks like a
// code identifier, wrap in backticks," but backtick-ing every
// mention of an algorithm name in a prose paragraph makes the text
// harder to read, not easier.
#![allow(clippy::doc_markdown)]

//! FxHash — a fast, non-DoS-resistant hash function.
//!
//! FxHash is the hash function `rustc` uses internally for its own
//! data structures. `koala-std` ports it here for the same reason:
//! the workload is non-adversarial (internal browser-engine
//! containers, never fed user-controlled keys), speed matters more
//! than collision quality on pathological inputs, and the algorithm
//! is small enough to audit in a single sitting.
//!
//! # Algorithm
//!
//! The hasher holds a single `usize` of state. Each absorbed input
//! `usize` is folded in with two wrapping arithmetic operations:
//!
//! ```text
//!   ┌─────────────────────┐
//!   │   state (usize)     │
//!   └─────────┬───────────┘
//!             │
//!             │  ┌── input (usize) ──┐
//!             ▼  ▼
//!   ┌─────────────────────┐
//!   │    wrapping_add     │
//!   └─────────┬───────────┘
//!             │
//!             ▼
//!   ┌─────────────────────┐
//!   │  wrapping_mul(K)    │
//!   └─────────┬───────────┘
//!             │
//!             ▼
//!   ┌─────────────────────┐
//!   │   new state value   │
//!   └─────────────────────┘
//! ```
//!
//! The multiply by the magic constant `K` provides almost all of
//! the mixing. `K` is not arbitrary — it comes from Steele & Vigna's
//! "Computationally Easy, Spectrally Good Multipliers for
//! Congruential Pseudorandom Number Generators" (2022), chosen for
//! the spectral properties that make it an excellent multiplier for
//! a linear congruential generator. Those same properties make it a
//! good hash-diffusion constant.
//!
//! When [`FxHasher::finish`] is called, the state goes through a
//! single final post-processing step — a rotate — before being
//! returned as a `u64`:
//!
//! ```text
//!   ┌─────────────────────┐
//!   │  final state        │
//!   └─────────┬───────────┘
//!             │
//!             ▼
//!   ┌─────────────────────┐
//!   │ rotate_left(ROTATE) │
//!   └─────────┬───────────┘
//!             │
//!             ▼
//!   ┌─────────────────────┐
//!   │   output (u64)      │
//!   └─────────────────────┘
//! ```
//!
//! The rotate happens at finalization time, not inside `add_to_hash`,
//! for two reasons:
//!
//! 1. **Speed on the hot path.** Absorbing a `usize` is two
//!    operations (`wrapping_add`, `wrapping_mul`). Adding a third
//!    op inside that loop would slow every absorption by ~33%. The
//!    rotate is applied once, at `finish()` time, not once per
//!    word.
//! 2. **Entropy redistribution for `hashbrown`-style tables.** The
//!    multiply alone leaves high state bits somewhat uncorrelated
//!    with low state bits. Rotating by 26 on 64-bit targets lifts
//!    bits from the middle of the word (where the multiply's
//!    diffusion is densest) into the top 7 positions, which is
//!    where `hashbrown`-style hash tables look for their H2 tag
//!    entropy. `koala-std`'s Robin Hood table does not use H2
//!    tags, but matching `rustc-hash` byte-for-byte keeps the
//!    correctness oracle (the differential test against
//!    `rustc_hash::FxHasher`) usable.
//!
//! # Processing a byte slice
//!
//! [`FxHasher::write`] accepts arbitrary `&[u8]` input. The bytes
//! are consumed in full `usize`-sized chunks through the main loop,
//! with any trailing bytes folded in one at a time:
//!
//! ```text
//!   bytes: [b0 b1 b2 b3 b4 b5 b6 b7 | b8 b9 ba bb bc bd be bf | c0 c1 c2]
//!          └────── chunk 1 ───────┘ └────── chunk 2 ───────┘ └─ tail ─┘
//!                    │                        │                  │
//!                    ▼                        ▼                  ▼
//!              add_to_hash              add_to_hash       add_to_hash × 3
//!              (one full usize)         (one full usize)  (one per byte)
//! ```
//!
//! The native-endian conversion ([`usize::from_ne_bytes`]) means
//! FxHash produces different hash values on little-endian vs
//! big-endian targets for the same input. This is fine for internal
//! containers that are never persisted across machines; it matches
//! `rustc-hash`'s behavior; and it is the fastest way to turn 8
//! bytes into a `usize` on contemporary hardware.
//!
//! # DoS resistance
//!
//! FxHash is **not** DoS-resistant. A motivated adversary who can
//! control the input keys can construct collision sets that degrade
//! any hash table using FxHash to quadratic probe cost. Do not use
//! `FxHasher` (or any `HashMap` keyed on it) for data that crosses
//! a trust boundary. For adversarial workloads, reach for
//! `std::hash::DefaultHasher` (SipHash) or `ahash` instead.
//!
//! # Known limitations
//!
//! **All-zero inputs collapse to zero.** Because `add_to_hash` is
//! `state ← (state + i) * K` and the initial state is `0`, feeding
//! `i = 0` leaves the state at `0`, and any subsequent zero input
//! keeps it there. As a result, **every all-zero-byte input of any
//! length hashes to `0`** — `hash("")`, `hash(&[0])`, `hash(&[0;
//! 100])` all produce the same output.
//!
//! This is specifically a consequence of our simplified `write()`
//! path; rustc-hash's upstream `hash_bytes` prevents the collapse
//! with a `PREVENT_TRIVIAL_ZERO_COLLAPSE` constant that gets XOR'd
//! into the mix, but our port does not. The tradeoff was taken
//! deliberately: in Koala's actual `HashMap` workloads, keys are
//! `NodeId`s (u64 indices, never all-zero for real nodes), DOM/CSS
//! identifiers (non-empty UTF-8 strings), and HTML entity names
//! (static strings, never all-zero bytes), so the collapse is not
//! observable in practice. If that changes and Koala ever grows a
//! `HashMap<[u8; N], V>`-shaped workload with all-zero keys in the
//! hot path, we should revisit: the fix is either to add a `len`
//! field to `FxHasher` and mix it into `finish`, or to port the
//! upstream `hash_bytes` verbatim.
//!
//! The tests under `tests/hash_fx_oracle.rs` include a dedicated
//! `all_zero_inputs_all_collapse_to_zero` regression test that
//! documents this limitation and will surface any unintentional
//! change to the behavior.
//!
//! # Source
//!
//! This is a near-verbatim port of the current `rustc-hash` crate
//! (version 2.x, using the Steele-Vigna multipliers). The algorithm
//! was updated in rustc-hash 2.0; earlier versions of FxHash used a
//! different multiplier and are *not* bit-level equivalent to this
//! one.

use core::hash::{BuildHasherDefault, Hasher};

// The magic multiplier. A Steele-Vigna constant: a multiplier whose
// spectral properties make it an excellent MCG multiplier, and
// therefore an excellent hash-diffusion constant.
//
// Do not change these without re-running a diffusion test and
// updating the correctness oracle.
#[cfg(target_pointer_width = "64")]
const K: usize = 0xf135_7aea_2e62_a9c5;
#[cfg(target_pointer_width = "32")]
const K: usize = 0x93d7_65dd;

// The finalization rotation amount, in bits. Tuned jointly with `K`
// to achieve good avalanche behavior and to push high-entropy bits
// into the top 7 positions of the output (see the module docs for
// why the top 7 matter).
#[cfg(target_pointer_width = "64")]
const ROTATE: u32 = 26;
#[cfg(target_pointer_width = "32")]
const ROTATE: u32 = 15;

/// A fast, non-cryptographic, non-DoS-resistant hash function used
/// as the default hasher in `koala_std::collections::HashMap`.
///
/// See the [module-level documentation](self) for the full algorithm
/// description, diagrams, and rationale.
///
/// # Examples
///
/// ```
/// # use koala_std::hash::FxHasher;
/// use core::hash::Hasher;
///
/// let mut hasher = FxHasher::new();
/// hasher.write(b"hello world");
/// let h = hasher.finish();
///
/// // Hash is deterministic: same input always yields the same output.
/// let mut again = FxHasher::new();
/// again.write(b"hello world");
/// assert_eq!(h, again.finish());
/// ```
///
/// # Time complexity
///
/// Absorbing an input of length *n* bytes is *O*(*n*). Each full
/// `usize`-sized chunk costs two wrapping arithmetic operations;
/// each trailing byte costs the same two operations per byte.
/// [`FxHasher::finish`] is *O*(1).
pub struct FxHasher {
    hash: usize,
}

impl FxHasher {
    /// Creates a new `FxHasher` initialized to the default state of
    /// zero.
    ///
    /// This is the constructor used by
    /// [`FxHasher::default()`](Default::default) — it is exposed as
    /// an inherent `const fn` so callers can construct an
    /// `FxHasher` at compile time, which the trait
    /// `Default::default` cannot do on stable Rust.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::hash::FxHasher;
    /// use core::hash::Hasher;
    ///
    /// let mut hasher = FxHasher::new();
    /// hasher.write_u64(42);
    /// assert_ne!(hasher.finish(), 0);
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { hash: 0 }
    }

    /// Creates a new `FxHasher` initialized with a caller-provided
    /// seed.
    ///
    /// Two hashers constructed with the same seed and fed the same
    /// input produce the same hash. Two hashers with different
    /// seeds (almost always) produce different hashes for the same
    /// input, which lets callers use `FxHasher` as the building
    /// block for a keyed hash map where each map instance has a
    /// distinct per-instance seed.
    ///
    /// A non-zero seed does **not** add DoS resistance — an
    /// adversary who can observe or guess the seed can still
    /// construct collision sets. For DoS-resistant hashing, use a
    /// cryptographic hasher instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use koala_std::hash::FxHasher;
    /// use core::hash::Hasher;
    ///
    /// let mut a = FxHasher::with_seed(0);
    /// let mut b = FxHasher::with_seed(1);
    /// a.write(b"same input");
    /// b.write(b"same input");
    /// assert_ne!(a.finish(), b.finish());
    /// ```
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[inline]
    #[must_use]
    pub const fn with_seed(seed: usize) -> Self {
        Self { hash: seed }
    }

    /// Absorb a single `usize`-sized chunk of input into the hash
    /// state.
    ///
    /// This is the inner loop of both [`Self::write`] and the
    /// per-width `write_*` methods. The operation is two wrapping
    /// arithmetic steps:
    ///
    /// ```text
    ///   state ← (state + i) * K
    /// ```
    ///
    /// where `K` is the Steele-Vigna multiplier defined at the top
    /// of this module. Both operations wrap on overflow, which is
    /// correct and intentional — the multiply's overflow is the
    /// source of most of the mixing.
    #[inline]
    const fn add_to_hash(&mut self, i: usize) {
        self.hash = self.hash.wrapping_add(i).wrapping_mul(K);
    }
}

impl Default for FxHasher {
    /// Creates a new `FxHasher` with the default state of zero.
    ///
    /// Equivalent to [`FxHasher::new()`], but via the [`Default`]
    /// trait so that `FxHasher` can be used as the type parameter
    /// of [`BuildHasherDefault`] — which is how [`FxBuildHasher`]
    /// is defined, and in turn how
    /// `HashMap<K, V, FxBuildHasher>` works.
    ///
    /// If you want a compile-time constant hasher, prefer
    /// `FxHasher::new()` directly — the inherent method is
    /// `const fn` and this trait method is not.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for FxHasher {
    /// Absorb an arbitrary byte slice into the hash state.
    ///
    /// The slice is processed in full `usize`-sized chunks via
    /// [`slice::chunks_exact`], followed by a byte-at-a-time loop
    /// over any trailing bytes that did not fit in a final chunk.
    /// See the module-level diagram for the overall flow.
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let chunks = bytes.chunks_exact(size_of::<usize>());
        let tail = chunks.remainder();
        for chunk in chunks {
            // `chunks_exact` guarantees each yielded slice has
            // exactly `size_of::<usize>()` bytes, so
            // `try_into().unwrap()` cannot panic.
            self.add_to_hash(usize::from_ne_bytes(chunk.try_into().unwrap()));
        }
        for &byte in tail {
            self.add_to_hash(byte as usize);
        }
    }

    /// Absorb a single byte by widening it to `usize` and calling
    /// `add_to_hash` once.
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    /// Absorb a `u16` by widening it to `usize` and calling
    /// `add_to_hash` once.
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    /// Absorb a `u32` by widening it to `usize` and calling
    /// `add_to_hash` once.
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    /// Absorb a `u64` into the hash state.
    ///
    /// On 64-bit targets, `usize` is already 64 bits wide, so a
    /// single `add_to_hash` call covers the whole value. On 32-bit
    /// targets, `usize` is 32 bits wide, so two `add_to_hash` calls
    /// are needed — one for the low half and one for the high half.
    /// Without the second call on 32-bit, bits `[32, 64)` of the
    /// input would be silently discarded.
    ///
    /// `cast_possible_truncation` is allowed because the truncation
    /// is intentional: on 32-bit, `i as usize` deliberately takes
    /// only the low 32 bits and the high 32 bits are picked up by
    /// the second `add_to_hash` call. On 64-bit the cast is a no-op.
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 32) as usize);
    }

    /// Absorb a `u128` into the hash state.
    ///
    /// On 64-bit targets the low and high `u64` halves are each one
    /// full `usize`, so two `add_to_hash` calls cover all 128 bits.
    /// On 32-bit targets we need four calls because each `usize` is
    /// only 32 bits wide — otherwise bits `[32, 64)` and `[96, 128)`
    /// would be silently dropped.
    ///
    /// `cast_possible_truncation` is allowed because the truncation
    /// is intentional: each `as usize` cast deliberately picks a
    /// `usize`-width slice out of the wider `u128`, and every bit
    /// is covered across the full set of calls.
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn write_u128(&mut self, i: u128) {
        #[cfg(target_pointer_width = "64")]
        {
            self.add_to_hash(i as usize);
            self.add_to_hash((i >> 64) as usize);
        }
        #[cfg(target_pointer_width = "32")]
        {
            self.add_to_hash(i as usize);
            self.add_to_hash((i >> 32) as usize);
            self.add_to_hash((i >> 64) as usize);
            self.add_to_hash((i >> 96) as usize);
        }
    }

    /// Absorb a `usize` by calling `add_to_hash` once. This is the
    /// "native" write — it matches exactly one call to the inner
    /// loop, with no width conversion.
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    /// Finalize the hash and return the result as a `u64`.
    ///
    /// This performs a single rotate of the accumulated state by
    /// `ROTATE` bits before returning. See the module-level diagram
    /// and the rationale there for why the rotate lives in
    /// `finish()` rather than inside `add_to_hash`.
    ///
    /// The cast from `usize` to `u64` is an `as` cast rather than a
    /// `u64::from` call because there is no `From<usize> for u64`
    /// impl in `core` — on a hypothetical 128-bit `usize` platform
    /// the conversion would be lossy and Rust's `From` trait
    /// requires losslessness. On 32-bit and 64-bit targets the cast
    /// is lossless (zero-extension or no-op respectively).
    #[inline]
    fn finish(&self) -> u64 {
        self.hash.rotate_left(ROTATE) as u64
    }
}

/// The [`BuildHasher`](core::hash::BuildHasher) used as the default
/// for `koala_std::collections::HashMap` and `HashSet`.
///
/// This is a thin type alias around [`BuildHasherDefault<FxHasher>`],
/// which is the standard way to build a `BuildHasher` from a
/// [`Hasher`] that implements [`Default`]. Each time the hash map
/// needs a fresh hasher (once per insert, lookup, or remove), it
/// calls `FxBuildHasher::build_hasher()`, which produces a new
/// `FxHasher` via `FxHasher::default()`.
///
/// # Examples
///
/// ```
/// # use koala_std::hash::FxBuildHasher;
/// use core::hash::{BuildHasher, Hasher};
///
/// let builder = FxBuildHasher::default();
/// let mut hasher = builder.build_hasher();
/// hasher.write(b"hello");
/// let _ = hasher.finish();
/// ```
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;
