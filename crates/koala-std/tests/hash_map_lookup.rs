//! Integration tests for the `HashMap` lookup family (Phase 3b-iii):
//! `get`, `get_mut`, and `contains_key`.
//!
//! These are the value-readback checks the `insert`-only suite
//! (`hash_map.rs`) could not make: that every entry is *findable* with
//! the right value after arbitrary displacement and many grows, that a
//! borrowed query (`&str` against a `HashMap<String, _>`) finds the
//! owned key, and that a mutation through `get_mut` is observable. As
//! with the insert suite, running these under miri is what guards the
//! lookup-walk `unsafe` against UB.

use koala_std::collections::HashMap;

#[test]
fn get_returns_inserted_value() {
    let mut map = HashMap::new();
    let _ = map.insert("a", 1);
    let _ = map.insert("b", 2);
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.get("b"), Some(&2));
}

#[test]
fn get_absent_key_returns_none() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    // Absent on an empty (unallocated) table, and absent after the table
    // has buckets but not this key.
    assert_eq!(map.get("missing"), None);
    let _ = map.insert("present", 1);
    assert_eq!(map.get("missing"), None);
}

#[test]
fn get_mut_allows_mutation() {
    let mut map = HashMap::new();
    let _ = map.insert("counter", 0);
    *map.get_mut("counter").expect("just inserted") += 5;
    assert_eq!(map.get("counter"), Some(&5));
    assert_eq!(map.get_mut("absent"), None);
}

#[test]
fn contains_key_tracks_membership() {
    let mut map = HashMap::new();
    assert!(!map.contains_key("x"));
    let _ = map.insert("x", 10);
    assert!(map.contains_key("x"));
    assert!(!map.contains_key("y"));
}

#[test]
fn get_finds_owned_key_via_borrowed_query() {
    // The `Borrow<Q>` bound is what lets a `String`-keyed map be queried
    // with a `&str` without allocating an owned `String` per lookup.
    let mut map: HashMap<String, i32> = HashMap::new();
    let _ = map.insert("hello".to_string(), 42);
    assert_eq!(map.get("hello"), Some(&42));
    assert!(map.contains_key("hello"));
    assert_eq!(map.get("goodbye"), None);
}

#[test]
fn every_key_findable_with_right_value_after_many_grows() {
    let mut map = HashMap::new();
    // 256 distinct keys forces a chain of grows (8 → … → 512 buckets) and
    // heavy Robin Hood displacement; the point of the test is that no
    // entry is lost or mis-associated through all that shuffling.
    for i in 0..256u64 {
        let _ = map.insert(i, i * 2);
    }
    for i in 0..256u64 {
        assert_eq!(map.get(&i), Some(&(i * 2)), "key {i} must survive the grows");
    }
    // Keys never inserted are reported absent, even though the table is full.
    for i in 256..300u64 {
        assert_eq!(map.get(&i), None, "key {i} was never inserted");
    }
}

#[test]
fn lookup_values_match_std() {
    use std::collections::HashMap as StdMap;

    let mut koala: HashMap<u64, u64> = HashMap::new();
    let mut reference: StdMap<u64, u64> = StdMap::new();

    // Same deterministic LCG mix of fresh inserts and overwrites as the
    // insert suite, but here we cross-check the *resident* value after the
    // whole stream — `get` must agree with `std` for every key in range,
    // present or not.
    let mut state: u64 = 0x1234_5678_9abc_def0;
    for _ in 0..1500 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let key = (state >> 33) % 128;
        let value = state;
        let _ = koala.insert(key, value);
        let _ = reference.insert(key, value);
    }
    for key in 0..128u64 {
        assert_eq!(koala.get(&key), reference.get(&key), "mismatch at key {key}");
    }
}
