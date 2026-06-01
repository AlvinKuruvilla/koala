//! Integration tests for `HashMap::remove` (Phase 3b-iv) and its Robin
//! Hood backshift.
//!
//! The signature risk of backshift deletion is *collateral*: removing one
//! entry shifts a run of displaced neighbors, and a bug there strands or
//! mis-homes entries that were never touched by name. So beyond the basic
//! return-value checks, these stress the neighbor-preservation property —
//! remove half the keys from a heavily-displaced table and demand the
//! other half still resolve to the right value — and cross-check a mixed
//! insert/remove/get stream against `std`. Boxed values let miri catch any
//! double-free or leak through the move-out + shift path.

use koala_std::collections::HashMap;

#[test]
fn remove_returns_value_then_reports_absent() {
    let mut map = HashMap::new();
    let _ = map.insert("a", 1);
    let _ = map.insert("b", 2);
    assert_eq!(map.remove("a"), Some(1));
    assert_eq!(map.len(), 1);
    assert_eq!(map.remove("a"), None, "second remove of the same key is a miss");
    assert!(!map.contains_key("a"));
    assert_eq!(map.get("b"), Some(&2), "the untouched key survives");
}

#[test]
fn remove_absent_returns_none() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    // Absent on an unallocated table, and absent after allocation.
    assert_eq!(map.remove("missing"), None);
    let _ = map.insert("present", 1);
    assert_eq!(map.remove("missing"), None);
    assert_eq!(map.len(), 1);
}

#[test]
fn remove_then_reinsert_roundtrips() {
    let mut map = HashMap::new();
    let _ = map.insert("k", 1);
    assert_eq!(map.remove("k"), Some(1));
    assert_eq!(map.insert("k", 2), None, "after removal the key is fresh again");
    assert_eq!(map.get("k"), Some(&2));
    assert_eq!(map.len(), 1);
}

#[test]
fn backshift_preserves_displaced_neighbors() {
    // Fill past several grows so the table carries long displaced runs,
    // then delete every even key. Each deletion backshifts the run after
    // it; if the shift loses or mis-homes an entry, an odd key it never
    // named will fail to resolve.
    let mut map = HashMap::new();
    for i in 0..512u64 {
        let _ = map.insert(i, i * 10);
    }
    for i in (0..512u64).step_by(2) {
        assert_eq!(map.remove(&i), Some(i * 10), "even key {i} removes its value");
    }
    assert_eq!(map.len(), 256);
    for i in 0..512u64 {
        if i % 2 == 0 {
            assert_eq!(map.get(&i), None, "even key {i} is gone");
        } else {
            assert_eq!(map.get(&i), Some(&(i * 10)), "odd key {i} survived the backshifts");
        }
    }
}

#[test]
fn remove_frees_boxed_values_without_double_free() {
    // Under miri this is the guard that the move-out + backshift neither
    // drops a value twice nor leaks one. Plain `cargo test` still checks
    // the values are returned intact.
    let mut map: HashMap<u64, Box<u64>> = HashMap::new();
    for i in 0..200u64 {
        let _ = map.insert(i, Box::new(i));
    }
    for i in (0..200u64).step_by(3) {
        assert_eq!(map.remove(&i).as_deref(), Some(&i));
    }
    // The survivors are still readable, and dropping the map at scope end
    // frees exactly them — miri verifies no survivor was already freed.
    for i in 0..200u64 {
        if i % 3 == 0 {
            assert!(map.get(&i).is_none());
        } else {
            assert_eq!(map.get(&i).map(|b| **b), Some(i));
        }
    }
}

#[test]
fn mixed_ops_match_std() {
    use std::collections::HashMap as StdMap;

    let mut koala: HashMap<u64, u64> = HashMap::new();
    let mut reference: StdMap<u64, u64> = StdMap::new();

    // A deterministic stream of inserts, removes, and gets over a small key
    // range, so removals genuinely hit and backshift fires often. Every
    // operation's observable result must match `std` regardless of the two
    // maps' internal layouts.
    let mut state: u64 = 0x0f0f_0f0f_1234_5678;
    for _ in 0..4000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let key = (state >> 40) % 96;
        match (state >> 33) & 0b11 {
            0 | 1 => {
                let value = state;
                assert_eq!(koala.insert(key, value), reference.insert(key, value));
            }
            2 => assert_eq!(koala.remove(&key), reference.remove(&key), "remove({key})"),
            _ => assert_eq!(koala.get(&key), reference.get(&key), "get({key})"),
        }
        assert_eq!(koala.len(), reference.len());
    }
    // Final full-range cross-check of residency.
    for key in 0..96u64 {
        assert_eq!(koala.get(&key), reference.get(&key), "final mismatch at {key}");
    }
}
