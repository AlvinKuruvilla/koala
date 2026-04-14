//! Hand-rolled `no_std` + `alloc` foundation library for Koala.
//!
//! `koala-std` is a learning-motivated replacement for selected types
//! from Rust's standard library. It is explicitly not a drop-in
//! replacement for `std`, not performance-tuned against it, and not
//! intended for external consumption.
//!
//! # Layering
//!
//! `koala-std` sits between `core`/`alloc` (provided by the compiler)
//! and the rest of the Koala workspace:
//!
//! ```text
//! core       rustc-provided: Copy, Sized, Drop, Iterator, primitive ops
//!   ↓
//! alloc      rustc-provided: GlobalAlloc, heap primitives
//!   ↓
//! koala-std  this crate: hash-based collections, browser-grade strings,
//!            arena allocator, and specialized vector types
//!   ↓
//! koala-os   future crate: File, TcpStream, Thread, real Mutex parking
//!   ↓
//! koala-{browser, css, html, dom, js, ...}
//! ```
//!
//! The crate is strictly `#![no_std]` and uses only `alloc` for heap
//! allocation. Importing anything from `std::` inside this crate is a
//! design violation. Syscall-dependent functionality (files, sockets,
//! threads) belongs in the future `koala-os` crate, not here.
//!
//! # Scope
//!
//! The full roadmap and the reasoning behind each decision live in
//! `project-memory/koala-std-roadmap.md`. In brief:
//!
//! - **Milestone 1** — `HashMap<K, V>` and `HashSet<T>`. Hash table
//!   internals are the one place in the "basic collections" layer
//!   that is genuinely algorithmic rather than a thin wrapper over
//!   the allocator. `Vec`, `Box`, and `String` were considered and
//!   dropped from scope — see the roadmap doc's
//!   "Considered and rejected" section for the rationale. An earlier
//!   `Vec<T>` implementation was built across tasks #3–#8 and then
//!   removed on 2026-04-13 after a retrospective; `RawVec<T>`
//!   survives as a `pub(crate)` allocation primitive for milestone 3.
//!
//! - **Milestone 2** — Browser-grade string family: `FlyString`
//!   (interned, `Rc<str>`-backed), `StringBuilder`, `Utf16String`
//!   (for ECMAScript interop), `CowStr`. These sit on top of `std`'s
//!   `Vec<u8>` and `Vec<u16>` as backing storage — they do not need
//!   a custom `String`.
//!
//! - **Milestone 3** — `BumpAllocator`, `SmallVec<T, N>` (inline
//!   storage), `ThinVec<T>` (metadata at the allocation header for
//!   empty-common DOM attribute lists), and `ArenaVec<T>` (tied to
//!   the bump allocator). Extended collections (`VecDeque`,
//!   `BinaryHeap`, `BTreeMap`) are demand-driven only.
//!
//! Refcounting (`Rc`/`Weak`/`Cell`/`RefCell`), concurrency
//! (`Arc`/`Mutex`/atomics), formatting (`Display`/`Debug`/`format!`),
//! and non-OS IO traits (`Read`/`Write`) are **deferred to `std`**
//! and will not be rebuilt here. See the roadmap doc for the full
//! rationale on each.
//!
//! # Testing posture
//!
//! Every public type is validated by a `quickcheck`-driven
//! differential harness against its `std` counterpart, and the crate
//! runs under `miri` in CI (`-Zmiri-strict-provenance`). Zero-sized
//! types and drop-ordering get explicit dedicated tests, because those
//! are the corners where hand-rolled collections most commonly break.

// koala-std is `no_std` when used as a library, but the built-in Rust
// test harness pulls in `std` for `#[test]`, so we conditionally
// disable `no_std` under `cfg(test)`. Production consumers never see
// `std`.
#![cfg_attr(not(test), no_std)]
// koala-std is intentionally unsafe-heavy — every collection type in
// this crate is built on raw pointers and manual allocation. The
// workspace-wide `unsafe_code = "deny"` lint is overridden here
// because denying unsafe would make the crate's entire purpose
// impossible. Unsafe is still reviewed carefully via miri and
// differential testing; it is not reviewed via lint.
#![allow(unsafe_code)]

extern crate alloc;

// `RawVec<T>` is the allocation primitive that originally backed
// `Vec<T>` in milestone 1. `Vec<T>` itself was removed on 2026-04-13
// after the scope retrospective, but `RawVec` stays as `pub(crate)`
// because the milestone-3 vector types (`SmallVec`, `ThinVec`,
// `ArenaVec`) will consume it. Until then the whole module is
// dead-code-allowed at the item level.
mod raw_vec;

pub mod hash;