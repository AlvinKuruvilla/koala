//! Correctness tests for `koala_std::hash::FxHasher`.
//!
//! This file uses a **mixed test strategy**:
//!
//! 1. **Oracle tests against `rustc-hash`** — for the paths where
//!    our implementation is bit-for-bit identical to rustc-hash.
//!    These cover every `write_u*` method (integer absorption) and
//!    seeded construction, where the code paths match the upstream
//!    reference exactly.
//!
//! 2. **Determinism + property tests** — for `write(&[u8])`, where
//!    our implementation deliberately deviates from rustc-hash.
//!    See the module docs of `koala_std::hash::fx` for the
//!    rationale; the short version is that rustc-hash's `write()`
//!    calls a wyhash-inspired `hash_bytes` helper that compresses
//!    the slice into a single `u64` via a multiply-mix function
//!    with two parallel state streams, while our simpler
//!    implementation absorbs the slice chunk-by-chunk through
//!    the standard `add_to_hash` loop. Both are correct hash
//!    functions in the general sense (deterministic, reasonable
//!    distribution, fast), they just produce different specific
//!    output values for the same byte input.
//!
//! For the slice path, determinism and "different-inputs-hash-
//! differently" are the contracts we verify. Regression protection
//! against silent output changes comes from the `write_u*` oracle
//! tests, which would catch any drift in the shared `add_to_hash`
//! core — since `write(&[u8])` also uses `add_to_hash`, a change
//! to that core would surface through the integer-path oracle
//! tests regardless of our slice-path implementation choice.
//!
//! If any test here fails, either our implementation has drifted
//! from what it was (investigate) or rustc-hash has updated the
//! specific paths we oracle against (acceptable if intentional,
//! update the test baseline).

use core::hash::Hasher;

use koala_std::hash::FxHasher as KFxHasher;
use quickcheck_macros::quickcheck;
use rustc_hash::FxHasher as ReferenceFxHasher;

// Helpers

/// Apply `body` to a fresh hasher from each implementation and
/// return `(ours, reference)`. Lets test code feed the same
/// sequence of `write_*` calls to both without duplicating the
/// write logic. Used by the oracle tests for integer absorption,
/// which is the path that matches rustc-hash bit-for-bit.
fn hash_both_with<F>(body: F) -> (u64, u64)
where
    F: Fn(&mut dyn Hasher),
{
    let mut k = KFxHasher::new();
    let mut r = ReferenceFxHasher::default();
    body(&mut k);
    body(&mut r);
    (k.finish(), r.finish())
}

/// Hash `bytes` through our implementation only. Used for
/// determinism and property tests where we don't need a cross-
/// implementation comparison.
fn hash_ours(bytes: &[u8]) -> u64 {
    let mut k = KFxHasher::new();
    k.write(bytes);
    k.finish()
}

/// Hash `bytes` through our implementation with a caller-provided
/// seed.
fn hash_ours_with_seed(bytes: &[u8], seed: usize) -> u64 {
    let mut k = KFxHasher::with_seed(seed);
    k.write(bytes);
    k.finish()
}

// Oracle tests — integer absorption (matches rustc-hash exactly)
//
// These tests verify that our `add_to_hash` core and per-width
// write methods are bit-for-bit identical to rustc-hash. Any
// regression in the shared core would surface here even though
// our `write(&[u8])` path diverges.

#[test]
fn write_u8_matches_reference() {
    for value in [0u8, 1, 42, 127, 128, 255] {
        let (ours, theirs) = hash_both_with(|h| h.write_u8(value));
        assert_eq!(ours, theirs, "write_u8({value}) hash mismatch");
    }
}

#[test]
fn write_u16_matches_reference() {
    for value in [0u16, 1, 0xBEEF, 0xFFFF] {
        let (ours, theirs) = hash_both_with(|h| h.write_u16(value));
        assert_eq!(ours, theirs, "write_u16({value:#x}) hash mismatch");
    }
}

#[test]
fn write_u32_matches_reference() {
    for value in [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF] {
        let (ours, theirs) = hash_both_with(|h| h.write_u32(value));
        assert_eq!(ours, theirs, "write_u32({value:#x}) hash mismatch");
    }
}

#[test]
fn write_u64_matches_reference() {
    for value in [0u64, 1, 0xDEAD_BEEF_CAFE_BABE, u64::MAX] {
        let (ours, theirs) = hash_both_with(|h| h.write_u64(value));
        assert_eq!(ours, theirs, "write_u64({value:#x}) hash mismatch");
    }
}

#[test]
fn write_u128_matches_reference() {
    for value in [0u128, 1, 0xDEAD_BEEF_CAFE_BABE_0123_4567_89AB_CDEF, u128::MAX] {
        let (ours, theirs) = hash_both_with(|h| h.write_u128(value));
        assert_eq!(ours, theirs, "write_u128({value:#x}) hash mismatch");
    }
}

