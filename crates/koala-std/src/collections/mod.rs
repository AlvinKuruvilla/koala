//! Hash-based collections for koala-std.
//!
//! This module is the milestone-1 flagship: a hand-rolled
//! `HashMap<K, V>` and its `HashSet<T>` wrapper, both built on
//! top of a private Robin Hood hash table (see `raw_table`).
//! The design — Robin Hood hashing with inline probe-length
//! tracking, a 70% load factor, backshift deletion, and a
//! `u32` cached hash fragment per bucket — is locked in
//! `project-memory/koala-std-hashmap-design.md`. That document
//! is the source of truth for field layouts, API surface, and
//! invariants; this file is the module-level entry point.
//!
//! # Relationship to other modules
//!
//! `collections/` pairs with `vec/` as the two collection
//! families koala-std owns. `vec/` is the home for specialized
//! vector types (milestone 3), currently containing only the
//! `pub(crate)` `RawVec` allocation primitive. `collections/`
//! is where the hash-based types live — it consumes
//! `koala_std::hash::FxBuildHasher` as the default hasher and
//! has no dependency on `vec/`.
//!
//! # Current state
//!
//! Phase 2 of the implementation plan: the `raw_table`
//! submodule holds the `Bucket<K, V>` / `RawTable<K, V>`
//! primitives. `HashMap<K, V>` (Phase 3) and `HashSet<T>`
//! (Phase 6) do not exist yet.

mod raw_table;
