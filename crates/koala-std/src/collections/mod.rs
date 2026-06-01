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
//! Phase 3b-i: the `raw_table` submodule holds the
//! `Bucket<K, V>` / `RawTable<K, V>` primitives, and `HashMap<K, V>`
//! exists with its struct, constructors, and size accessors. The
//! probing methods (`insert` / `get` / `remove`) and `HashSet<T>`
//! (Phase 6) do not exist yet.

mod hash_map;
mod raw_table;

pub use hash_map::{HashMap, IntoIter, Iter, IterMut};
