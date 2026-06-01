//! Integration tests for `koala_std::collections::HashMap::insert`.
//!
//! `get`/`remove` do not exist yet (Phases 3b-iii / 3b-iv), so these
//! validate what `insert` alone exposes: its return value, `len`, and
//! `capacity` growth — plus a differential check of the return values
//! against `std::collections::HashMap`. The deeper "every entry is
//! findable with the right value after many grows" check arrives with
//! `get`. Running this suite under miri is what currently guards the
//! displacement and rehash `unsafe` against UB.

use koala_std::collections::HashMap;

#[test]
fn insert_new_key_returns_none() {
    let mut map = HashMap::new();
    assert_eq!(map.insert("a", 1), None);
    assert_eq!(map.insert("b", 2), None);
    assert_eq!(map.len(), 2);
}

#[test]
fn insert_existing_key_returns_old_value_and_keeps_len() {
    let mut map = HashMap::new();
    assert_eq!(map.insert("k", 1), None);
    assert_eq!(map.insert("k", 2), Some(1));
    assert_eq!(map.insert("k", 3), Some(2));
    assert_eq!(map.len(), 1, "overwriting an existing key must not grow len");
}

#[test]
fn len_tracks_distinct_keys_through_many_grows() {
    let mut map = HashMap::new();
    // 256 distinct keys forces a chain of grows (8 → 16 → … → 512 buckets)
    // and exercises Robin Hood displacement as each table fills.
    for i in 0..256 {
        assert_eq!(map.insert(i, i * 2), None, "key {i} should be new");
    }
    assert_eq!(map.len(), 256);

    // Re-inserting every key returns the previous value and leaves len fixed.
    for i in 0..256 {
        assert_eq!(map.insert(i, i * 3), Some(i * 2));
    }
    assert_eq!(map.len(), 256);
}

#[test]
fn capacity_grows_to_hold_inserted_entries() {
    let mut map = HashMap::new();
    assert_eq!(map.capacity(), 0, "an unallocated map reports zero capacity");
    for i in 0..100 {
        assert_eq!(map.insert(i, i), None, "key {i} is distinct, so a fresh insert");
    }
    assert!(
        map.capacity() >= 100,
        "capacity must cover the live entries; got {}",
        map.capacity()
    );
}

#[test]
fn with_capacity_avoids_growth_for_known_size() {
    let mut map = HashMap::with_capacity(100);
    let initial = map.capacity();
    for i in 0..initial {
        assert_eq!(map.insert(i, i), None);
    }
    // Filling exactly to the reserved capacity must not have resized.
    assert_eq!(map.capacity(), initial);
    assert_eq!(map.len(), initial);
}

#[test]
fn insert_return_values_match_std() {
    use std::collections::HashMap as StdMap;

    let mut koala: HashMap<u64, u64> = HashMap::new();
    let mut reference: StdMap<u64, u64> = StdMap::new();

    // A deterministic LCG sequence with a small key range, so the stream is
    // a realistic mix of fresh inserts and overwrites. For every operation
    // the two maps must agree on the returned previous value — that depends
    // only on whether the key was already present, which is identical for
    // both regardless of their internal hashing.
    let mut state: u64 = 0x1234_5678_9abc_def0;
    for _ in 0..1500 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let key = (state >> 33) % 128;
        let value = state;
        assert_eq!(koala.insert(key, value), reference.insert(key, value));
    }
    assert_eq!(koala.len(), reference.len());
}
