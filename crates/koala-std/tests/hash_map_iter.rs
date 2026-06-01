//! Integration tests for `HashMap`'s iterators (Phase 5, first chunk):
//! `iter`, `iter_mut`, the consuming `IntoIter`, and the three
//! `IntoIterator` impls.
//!
//! Iteration order is unspecified, so every check here is
//! order-independent: collect into a `std` `Vec` and sort, or assert an
//! order-free fact (count, sum, exact size). The `IntoIter` drop test
//! uses boxed values so miri guards the move-out / leave-the-rest path
//! against leaks and double-frees.

use koala_std::collections::HashMap;

/// A map of `0..n` to `i * 2`, the common fixture below.
fn build(n: u64) -> HashMap<u64, u64> {
    let mut map = HashMap::new();
    for i in 0..n {
        let _ = map.insert(i, i * 2);
    }
    map
}

fn sorted<I: IntoIterator<Item = (u64, u64)>>(it: I) -> Vec<(u64, u64)> {
    let mut v: Vec<(u64, u64)> = it.into_iter().collect();
    v.sort_unstable();
    v
}

#[test]
fn iter_visits_every_entry() {
    let map = build(200);
    let got = sorted(map.iter().map(|(k, v)| (*k, *v)));
    let want: Vec<(u64, u64)> = (0..200).map(|i| (i, i * 2)).collect();
    assert_eq!(got, want);
}

#[test]
fn iter_reports_exact_size() {
    let map = build(50);
    let mut it = map.iter();
    assert_eq!(it.len(), 50, "ExactSizeIterator::len before consuming");
    assert_eq!(it.size_hint(), (50, Some(50)));
    let _ = it.next();
    assert_eq!(it.len(), 49, "len must drop by one after a yield");
}

#[test]
fn iter_mut_modifies_values_in_place() {
    let mut map = build(100);
    // Call `iter_mut()` directly (the `&mut map` form is covered elsewhere);
    // `for_each` keeps clippy's explicit-iter-loop lint satisfied.
    map.iter_mut().for_each(|(_, v)| *v += 1);
    for i in 0..100u64 {
        assert_eq!(map.get(&i), Some(&(i * 2 + 1)));
    }
}

#[test]
fn into_iter_yields_every_owned_pair() {
    let map = build(200);
    let got = sorted(map); // consumes the map via IntoIterator
    let want: Vec<(u64, u64)> = (0..200).map(|i| (i, i * 2)).collect();
    assert_eq!(got, want);
}

#[test]
fn reference_into_iterator_forms() {
    let mut map = build(10);
    // &map → borrowed
    let sum: u64 = (&map).into_iter().map(|(_, v)| *v).sum();
    assert_eq!(sum, (0..10).map(|i| i * 2).sum());
    // &mut map → mutable
    for (_, v) in &mut map {
        *v = 0;
    }
    assert!(map.iter().all(|(_, v)| *v == 0));
}

#[test]
fn iterators_on_empty_map_are_immediately_none() {
    let mut map: HashMap<u64, u64> = HashMap::new();
    assert_eq!(map.iter().count(), 0);
    assert_eq!(map.iter_mut().count(), 0);
    assert_eq!(map.into_iter().next(), None);
}

#[test]
fn iter_is_fused_after_exhaustion() {
    let map = build(3);
    let mut it = map.iter();
    for _ in 0..3 {
        assert!(it.next().is_some());
    }
    // Past the end, a fused iterator keeps returning None.
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None);
}

#[test]
fn iter_is_cloneable_and_independent() {
    let map = build(20);
    let it = map.iter();
    let clone = it.clone();
    assert_eq!(sorted(it.map(|(k, v)| (*k, *v))), sorted(clone.map(|(k, v)| (*k, *v))));
}

#[test]
fn into_iter_drops_unyielded_entries_without_leak() {
    // Under miri: take only some entries, then drop the iterator. The
    // entries already yielded are owned by the test; the rest must be freed
    // exactly once by the table's Drop — no leak, no double-free.
    let mut map: HashMap<u64, Box<u64>> = HashMap::new();
    for i in 0..100u64 {
        let _ = map.insert(i, Box::new(i));
    }
    let mut it = map.into_iter();
    let taken: Vec<(u64, Box<u64>)> = it.by_ref().take(10).collect();
    assert_eq!(taken.len(), 10);
    drop(it); // frees the other 90
    // The taken values are still valid here.
    assert!(taken.iter().all(|(k, v)| **v == *k));
}
