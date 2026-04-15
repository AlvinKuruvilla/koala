//! Vec-family types for koala-std.
//!
//! In the long run this module is the home for the hand-rolled
//! vector types milestone 3 will bring: `SmallVec<T, N>` with inline
//! storage for the small-count case, `ThinVec<T>` with its metadata
//! folded into the allocation header for empty-common lists like DOM
//! attribute maps, and `ArenaVec<T>` tied to the milestone-3 bump
//! allocator. See `project-memory/koala-std-roadmap.md` for the full
//! milestone-3 story and the rationale for each of those types.
//!
//! In the short run the module is intentionally sparse. An earlier
//! `Vec<T>` was built out across tasks #3–#8 and then removed on
//! 2026-04-13 after a scope retrospective concluded that duplicating
//! `alloc::vec::Vec` was not buying the crate anything a milestone-3
//! specialized vector type would not cover better. What survives
//! from that work is [`RawVec`] — the allocation primitive that
//! handled grow, shrink, ZST quirks, and `Drop`. It is kept
//! `pub(crate)` because the milestone-3 types will consume it
//! unchanged rather than re-derive it from scratch.

mod raw_vec;

// `unused_imports` is allowed because no current consumer imports
// `RawVec` — the milestone-1 `Vec<T>` that did was removed on
// 2026-04-13 and the milestone-3 vector types that will consume it
// haven't landed yet. The re-export is kept so those future types
// get `crate::vec::RawVec` rather than having to reach into the
// private `raw_vec` submodule.
#[allow(unused_imports)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) use raw_vec::RawVec;
