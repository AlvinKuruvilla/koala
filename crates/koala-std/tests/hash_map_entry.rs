//! Integration tests for the `HashMap::entry` API (Phase 5c).
//!
//! The load-bearing case is `vacant_insert_survives_a_resize`: a vacant
//! insert may grow the table and re-home every entry, so the reference it
//! returns must point at the value's *landing* slot, not at wherever the
//! initial probe looked. The rest exercises the occupied/vacant method
//! surface and the `or_insert` / `and_modify` combinators.

use koala_std::collections::HashMap;

#[test]
fn or_insert_inserts_when_vacant_and_reads_when_occupied() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    assert_eq!(*map.entry("a").or_insert(1), 1, "vacant → inserts default");
    assert_eq!(*map.entry("a").or_insert(99), 1, "occupied → keeps existing");
    assert_eq!(map.len(), 1);
}

#[test]
fn or_insert_returns_a_live_mutable_reference() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    *map.entry("counter").or_insert(0) += 5;
    *map.entry("counter").or_insert(0) += 5;
    assert_eq!(map.get("counter"), Some(&10));
}

#[test]
fn word_count_idiom() {
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for word in ["a", "b", "a", "c", "b", "a"] {
        *counts.entry(word).or_insert(0) += 1;
    }
    assert_eq!(counts.get("a"), Some(&3));
    assert_eq!(counts.get("b"), Some(&2));
    assert_eq!(counts.get("c"), Some(&1));
}

#[test]
fn and_modify_then_or_insert() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    // Vacant: and_modify is a no-op, or_insert supplies the seed.
    let _ = map.entry("x").and_modify(|v| *v += 100).or_insert(1);
    assert_eq!(map.get("x"), Some(&1));
    // Occupied: and_modify runs, or_insert is ignored.
    let _ = map.entry("x").and_modify(|v| *v += 100).or_insert(1);
    assert_eq!(map.get("x"), Some(&101));
}

#[test]
fn or_insert_with_and_or_default() {
    let mut map: HashMap<&str, String> = HashMap::new();
    map.entry("a").or_insert_with(|| "made".to_string()).push('!');
    assert_eq!(map.get("a").map(String::as_str), Some("made!"));

    let mut nums: HashMap<&str, i32> = HashMap::new();
    *nums.entry("z").or_default() += 7;
    assert_eq!(nums.get("z"), Some(&7));
}

#[test]
fn or_insert_with_key_sees_the_key() {
    let mut map: HashMap<&str, usize> = HashMap::new();
    let v = map.entry("hello").or_insert_with_key(|k| k.len());
    assert_eq!(*v, 5);
}

#[test]
fn entry_key_accessor() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    assert_eq!(*map.entry("vacant").key(), "vacant");
    let _ = map.insert("present", 1);
    assert_eq!(*map.entry("present").key(), "present");
}

#[test]
fn occupied_entry_methods() {
    use koala_std::collections::Entry;

    let mut map: HashMap<&str, i32> = HashMap::new();
    let _ = map.insert("a", 1);
    match map.entry("a") {
        Entry::Occupied(mut e) => {
            assert_eq!(*e.key(), "a");
            assert_eq!(*e.get(), 1);
            *e.get_mut() += 10;
            assert_eq!(e.insert(100), 11, "insert returns the previous value");
        }
        Entry::Vacant(_) => panic!("key 'a' should be occupied"),
    }
    assert_eq!(map.get("a"), Some(&100));
}

#[test]
fn occupied_entry_remove() {
    use koala_std::collections::Entry;

    let mut map: HashMap<&str, i32> = HashMap::new();
    let _ = map.insert("a", 1);
    let _ = map.insert("b", 2);
    match map.entry("a") {
        Entry::Occupied(e) => assert_eq!(e.remove(), 1),
        Entry::Vacant(_) => panic!("occupied"),
    }
    assert_eq!(map.get("a"), None);
    assert_eq!(map.get("b"), Some(&2), "the neighbor survives the backshift");
    assert_eq!(map.len(), 1);
}

#[test]
fn vacant_entry_into_key() {
    use koala_std::collections::Entry;

    let mut map: HashMap<String, i32> = HashMap::new();
    match map.entry("owned".to_string()) {
        Entry::Vacant(e) => {
            assert_eq!(e.key(), "owned");
            // into_key hands the owned key back without inserting.
            assert_eq!(e.into_key(), "owned".to_string());
        }
        Entry::Occupied(_) => panic!("vacant"),
    }
    assert!(map.is_empty(), "into_key must not insert");
}

#[test]
fn vacant_insert_survives_a_resize() {
    // Build a map, then for each key use `entry(...).or_insert(...)` on a
    // *fresh* key that forces growth, and demand the returned reference is
    // the right value's slot even though the resize re-homed everything.
    let mut map: HashMap<u64, u64> = HashMap::new();
    for i in 0..500u64 {
        // The reference returned by a vacant or_insert must survive the very
        // grow that this insert may trigger.
        let slot = map.entry(i).or_insert(i * 3);
        assert_eq!(*slot, i * 3, "fresh insert {i} yields its own value");
        *slot += 1; // and it must be writable in place
    }
    assert_eq!(map.len(), 500);
    for i in 0..500u64 {
        assert_eq!(map.get(&i), Some(&(i * 3 + 1)), "entry {i} kept its value through the grows");
    }
}

#[test]
fn entry_drains_boxed_values_cleanly_under_miri() {
    // Mix vacant inserts (which may grow) and occupied removes through the
    // entry API, with boxed values so miri checks the placement + backshift
    // paths for leaks / double-frees.
    use koala_std::collections::Entry;

    let mut map: HashMap<u64, Box<u64>> = HashMap::new();
    for i in 0..100u64 {
        let _ = map.entry(i).or_insert_with(|| Box::new(i));
    }
    for i in (0..100u64).step_by(2) {
        if let Entry::Occupied(e) = map.entry(i) {
            assert_eq!(*e.remove(), i);
        }
    }
    assert_eq!(map.len(), 50);
    for i in 1..100u64 {
        if i % 2 == 1 {
            assert_eq!(map.get(&i).map(|b| **b), Some(i));
        }
    }
}
