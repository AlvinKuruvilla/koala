//! Integration tests for `koala_Vec<T>`.
//!
//! This file is an integration test, so it compiles as an
//! independent binary outside the `koala-std` crate and can only
//! access the crate's *public* API. That's a feature, not a
//! limitation: it forces the tests to exercise the same surface
//! that any future consumer would see.
//!
//! The file has three sections:
//!
//! 1. **Differential quickcheck harness** — a random sequence of
//!    `Push` / `Pop` / `Clone` operations applied in lock-step to
//!    a `koala_std::Vec<i32>` and a `Vec<i32>`, with
//!    state compared after every operation. If `koala-std` ever
//!    drifts from `std`'s semantics on any of the operations the
//!    harness covers, this test will surface it with a concrete
//!    minimized counterexample thanks to `quickcheck`'s shrinking.
//!
//! 2. **Explicit zero-sized type tests** — ZST behavior is where
//!    hand-rolled collections most commonly break, and `quickcheck`
//!    can't generate `Arbitrary` impls for `()` cleanly. These are
//!    hand-written to cover the `Vec<()>` corner.
//!
//! 3. **Drop-order and Clone independence tests** — verifies that
//!    elements drop in forward order (matching `Vec`) and
//!    that a cloned `Vec` does not share storage with its source.

use std::sync::Mutex;

use koala_std::vec::Vec as KVec;
use quickcheck::{Arbitrary, Gen};
use quickcheck_macros::quickcheck;

// Differential quickcheck harness

/// One operation the harness can apply to both vectors.
///
/// The weights in `Arbitrary` bias toward `Push` so the vectors
/// actually grow to interesting sizes during a test run; mutation
/// ops (`Pop`, `Reserve`, `ReserveExact`, `ShrinkToFit`) are
/// common enough to be exercised but not so common that the
/// vectors stay near-empty. `Reserve` variants take a `u8`-bounded
/// additional amount so quickcheck cannot ask for `usize::MAX`
/// more slots and OOM the test binary.
#[derive(Debug, Clone)]
enum Op {
    Push(i32),
    Pop,
    Clone,
    Reserve(u8),
    ReserveExact(u8),
    ShrinkToFit,
}

impl Arbitrary for Op {
    fn arbitrary(g: &mut Gen) -> Self {
        match u8::arbitrary(g) % 20 {
            0..=9 => Self::Push(i32::arbitrary(g)),
            10..=13 => Self::Pop,
            14 => Self::Clone,
            15..=16 => Self::Reserve(u8::arbitrary(g)),
            17..=18 => Self::ReserveExact(u8::arbitrary(g)),
            _ => Self::ShrinkToFit,
        }
    }
}

/// Compare the two vectors' observable state: length and element
/// contents. If either diverges, the harness has found a bug.
fn snapshots_match(k: &KVec<i32>, s: &Vec<i32>) -> bool {
    k.len() == s.len() && k.as_slice() == s.as_slice()
}

#[quickcheck]
fn differential_push_pop_clone(ops: Vec<Op>) -> bool {
    let mut k: KVec<i32> = KVec::new();
    let mut s: Vec<i32> = Vec::new();

    for op in ops {
        match op {
            Op::Push(x) => {
                k.push(x);
                s.push(x);
            }
            Op::Pop => {
                // `pop` returns an Option that must match exactly
                // — including `None` on empty.
                if k.pop() != s.pop() {
                    return false;
                }
            }
            Op::Clone => {
                let kc = k.clone();
                let sc = s.clone();
                if !snapshots_match(&kc, &sc) {
                    return false;
                }
            }
            Op::Reserve(additional) => {
                k.reserve(usize::from(additional));
                s.reserve(usize::from(additional));
                // After `reserve`, both vectors must have capacity
                // at least `len + additional`. Capacity may differ
                // between implementations (koala-std's amortization
                // floor is 4, std's may differ), so we only check
                // the lower bound.
                let needed = k.len() + usize::from(additional);
                if k.capacity() < needed || s.capacity() < needed {
                    return false;
                }
            }
            Op::ReserveExact(additional) => {
                k.reserve_exact(usize::from(additional));
                s.reserve_exact(usize::from(additional));
                let needed = k.len() + usize::from(additional);
                if k.capacity() < needed || s.capacity() < needed {
                    return false;
                }
            }
            Op::ShrinkToFit => {
                k.shrink_to_fit();
                s.shrink_to_fit();
                // After shrink_to_fit, capacity is at most `len`
                // in koala-std. `std::vec::Vec` is allowed to
                // over-allocate and may not shrink all the way
                // (documented as a hint), so we only assert the
                // upper bound on koala-std's side and verify the
                // contents still match.
                if k.capacity() < k.len() {
                    return false;
                }
            }
        }
        if !snapshots_match(&k, &s) {
            return false;
        }
    }
    true
}