#[test]
fn write_usize_matches_reference() {
    for value in [0usize, 1, 42, usize::MAX] {
        let (ours, theirs) = hash_both_with(|h| h.write_usize(value));
        assert_eq!(ours, theirs, "write_usize({value}) hash mismatch");
    }
}

#[test]
fn many_short_writes_match_reference() {
    // 100 individual one-byte writes, all going through
    // `write_u8` which matches rustc-hash.
    let (ours, theirs) = hash_both_with(|h| {
        for i in 0..100u8 {
            h.write_u8(i);
        }
    });
    assert_eq!(ours, theirs, "100 write_u8 sequence hash mismatch");
}

#[quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn random_u64_matches_reference(value: u64) -> bool {
    let (ours, theirs) = hash_both_with(|h| h.write_u64(value));
    ours == theirs
}

#[quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn random_u128_matches_reference(value: u128) -> bool {
    let (ours, theirs) = hash_both_with(|h| h.write_u128(value));
    ours == theirs
}

// Determinism tests — write(&[u8])
//
// These verify that our `write(&[u8])` path is deterministic:
// hashing the same input through two fresh hashers must produce
// the same output. Our implementation deliberately deviates from
// rustc-hash here (see module docs), so we cannot oracle-test
// against rustc-hash for slice inputs; determinism + distribution
// are what we verify instead.

#[test]
fn empty_input_is_deterministic() {
    assert_eq!(hash_ours(b""), hash_ours(b""));
}

#[test]
fn single_byte_is_deterministic() {
    assert_eq!(hash_ours(b"a"), hash_ours(b"a"));
}

#[test]
fn short_input_is_deterministic() {
    assert_eq!(hash_ours(b"hello"), hash_ours(b"hello"));
}

#[test]
fn hello_world_is_deterministic() {
    assert_eq!(hash_ours(b"hello world"), hash_ours(b"hello world"));
}

#[test]
fn exactly_one_chunk_is_deterministic() {
    // 8 bytes on 64-bit — exactly one `size_of::<usize>()` chunk.
    let input = b"01234567";
    assert_eq!(hash_ours(input), hash_ours(input));
}

#[test]
fn chunk_plus_one_is_deterministic() {
    // One full chunk plus a single tail byte.
    let input = b"012345678";
    assert_eq!(hash_ours(input), hash_ours(input));
}

#[test]
fn two_chunks_exact_is_deterministic() {
    let input = b"0123456789abcdef";
    assert_eq!(hash_ours(input), hash_ours(input));
}

#[test]
fn odd_length_is_deterministic() {
    // 17 bytes = 2 full chunks + 1 tail byte on 64-bit.
    let input = b"0123456789abcdefZ";
    assert_eq!(hash_ours(input), hash_ours(input));
}

#[test]
fn all_tail_is_deterministic() {
    // 7 bytes < size_of::<usize>() on 64-bit — all bytes in tail.
    let input = b"tailonl";
    assert_eq!(hash_ours(input), hash_ours(input));
}

#[test]
fn large_input_is_deterministic() {
    let input: Vec<u8> = (0..1019u32).map(|i| (i & 0xFF) as u8).collect();
    assert_eq!(hash_ours(&input), hash_ours(&input));
}

// Distinguishing tests — write(&[u8])
//
// Different inputs should (almost always) produce different
// hashes. These are not a formal collision-freedom guarantee —
// they just catch catastrophic regressions where the hasher
// starts producing the same output for everything.

#[test]
fn different_single_bytes_hash_differently() {
    assert_ne!(hash_ours(b"a"), hash_ours(b"b"));
    assert_ne!(hash_ours(b"a"), hash_ours(b""));
}

#[test]
fn different_short_strings_hash_differently() {
    assert_ne!(hash_ours(b"hello"), hash_ours(b"world"));
    assert_ne!(hash_ours(b"hello"), hash_ours(b"hell"));
    assert_ne!(hash_ours(b"hello"), hash_ours(b"helloo"));
}

#[test]
fn order_matters() {
    // AB and BA should hash differently. If they don't, the
    // hasher is commutative, which means it's essentially
    // reducing to a sum and has catastrophic collision
    // properties.
    assert_ne!(hash_ours(b"AB"), hash_ours(b"BA"));
    assert_ne!(hash_ours(b"abcdefgh"), hash_ours(b"hgfedcba"));
}

