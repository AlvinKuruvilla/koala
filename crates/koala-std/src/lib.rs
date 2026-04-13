//! Hand-rolled `no_std` + `alloc` foundation library for Koala.
//!
//! `koala-std` is a learning-motivated replacement for selected types from
//! Rust's standard library. It is explicitly not a drop-in replacement for
//! `std`, not performance-tuned against it, and not intended for external
//! consumption.
//!
//! # Layering
//!
//! `koala-std` sits between `core`/`alloc` (provided by the compiler) and
//! the rest of the Koala workspace:
//!
//! ```text
//! core       rustc-provided: Copy, Sized, Drop, Iterator, primitive ops
//!   ↓
//! alloc      rustc-provided: GlobalAlloc, heap primitives
//!   ↓
//! koala-std  this crate: collections, strings, arena allocator
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
//! The full roadmap lives in
//! `project-memory/project_build_from_scratch.md`. In brief:
//!
//! - **Milestone 1** — `Vec`, `Box`, `String`, `HashMap`, `HashSet`.
//!   Owned heap storage, no sharing, no concurrency.
//! - **Milestone 2** — Browser-grade string family: `FlyString`,
//!   `StringBuilder`, `Utf16String`, `CowStr`.
//! - **Milestone 3** — `BumpAllocator` / arena. Extended collections
//!   (`VecDeque`, `BinaryHeap`, `BTreeMap`) only when profiling
//!   justifies them.
//!
//! Refcounting (`Rc`/`Weak`/`Cell`/`RefCell`), concurrency
//! (`Arc`/`Mutex`/atomics), formatting (`Display`/`Debug`/`format!`),
//! and non-OS IO traits (`Read`/`Write`) are **deferred to `std`** and
//! will not be rebuilt here. See the memory file for the rationale.
//!
//! # Testing posture
//!
//! Every public type is validated by a `quickcheck`-driven differential
//! harness against its `std` counterpart, and the crate runs under
//! `miri` in CI (`-Zmiri-strict-provenance`). Zero-sized types and
//! drop-ordering get explicit dedicated tests, because those are the
//! corners where hand-rolled collections most commonly break.

// koala-std is `no_std` when used as a library, but the built-in Rust
// test harness pulls in `std` for `#[test]`, so we conditionally disable
// `no_std` under `cfg(test)`. Production consumers never see `std`.
#![cfg_attr(not(test), no_std)]
// koala-std is intentionally unsafe-heavy — every collection type in this
// crate is built on raw pointers and manual allocation. The workspace-wide
// `unsafe_code = "deny"` lint is overridden here because denying unsafe
// would make the crate's entire purpose impossible. Unsafe is still
// reviewed carefully via miri and differential testing; it is not
// reviewed via lint.
#![allow(unsafe_code)]

extern crate alloc;
pub mod vec;
mod raw_vec;