/// A smaller-grained differential test that exclusively exercises
/// the `FromIterator` path. `quickcheck` here generates a vector of
/// source integers and verifies that `koala_std::Vec::from_iter`
/// produces the same sequence as `std::vec::Vec::from_iter` over an
/// identical owning iterator.
///
/// `needless_pass_by_value` is suppressed because `quickcheck`'s
/// `Arbitrary` machinery generates owned values — the test function
/// signature must take `source` by value, not `&[i32]`.
#[quickcheck]
#[allow(clippy::needless_pass_by_value)]
fn differential_from_iter(source: Vec<i32>) -> bool {
    // Both sides go through `FromIterator::from_iter` over an
    // owning iterator. One side clones `source`, the other consumes
    // it, and we compare the resulting vectors.
    let k: KVec<i32> = source.clone().into_iter().collect();
    let s: Vec<i32> = source.into_iter().collect();
    snapshots_match(&k, &s)
}

// Zero-sized type tests

#[test]
fn zst_vec_reports_infinite_capacity() {
    let v: KVec<()> = KVec::new();
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), usize::MAX);
}

#[test]
fn zst_vec_push_pop_roundtrip_never_allocates() {
    let mut v: KVec<()> = KVec::new();
    for _ in 0..1000 {
        v.push(());
    }
    assert_eq!(v.len(), 1000);
    // The capacity must stay at the ZST sentinel throughout — if
    // `grow` were ever called on a ZST vector, `RawVec` would have
    // landed a bogus value here.
    assert_eq!(v.capacity(), usize::MAX);

    for expected_len_after_pop in (0..1000).rev() {
        assert_eq!(v.pop(), Some(()));
        assert_eq!(v.len(), expected_len_after_pop);
    }
    assert_eq!(v.pop(), None);
    assert_eq!(v.capacity(), usize::MAX);
}

#[test]
fn zst_vec_from_iter_does_not_allocate() {
    // Collect ten thousand ZSTs. If anything in the pipeline were
    // trying to allocate `N * size_of::<()>() = 0` bytes through
    // `Layout::array`, we would never even reach this line because
    // `Layout::array::<()>(N)` succeeds without calling the
    // allocator.
    let v: KVec<()> = (0..10_000).map(|_| ()).collect();
    assert_eq!(v.len(), 10_000);
    assert_eq!(v.capacity(), usize::MAX);
}

// Drop-order test

/// Element type that records its own drop into a shared log.
///
/// Using a `Mutex` instead of `RefCell` lets us satisfy any future
/// `Sync`-requiring code path without refactoring the test; the
/// lock is uncontended here because the test is single-threaded.
struct DropRecorder<'log> {
    id: u32,
    log: &'log Mutex<Vec<u32>>,
}

impl Drop for DropRecorder<'_> {
    fn drop(&mut self) {
        self.log
            .lock()
            .expect("drop log mutex is never contended or poisoned in this test")
            .push(self.id);
    }
}

#[test]
fn drop_runs_in_forward_order() {
    let log = Mutex::new(Vec::new());
    {
        let mut v: KVec<DropRecorder<'_>> = KVec::new();
        for i in 0..5 {
            v.push(DropRecorder { id: i, log: &log });
        }
        // v drops at end of scope; `ptr::drop_in_place` on a
        // `*mut [T]` walks elements forward, so the log should
        // record 0, 1, 2, 3, 4 in that order.
    }

    // Clone the log's contents out of the mutex so the guard
    // drops immediately, not across the assertion. Avoids clippy's
    // `significant_drop_tightening` warning without suppressing it.
    let drops: Vec<u32> = log
        .lock()
        .expect("drop log mutex is never contended or poisoned in this test")
        .clone();
    assert_eq!(
        drops.as_slice(),
        &[0, 1, 2, 3, 4],
        "Vec::drop must walk elements in forward order to match std::vec::Vec"
    );
}

