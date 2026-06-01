//! Integration tests for `HashMap::reserve` and `shrink_to_fit`
//! (Phase 4).
//!
//! Both route through the same `resize_to` engine that `insert`'s grow
//! uses, but with caller-chosen targets rather than doubling — so beyond
//! the capacity arithmetic (does the map end up able to hold the right
//! number of entries?) these demand that the re-home preserves every
//! entry's value, the same property the grow path needs but now driven
//! through an explicit-target resize and, for `shrink_to_fit`, a resize
//! that makes the table *smaller*.

use koala_std::collections::HashMap;

#[test]
fn reserve_covers_requested_headroom() {
    let mut map: HashMap<usize, usize> = HashMap::new();
    map.reserve(100);
    let cap = map.capacity();
    assert!(cap >= 100, "reserve(100) must leave room for 100 entries, got {cap}");
    // Filling exactly to the reserved capacity must not trigger another resize.
    for i in 0..cap {
        assert_eq!(map.insert(i, i), None);
    }
    assert_eq!(map.capacity(), cap, "filling reserved capacity should not resize");
}

#[test]
fn reserve_accounts_for_existing_entries() {
    let mut map: HashMap<i32, i32> = HashMap::new();
    for i in 0..50 {
        let _ = map.insert(i, i);
    }
    map.reserve(100);
    assert!(
        map.capacity() >= 150,
        "capacity must cover the 50 live entries plus 100 reserved; got {}",
        map.capacity()
    );
}

#[test]
fn reserve_is_noop_when_capacity_already_suffices() {
    let mut map: HashMap<i32, i32> = HashMap::with_capacity(100);
    let before = map.capacity();
    map.reserve(10); // 0 live + 10 << existing capacity
    assert_eq!(map.capacity(), before, "reserve must not grow when there is already room");
}

#[test]
fn reserve_preserves_all_entries_through_the_resize() {
    let mut map: HashMap<u64, u64> = HashMap::new();
    for i in 0..200u64 {
        let _ = map.insert(i, i * 7);
    }
    // A reserve large enough to force a re-home into a bigger backing.
    map.reserve(1000);
    assert!(map.capacity() >= 1200);
    assert_eq!(map.len(), 200);
    for i in 0..200u64 {
        assert_eq!(map.get(&i), Some(&(i * 7)), "entry {i} must survive the reserve");
    }
}

#[test]
fn shrink_to_fit_reduces_backing_but_keeps_entries() {
    let mut map: HashMap<u64, u64> = HashMap::with_capacity(1000);
    let roomy = map.capacity();
    for i in 0..10u64 {
        let _ = map.insert(i, i * 3);
    }
    map.shrink_to_fit();
    let tight = map.capacity();
    assert!(tight < roomy, "shrink must reduce capacity: {tight} should be < {roomy}");
    assert!(tight >= map.len(), "but it must still hold the live entries");
    for i in 0..10u64 {
        assert_eq!(map.get(&i), Some(&(i * 3)), "entry {i} must survive the shrink");
    }
}

#[test]
fn shrink_to_fit_is_idempotent() {
    let mut map: HashMap<u64, u64> = HashMap::with_capacity(1000);
    for i in 0..10u64 {
        let _ = map.insert(i, i);
    }
    map.shrink_to_fit();
    let once = map.capacity();
    map.shrink_to_fit();
    assert_eq!(map.capacity(), once, "a second shrink with no change must be a no-op");
}

#[test]
fn shrink_then_grow_roundtrips() {
    let mut map: HashMap<u64, u64> = HashMap::with_capacity(500);
    for i in 0..5u64 {
        let _ = map.insert(i, i);
    }
    map.shrink_to_fit();
    // Inserting past the shrunk capacity must grow cleanly and keep everything.
    for i in 5..300u64 {
        let _ = map.insert(i, i);
    }
    assert_eq!(map.len(), 300);
    for i in 0..300u64 {
        assert_eq!(map.get(&i), Some(&i));
    }
}

#[test]
fn reserve_and_shrink_preserve_boxed_values_under_miri() {
    // Boxed values let miri verify the explicit-target re-home (both
    // directions) neither double-frees nor leaks.
    let mut map: HashMap<u64, Box<u64>> = HashMap::new();
    map.reserve(300);
    for i in 0..150u64 {
        let _ = map.insert(i, Box::new(i));
    }
    map.shrink_to_fit();
    for i in 0..150u64 {
        assert_eq!(map.get(&i).map(|b| **b), Some(i));
    }
}
