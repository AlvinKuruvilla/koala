//! String types for koala-std (milestone 2).
//!
//! Currently the interned-string family: [`FlyString`] and its backing
//! [`Interner`]. The other planned members — `StringBuilder`,
//! `Utf16String` (ECMAScript interop), `CowStr` — are future work; see
//! `project-memory/koala-std-roadmap.md`.

mod fly;

pub use fly::{FlyString, Interner};