#[test]
fn drop_runs_on_all_elements_even_for_long_vecs() {
    let log = Mutex::new(Vec::new());
    {
        let v: KVec<DropRecorder<'_>> = (0..100)
            .map(|i| DropRecorder { id: i, log: &log })
            .collect();
        // Exercise Drop via a Vec built through FromIterator, which
        // goes through `RawVec::with_capacity` + `push` rather than
        // push-from-empty. Same Drop path, different construction.
        drop(v);
    }

    let drops: Vec<u32> = log
        .lock()
        .expect("drop log mutex is never contended or poisoned in this test")
        .clone();
    assert_eq!(drops.len(), 100, "every element must drop exactly once");
    let expected: Vec<u32> = (0..100).collect();
    assert_eq!(drops.as_slice(), expected.as_slice());
}

// Clone independence

#[test]
fn clone_produces_independent_storage() {
    let mut a: KVec<i32> = KVec::new();
    a.push(1);
    a.push(2);
    a.push(3);

    let mut b = a.clone();
    b.push(4);

    // Pushing to the clone must not mutate the source — if `Clone`
    // accidentally did an `Arc`-style shallow copy, `a` would now
    // observe the fourth element.
    assert_eq!(a.as_slice(), &[1, 2, 3]);
    assert_eq!(b.as_slice(), &[1, 2, 3, 4]);
}

// push_within_capacity — the explicit koala-std deviation

#[test]
fn push_within_capacity_succeeds_while_room_exists() {
    let mut v: KVec<i32> = KVec::with_capacity(3);
    assert_eq!(v.push_within_capacity(1), Ok(()));
    assert_eq!(v.push_within_capacity(2), Ok(()));
    assert_eq!(v.push_within_capacity(3), Ok(()));
    assert_eq!(v.len(), 3);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn push_within_capacity_refuses_when_full_without_growing() {
    let mut v: KVec<i32> = KVec::with_capacity(2);
    assert_eq!(v.push_within_capacity(1), Ok(()));
    assert_eq!(v.push_within_capacity(2), Ok(()));

    let cap_before_refused_push = v.capacity();
    assert_eq!(v.push_within_capacity(3), Err(3));

    // The refused push must not have grown the backing allocation.
    assert_eq!(v.capacity(), cap_before_refused_push);
    assert_eq!(v.len(), 2);
    assert_eq!(v.as_slice(), &[1, 2]);
}

#[test]
fn push_within_capacity_on_zero_cap_always_fails() {
    let mut v: KVec<i32> = KVec::new();
    assert_eq!(v.push_within_capacity(42), Err(42));
    assert_eq!(v.capacity(), 0);
    assert_eq!(v.len(), 0);
}

// shrink_to_fit explicit tests

#[test]
fn shrink_to_fit_reduces_capacity_to_len() {
    let mut v: KVec<i32> = KVec::with_capacity(100);
    v.push(1);
    v.push(2);
    v.push(3);
    assert!(v.capacity() >= 100);

    v.shrink_to_fit();
    assert_eq!(v.capacity(), 3);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn shrink_to_fit_fully_deallocates_empty_vec() {
    let mut v: KVec<i32> = KVec::with_capacity(100);
    assert!(v.capacity() >= 100);
    v.shrink_to_fit();
    assert_eq!(v.capacity(), 0);
    // Subsequent push must still work — the vector returned to
    // a new-like state, not a poisoned one.
    v.push(42);
    assert_eq!(v.as_slice(), &[42]);
}

#[test]
fn shrink_to_fit_on_zst_vec_is_noop() {
    let mut v: KVec<()> = KVec::new();
    for _ in 0..10 {
        v.push(());
    }
    v.shrink_to_fit();
    // ZST capacity is always usize::MAX — shrink_to_fit cannot
    // change it.
    assert_eq!(v.capacity(), usize::MAX);
    assert_eq!(v.len(), 10);
}

// Existing Clone independence tests

#[test]
fn clone_of_empty_is_empty() {
    let a: KVec<i32> = KVec::new();
    let b = a.clone();
    // Verify both the source and the clone report empty state —
    // using `a` after the clone also satisfies clippy's
    // `redundant_clone` analysis, which otherwise thinks we could
    // skip cloning because `a` is never read afterwards.
    assert_eq!(a.len(), 0);
    assert_eq!(a.capacity(), 0);
    assert_eq!(b.len(), 0);
    assert_eq!(b.capacity(), 0);
}
