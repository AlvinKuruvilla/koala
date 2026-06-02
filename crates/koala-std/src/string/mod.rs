//! String types for koala-std (milestone 2).
//!
//! Currently [`FlyString`] — a small-string-optimized, optionally-interned
//! string handle. It depends on the process-global interner and so lives
//! behind the crate's `std` feature; see [`fly`]. The other planned
//! members — `StringBuilder`, `Utf16String` (ECMAScript interop), `CowStr`
//! — are future work and will be `no_std`; see
//! `project-memory/koala-std-roadmap.md`.

#[cfg(feature = "std")]
pub mod fly;

#[cfg(feature = "std")]
pub use fly::FlyString;
