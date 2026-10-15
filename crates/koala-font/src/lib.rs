//! OpenType font parsing for Koala.
//!
//! `koala-font` reads OpenType and TrueType fonts, and faces of font
//! collections, through a [`FontSource`] the caller provides; it does no
//! I/O itself. It replaces `fontdue` one slice at a time, and `fontdue`
//! keeps rasterizing glyphs until this crate can.
//!
//! The first slice reads the font-wide metrics CSS needs to place text
//! and its decorations: the units per em, the ascent, descent and line
//! gap, and the underline and strikeout lines. What to read and why is
//! in `project-memory/koala-font-metrics-reading-guide.md`.
//!
//! A [`Font`] asks its source only for the byte ranges it needs, so
//! answering these questions reads a few hundred bytes of a font file
//! whether the file is in memory or on disk:
//!
//! ```text
//!   caller ── FontSource::read_exact_at(buf, offset) ──► bytes
//!     │            ▲
//!     │            └── Font: collection header, table directory,
//!     │                      head, hhea, OS/2, post
//!     └── Font::from_source(source, settings) ──► Font<S>
//! ```
//!
//! All values are in font units; [`Font::scale_factor`] converts to
//! pixels.
//!
//! The modules follow the specification, OpenType 1.9.1 as Microsoft
//! publishes it: `directory` for the file's structure (the `otff`
//! chapter), and one module per table under `tables`.

// A font file is untrusted input: no read may panic, so nothing indexes
// a slice directly.
#![deny(clippy::indexing_slicing)]

mod directory;
mod error;
mod font;
mod metrics;
mod source;
mod tables;
mod tag;

pub use error::FontError;
pub use font::{Font, FontSettings, face_count};
pub use metrics::{DecorationLine, VerticalMetrics};
pub use source::FontSource;
pub use tag::Tag;
