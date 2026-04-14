// `doc_markdown` is allowed for the same reason as in `fx.rs`: the
// prose discusses algorithm names and crate names in a natural
// register that backtick-ing would make noisier.
#![allow(clippy::doc_markdown)]

//! Hash functions for `koala-std`.
//!
//! Currently exports a single hasher, [`FxHasher`], which is the
//! non-DoS-resistant hasher used as the default for
//! [`koala_std::collections::HashMap`] and `HashSet`. See the
//! [`fx`] module for the full algorithm description, diagrams, and
//! rationale for the design choices.
//!
//! Additional hashers may be added here in the future (a SipHash
//! port for DoS-resistant workloads, or a SeaHash-style alternative
//! for different quality/speed tradeoffs), but the plan for
//! milestone 1 is `FxHasher` only — it is sufficient for every
//! Koala use case identified in the codebase scan.

pub mod fx;

pub use fx::{FxBuildHasher, FxHasher};
