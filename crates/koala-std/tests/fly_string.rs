//! Contract tests for `koala_std::string::{FlyString, Interner}`.
//!
//! RED-FIRST SCAFFOLD: written before the bodies exist, so it compiles
//! but fails on `todo!()` until implemented. Run in isolation while
//! filling in the skeleton: `cargo test -p koala-std --test fly_string`.
//!
//! There is no `std::FlyString` to differentially test against, so these
//! verify the contract directly: the interning invariant (equal content
//! ⟺ same handle), pointer-based eq/hash agreement, `Deref`, dedup, and
//! `Send + Sync` / cross-thread use.

use std::thread;

use koala_std::collections::HashMap;
use koala_std::string::{FlyString, Interner};

#[test]
fn equal_content_interns_to_equal_handles() {
    let mut interner = Interner::new();
    let a = interner.intern("div");
    let b = interner.intern("div");
    // Pointer equality: same content from the same interner is one Arc.
    assert_eq!(a, b);
}

#[test]
fn distinct_content_is_not_equal() {
    let mut interner = Interner::new();
    let div = interner.intern("div");
    let span = interner.intern("span");
    assert_ne!(div, span);
}

#[test]
fn deref_and_as_str_round_trip_the_bytes() {
    let mut interner = Interner::new();
    let s = interner.intern("hello");
    assert_eq!(s.as_str(), "hello");
    assert_eq!(&*s, "hello");
    // Methods reachable through Deref<str>.
    assert_eq!(s.len(), 5);
    assert!(s.starts_with("hel"));
}

#[test]
fn interning_deduplicates() {
    let mut interner = Interner::new();
    assert!(interner.is_empty());
    for _ in 0..5 {
        let _ = interner.intern("a");
    }
    assert_eq!(interner.len(), 1, "same string interned once");
    let _ = interner.intern("b");
    assert_eq!(interner.len(), 2);
}

#[test]
fn usable_as_a_hashmap_key() {
    // The payoff: a map keyed by FlyString works because hash agrees
    // with the pointer eq. A handle interned separately but with equal
    // content must hit the same entry.
    let mut interner = Interner::new();
    let k1 = interner.intern("color");
    let k2 = interner.intern("color");

    let mut map: HashMap<FlyString, i32> = HashMap::new();
    let _ = map.insert(k1, 7);
    assert_eq!(map.get(&k2), Some(&7));
}

/// Compile-time assertion: only type-checks if `T: Send + Sync`.
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn flystring_is_send_and_sync() {
    assert_send_sync::<FlyString>();
}

#[test]
fn flystring_moves_across_threads() {
    let mut interner = Interner::new();
    let s = interner.intern("worker");
    let handle = thread::spawn(move || s.as_str().to_owned());
    assert_eq!(handle.join().expect("worker thread did not panic"), "worker");
}
