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

// A font file is untrusted input: no read may panic, so nothing indexes
// a slice directly.
#![deny(clippy::indexing_slicing)]

use std::fmt;
use std::io;
use std::sync::Arc;

/// Where a font's bytes come from.
///
/// The signature is `std::os::unix::fs::FileExt::read_exact_at`'s, so a
/// file can be a source through positioned reads, without loading it.
/// Bytes already in memory are a source as they are.
pub trait FontSource {
    /// Fill `buf` with the bytes starting at `offset`.
    ///
    /// # Errors
    ///
    /// If fewer than `buf.len()` bytes exist from `offset` on, an error
    /// of kind [`io::ErrorKind::UnexpectedEof`]; any other error the
    /// underlying storage reports.
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
}

impl FontSource for [u8] {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let _ = (buf, offset);
        todo!("copy the requested range, or UnexpectedEof past the end")
    }
}

impl FontSource for Vec<u8> {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.as_slice().read_exact_at(buf, offset)
    }
}

impl FontSource for Arc<[u8]> {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        (**self).read_exact_at(buf, offset)
    }
}

/// How to open a font.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FontSettings {
    /// Which face of a font collection to open. `0` for a file that
    /// holds a single font.
    pub collection_index: u32,
}

/// One face of a font, read on demand from its source.
pub struct Font<S> {
    source: S,
}

impl<S: FontSource> Font<S> {
    /// Open the face `settings.collection_index` of the font in `source`.
    ///
    /// # Errors
    ///
    /// See [`FontError`]: the source could not be read, it is not an
    /// OpenType font or collection, the face does not exist, or a table
    /// every face needs is missing or too short.
    pub fn from_source(source: S, settings: FontSettings) -> Result<Self, FontError> {
        let _ = (source, settings);
        todo!("read the collection header if any, the table directory, and the v0 tables")
    }

    /// The source this font reads from.
    pub const fn source(&self) -> &S {
        &self.source
    }

    /// `head.unitsPerEm`: font units per em.
    #[must_use]
    pub fn units_per_em(&self) -> u16 {
        todo!()
    }

    /// `hhea`'s `ascender`, `descender` and `lineGap`.
    #[must_use]
    pub fn hhea_metrics(&self) -> VerticalMetrics {
        todo!()
    }

    /// `OS/2`'s `sTypoAscender`, `sTypoDescender` and `sTypoLineGap`, or
    /// `None` if the font has no `OS/2` table or its table stops before
    /// them.
    #[must_use]
    pub fn typo_metrics(&self) -> Option<VerticalMetrics> {
        todo!()
    }

    /// `post`'s `underlinePosition` and `underlineThickness`, or `None`
    /// if the font has no `post` table.
    #[must_use]
    pub fn underline(&self) -> Option<DecorationLine> {
        todo!()
    }

    /// `OS/2`'s `yStrikeoutPosition` and `yStrikeoutSize`, or `None` if
    /// the font has no `OS/2` table.
    #[must_use]
    pub fn strikeout(&self) -> Option<DecorationLine> {
        todo!()
    }

    /// Pixels per font unit at a font size of `px` pixels per em.
    #[must_use]
    pub fn scale_factor(&self, px: f32) -> f32 {
        let _ = px;
        todo!()
    }
}

/// How many faces `source` holds: the collection's count, or 1 for a
/// single font.
///
/// # Errors
///
/// As [`Font::from_source`], for the header alone.
pub fn face_count<S: FontSource + ?Sized>(source: &S) -> Result<u32, FontError> {
    let _ = source;
    todo!("1 for a single font, the TTC header's count for a collection")
}

/// Vertical metrics of a face, in font units: distances above (positive)
/// and below (negative) the baseline, and the gap between lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerticalMetrics {
    /// Distance from the baseline to the top of the face's line.
    pub ascender: i16,
    /// Distance from the baseline to the bottom of the face's line;
    /// negative below the baseline.
    pub descender: i16,
    /// Recommended extra space between lines.
    pub line_gap: i16,
}

/// A decoration line's placement, in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationLine {
    /// Position relative to the baseline, positive upward. See the
    /// `post` and `OS/2` field descriptions for which edge of the line
    /// it gives.
    pub position: i16,
    /// The line's thickness.
    pub thickness: i16,
}

/// A table tag, such as `head` or `OS/2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag(pub [u8; 4]);

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &byte in &self.0 {
            if byte.is_ascii_graphic() || byte == b' ' {
                write!(f, "{}", char::from(byte))?;
            } else {
                write!(f, "\\x{byte:02x}")?;
            }
        }
        Ok(())
    }
}

/// Why a font could not be opened. Each is a reason to fall back to
/// another font.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// The source failed, or ended where the font's structure says more
    /// bytes follow.
    #[error("reading the font failed: {0}")]
    Source(#[from] io::Error),
    /// The first four bytes are neither an OpenType `sfntVersion` nor
    /// `ttcf`.
    #[error("not an OpenType font or font collection: it starts with '{0}'")]
    UnrecognizedSignature(Tag),
    /// The requested face is past the end of the collection.
    #[error("face {index} was requested, but the font holds {count}")]
    FaceIndexOutOfRange {
        /// The face asked for.
        index: u32,
        /// How many faces the font holds.
        count: u32,
    },
    /// A table every face needs is not in the table directory.
    #[error("the font has no '{0}' table")]
    MissingTable(Tag),
    /// A table is shorter than the fields read from it.
    #[error("the '{tag}' table is {length} bytes, too short for its fields")]
    TableTooShort {
        /// The table.
        tag: Tag,
        /// Its length in the table directory.
        length: u32,
    },
}
