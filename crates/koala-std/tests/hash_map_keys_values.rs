//! Integration tests for `HashMap::{keys, values, values_mut}` (Phase
//! 5b) — the key/value projections of the entry iterators.
//!
//! As with the entry iterators, order is unspecified, so checks collect
//! and sort or assert order-free facts. The point of these is that the
//! projection picks the right half and preserves the exact size of the
//! underlying walk.

use koala_std::collections::HashMap;

fn build(n: u64) -> HashMap<u64, u64> {
    let mut map = HashMap::new();
    for i in 0..n {
        let _ = map.insert(i, i * 2);
    }
    map
}

fn sorted<I: IntoIterator<Item = u64>>(it: I) -> Vec<u64> {
    let mut v: Vec<u64> = it.into_iter().collect();
    v.sort_unstable();
    v
}

#[test]
fn keys_yields_every_key() {
    let map = build(200);
    assert_eq!(sorted(map.keys().copied()), (0..200).collect::<Vec<_>>());
}

#[test]
fn values_yields_every_value() {
    let map = build(200);
    assert_eq!(sorted(map.values().copied()), (0..200).map(|i| i * 2).collect::<Vec<_>>());
}

#[test]
fn values_mut_modifies_in_place() {
    let mut map = build(100);
    map.values_mut().for_each(|v| *v += 1);
    for i in 0..100u64 {
        assert_eq!(map.get(&i), Some(&(i * 2 + 1)));
    }
}

#[test]
fn projections_report_exact_size() {
    let map = build(50);
    assert_eq!(map.keys().len(), 50);
    assert_eq!(map.values().len(), 50);
    let mut it = map.keys();
    let _ = it.next();
    assert_eq!(it.len(), 49, "len drops by one after a yield");
}

#[test]
fn values_mut_size_is_exact() {
    let mut map = build(30);
    let mut it = map.values_mut();
    assert_eq!(it.size_hint(), (30, Some(30)));
    let _ = it.next();
    assert_eq!(it.size_hint(), (29, Some(29)));
}

#[test]
fn projections_on_empty_map() {
    let mut map: HashMap<u64, u64> = HashMap::new();
    assert_eq!(map.keys().count(), 0);
    assert_eq!(map.values().count(), 0);
    assert_eq!(map.values_mut().count(), 0);
}

#[test]
fn keys_and_values_are_cloneable() {
    let map = build(20);
    let it = map.values();
    let clone = it.clone();
    assert_eq!(sorted(it.copied()), sorted(clone.copied()));
}
