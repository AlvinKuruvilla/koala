//! Differential tests for `koala_std::collections::HashSet` (Phase 6).
//!
//! RED-FIRST SCAFFOLD: this file is written before `HashSet` exists,
//! so it will not compile until the implementation lands. It encodes
//! the contract the implementation must satisfy. While implementing,
//! run it in isolation:
//!
//! ```text
//! cargo test -p koala-std --test hash_set
//! ```
//!
//! The other koala-std test targets are independent binaries and still
//! build/run while this one is red.
//!
//! Strategy: `HashSet` is a thin wrapper over `HashMap<T, ()>`, so the
//! interesting risk is not the hash table (already validated for
//! `HashMap`) but that the wrapper forwards correctly and returns the
//! right `bool`s. We pin that by running the same operation stream
//! against `std::collections::HashSet` in lock-step. Iteration order
//! differs between the two, so equality is never compared by iteration
//! order — only by membership, length, and per-op return values.

use std::collections::HashSet as StdSet;

use koala_std::collections::HashSet;

fn build(values: &[i32]) -> HashSet<i32> {
    let mut set = HashSet::new();
    for &v in values {
        let _ = set.insert(v);
    }
    set
}

// Core contract — return values and membership

#[test]
fn insert_returns_true_only_for_new_values() {
    let mut set = HashSet::new();
    // First insert of a value is new -> true; a repeat is not -> false.
    assert!(set.insert(7));
    assert!(!set.insert(7));
    assert!(set.insert(8));
    assert_eq!(set.len(), 2);
}

#[test]
fn contains_reflects_membership() {
    let set = build(&[1, 2, 3]);
    assert!(set.contains(&2));
    assert!(!set.contains(&4));
}

#[test]
fn remove_returns_whether_present() {
    let mut set = build(&[1, 2, 3]);
    assert!(set.remove(&2));
    assert!(!set.remove(&2));
    assert!(!set.contains(&2));
    assert_eq!(set.len(), 2);
}

#[test]
fn new_set_is_empty() {
    let set: HashSet<i32> = HashSet::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
}

#[test]
fn clear_empties_but_keeps_the_set_usable() {
    // The `warning.rs` reset path: a long-lived set that is cleared
    // and reused. Requires `HashMap::clear` underneath.
    let mut set = build(&[10, 20, 30, 40]);
    assert_eq!(set.len(), 4);
    set.clear();
    assert!(set.is_empty());
    assert!(!set.contains(&10));
    // Still usable after clear.
    assert!(set.insert(99));
    assert_eq!(set.len(), 1);
}

// Engine-shape tests — the two real consumers being unblocked

#[test]
fn collect_then_contains_borrowed() {
    // The `DomNode::classes()` shape: collect string slices into a set,
    // then membership-test with a borrowed `&str`. Exercises
    // `FromIterator` and `contains<Q>` with `T = &str`, `Q = str`.
    let classlist = "btn btn-primary is-active";
    let classes: HashSet<&str> = classlist.split(' ').collect();
    assert_eq!(classes.len(), 3);
    assert!(classes.contains("btn-primary"));
    assert!(classes.contains("is-active"));
    assert!(!classes.contains("hidden"));
}

#[test]
fn owned_string_insert_and_clear() {
    // The `warn_once` shape: a `HashSet<String>` of seen keys.
    let mut seen: HashSet<String> = HashSet::new();
    assert!(seen.insert("letter-spacing".to_string()));
    assert!(!seen.insert("letter-spacing".to_string()));
    assert!(seen.contains("letter-spacing"));
    seen.clear();
    assert!(seen.is_empty());
}

// Trait impls (same surface as HashMap)

#[test]
fn from_iter_deduplicates() {
    let set: HashSet<i32> = [1, 2, 2, 3, 3, 3].into_iter().collect();
    assert_eq!(set.len(), 3);
}

#[test]
fn extend_adds_new_only() {
    let mut set = build(&[1, 2]);
    set.extend([2, 3, 4]);
    assert_eq!(set.len(), 4);
    for v in [1, 2, 3, 4] {
        assert!(set.contains(&v));
    }
}

#[test]
fn iter_visits_each_element_once() {
    let set = build(&[5, 6, 7]);
    let mut collected: Vec<i32> = set.iter().copied().collect();
    collected.sort_unstable();
    assert_eq!(collected, vec![5, 6, 7]);
}

#[test]
fn clone_is_an_independent_copy() {
    let original = build(&[1, 2, 3]);
    let mut cloned = original.clone();
    let _ = cloned.insert(4);
    assert_eq!(original.len(), 3);
    assert_eq!(cloned.len(), 4);
    assert!(!original.contains(&4));
}

#[test]
fn equality_is_order_independent() {
    let a = build(&[1, 2, 3]);
    let b = build(&[3, 1, 2]);
    assert_eq!(a, b);
    let c = build(&[1, 2]);
    assert_ne!(a, c);
}

// Lock-step differential against std::HashSet

/// Apply a random op stream to both sets and require they agree on
/// every per-op return value and on membership/length throughout.
/// `i8` keys keep the value range small so the stream is a realistic
/// mix of fresh inserts, repeats, and hits/misses on remove/contains.
#[quickcheck_macros::quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn behaves_like_std_for_random_ops(ops: Vec<(u8, i8)>) -> bool {
    let mut ours: HashSet<i32> = HashSet::new();
    let mut reference: StdSet<i32> = StdSet::new();

    for (tag, raw) in ops {
        let value = i32::from(raw);
        match tag % 5 {
            0 => {
                if ours.insert(value) != reference.insert(value) {
                    return false;
                }
            }
            1 => {
                if ours.remove(&value) != reference.remove(&value) {
                    return false;
                }
            }
            2 => {
                if ours.contains(&value) != reference.contains(&value) {
                    return false;
                }
            }
            3 => {
                if ours.len() != reference.len() {
                    return false;
                }
            }
            _ => {
                // Occasionally clear both and confirm they stay in sync.
                ours.clear();
                reference.clear();
            }
        }
        if ours.len() != reference.len() {
            return false;
        }
    }

    // Final whole-domain membership agreement.
    (i8::MIN..=i8::MAX).all(|v| {
        let value = i32::from(v);
        ours.contains(&value) == reference.contains(&value)
    })
}
