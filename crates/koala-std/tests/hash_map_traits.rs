//! Integration tests for `HashMap`'s standard trait impls (Phase 5d):
//! `Clone`, `Debug`, `PartialEq` / `Eq`, `Extend`, and `FromIterator`.

use koala_std::collections::HashMap;

fn build(n: u64) -> HashMap<u64, u64> {
    let mut map = HashMap::new();
    for i in 0..n {
        let _ = map.insert(i, i * 2);
    }
    map
}

#[test]
fn clone_preserves_all_entries() {
    let original = build(200);
    let cloned = original.clone();
    assert_eq!(cloned.len(), original.len());
    for i in 0..200u64 {
        assert_eq!(cloned.get(&i), Some(&(i * 2)));
    }
}

#[test]
fn clone_is_independent_of_original() {
    let mut original = build(10);
    let cloned = original.clone();
    // Mutating the original must not touch the clone.
    let _ = original.insert(0, 9999);
    let _ = original.insert(100, 100);
    assert_eq!(cloned.get(&0), Some(&0), "clone keeps the old value");
    assert_eq!(cloned.get(&100), None, "clone does not see new keys");
    assert_eq!(cloned.len(), 10);
}

#[test]
fn clone_of_boxed_values_does_not_alias() {
    // Under miri: the clone deep-copies each value, so dropping both maps
    // frees distinct allocations — no double-free, no leak.
    let mut original: HashMap<u64, Box<u64>> = HashMap::new();
    for i in 0..50u64 {
        let _ = original.insert(i, Box::new(i));
    }
    let cloned = original.clone();
    drop(original);
    // The clone's values are still valid after the original is gone.
    for i in 0..50u64 {
        assert_eq!(cloned.get(&i).map(|b| **b), Some(i));
    }
}

#[test]
fn debug_formats_like_std() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    let _ = map.insert("a", 1);
    // A single entry formats deterministically.
    assert_eq!(format!("{map:?}"), r#"{"a": 1}"#);

    let empty: HashMap<&str, i32> = HashMap::new();
    assert_eq!(format!("{empty:?}"), "{}");
}

#[test]
fn equality_is_order_independent() {
    let mut a: HashMap<u64, u64> = HashMap::new();
    let mut b: HashMap<u64, u64> = HashMap::new();
    for i in 0..100u64 {
        let _ = a.insert(i, i * 2);
    }
    // Same pairs, reverse insertion order → different internal layout.
    for i in (0..100u64).rev() {
        let _ = b.insert(i, i * 2);
    }
    assert_eq!(a, b);
}

#[test]
fn inequality_on_value_key_and_length() {
    let base = build(50);

    let mut diff_value = base.clone();
    let _ = diff_value.insert(10, 0);
    assert_ne!(base, diff_value, "a differing value breaks equality");

    let mut diff_key = base.clone();
    let _ = diff_key.remove(&10);
    let _ = diff_key.insert(999, 20);
    assert_ne!(base, diff_key, "same length, different keys are not equal");

    let mut shorter = base.clone();
    let _ = shorter.remove(&10);
    assert_ne!(base, shorter, "differing length is not equal");
}

#[test]
fn extend_inserts_and_overwrites() {
    let mut map = build(5);
    // New keys plus an overwrite of an existing one.
    map.extend([(5u64, 10u64), (6, 12), (0, 999)]);
    assert_eq!(map.len(), 7);
    assert_eq!(map.get(&0), Some(&999), "extend overwrites existing keys");
    assert_eq!(map.get(&5), Some(&10));
    assert_eq!(map.get(&6), Some(&12));
}

#[test]
fn extend_from_references() {
    let mut target = build(3);
    let source = build(6); // keys 0..6
    target.extend(&source); // the (&K, &V) Copy variant
    assert_eq!(target.len(), 6);
    for i in 0..6u64 {
        assert_eq!(target.get(&i), Some(&(i * 2)));
    }
}

#[test]
fn from_iterator_builds_the_map() {
    let map: HashMap<u64, u64> = (0..100).map(|i| (i, i * 2)).collect();
    assert_eq!(map.len(), 100);
    for i in 0..100u64 {
        assert_eq!(map.get(&i), Some(&(i * 2)));
    }
}

#[test]
fn from_iterator_keeps_last_on_duplicate_keys() {
    // Matching std: a later pair wins.
    let map: HashMap<u64, u64> = [(1u64, 1u64), (1, 2), (1, 3)].into_iter().collect();
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&1), Some(&3));
}

// `Send` + `Sync`, inherited from the manual `RawTable` impls. The
// engine needs both: a `HashMap` is `move`d into a render worker
// thread (`Send`), and the HTML named-entity table is a
// `static LazyLock<HashMap<&'static str, &'static str>>` (`Sync`, since
// statics must be `Sync`). These tests pin both guarantees so a future
// change to `RawTable`'s fields that silently broke them would fail
// here rather than at a distant call site.

/// Compile-time assertion: the call only type-checks if `T: Send + Sync`.
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn hash_map_is_send_and_sync() {
    assert_send_sync::<HashMap<u64, u64>>();
    // The exact shape the HTML entity-table static uses.
    assert_send_sync::<HashMap<&'static str, &'static str>>();
}

#[test]
fn hash_map_can_move_to_another_thread() {
    // `Send`: transfer ownership into a spawned thread and read it back.
    let map = build(500);
    let handle = std::thread::spawn(move || {
        let mut sum = 0u64;
        for i in 0..500u64 {
            sum += *map.get(&i).expect("entry present after move across threads");
        }
        sum
    });
    let sum = handle.join().expect("worker thread did not panic");
    // sum of i*2 for i in 0..500
    assert_eq!(sum, (0..500u64).map(|i| i * 2).sum());
}

#[test]
fn hash_map_can_be_shared_by_ref_across_threads() {
    use std::sync::Arc;

    // `Sync`: share a single `&` across several threads doing concurrent
    // reads. This is the static-table access pattern in miniature.
    let map = Arc::new(build(1000));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let map = Arc::clone(&map);
            std::thread::spawn(move || {
                for i in 0..1000u64 {
                    assert_eq!(map.get(&i), Some(&(i * 2)));
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("reader thread did not panic");
    }
}