#[test]
fn length_matters_for_nonzero_content() {
    // Different-length inputs with the same *non-zero* byte
    // content should hash differently. Zero-byte content is a
    // known limitation: because our `add_to_hash` is
    // `(state + i) * K`, feeding `i = 0` into an initially-zero
    // state leaves the state at 0, and all-zero inputs of any
    // length hash to 0. See the `# Known limitations` section
    // of `koala_std::hash::fx` module docs for the full story.
    assert_ne!(hash_ours(&[1u8; 7]), hash_ours(&[1u8; 8]));
    assert_ne!(hash_ours(&[1u8; 8]), hash_ours(&[1u8; 9]));
    assert_ne!(hash_ours(&[1u8; 16]), hash_ours(&[1u8; 17]));
    assert_ne!(hash_ours(&[0xFFu8; 7]), hash_ours(&[0xFFu8; 8]));
}

/// Regression test documenting the known all-zero-collapse
/// limitation. If this test starts *failing* (i.e., our hasher
/// starts distinguishing all-zero inputs of different lengths),
/// either we've intentionally upgraded `FxHasher` to fix the
/// limitation (update the docs and this test) or something
/// unintentional has changed in the algorithm (investigate).
#[test]
fn all_zero_inputs_all_collapse_to_zero() {
    // Documented limitation: our simpler FxHasher absorbs i=0
    // without changing state, so any all-zero input stays at
    // state=0 and finish() returns 0.rotate_left(26) = 0.
    assert_eq!(hash_ours(&[0u8; 0]), 0);
    assert_eq!(hash_ours(&[0u8; 1]), 0);
    assert_eq!(hash_ours(&[0u8; 8]), 0);
    assert_eq!(hash_ours(&[0u8; 16]), 0);
    assert_eq!(hash_ours(&[0u8; 100]), 0);
}

// Seeded construction — oracle-testable because `with_seed` and
// the integer absorption path both match rustc-hash.

#[test]
fn seeded_with_integer_input_matches_reference() {
    // Seeded construction + write_u64 goes through paths that
    // match rustc-hash bit-for-bit.
    for seed in [0usize, 1, 42, 0xDEAD_BEEF, usize::MAX] {
        let mut k = KFxHasher::with_seed(seed);
        let mut r = ReferenceFxHasher::with_seed(seed);
        k.write_u64(0xCAFE_BABE);
        r.write_u64(0xCAFE_BABE);
        assert_eq!(
            k.finish(),
            r.finish(),
            "seeded write_u64 hash mismatch at seed {seed:#x}"
        );
    }
}

// Seeded determinism — slice path with a non-zero seed

#[test]
fn seeded_slice_is_deterministic() {
    for seed in [0usize, 1, 42, 0xDEAD_BEEF, usize::MAX] {
        assert_eq!(
            hash_ours_with_seed(b"hello world", seed),
            hash_ours_with_seed(b"hello world", seed),
        );
    }
}

#[test]
fn different_seeds_produce_different_hashes() {
    // Two hashers with different seeds and the same input should
    // (almost always) produce different outputs.
    assert_ne!(
        hash_ours_with_seed(b"hello", 0),
        hash_ours_with_seed(b"hello", 1),
    );
    assert_ne!(
        hash_ours_with_seed(b"hello", 42),
        hash_ours_with_seed(b"hello", 43),
    );
}

// Quickcheck property tests

/// Determinism property: hashing the same input twice must
/// produce the same output.
#[quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn write_is_deterministic(bytes: Vec<u8>) -> bool {
    hash_ours(&bytes) == hash_ours(&bytes)
}

/// Seeded determinism: hashing the same input with the same seed
/// must produce the same output.
#[quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn seeded_write_is_deterministic(seed: usize, bytes: Vec<u8>) -> bool {
    hash_ours_with_seed(&bytes, seed) == hash_ours_with_seed(&bytes, seed)
}

/// Appending non-empty, non-all-zero bytes to a non-all-zero
/// prefix changes the hash. We exclude all-zero cases because
/// our simplified `write()` has a known limitation where
/// all-zero input of any length collapses to state=0 — see the
/// `all_zero_inputs_all_collapse_to_zero` regression test and
/// the `# Known limitations` section of `koala_std::hash::fx`.
#[quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn appending_bytes_usually_changes_hash(a: Vec<u8>, b: Vec<u8>) -> bool {
    if b.is_empty() {
        // Appending nothing doesn't change the hash — trivially true.
        return true;
    }
    // Skip cases where both the prefix and the suffix are all-zero
    // — those hit the known limitation and aren't counterexamples
    // to anything we care about.
    let a_all_zero = a.iter().all(|&x| x == 0);
    let b_all_zero = b.iter().all(|&x| x == 0);
    if a_all_zero && b_all_zero {
        return true;
    }
    let mut combined = a.clone();
    combined.extend_from_slice(&b);
    hash_ours(&a) != hash_ours(&combined)
}
