//! A face of a font, and how to open one.

use crate::directory::{Header, TableDirectory};
use crate::error::FontError;
use crate::metrics::{DecorationLine, VerticalMetrics};
use crate::source::FontSource;
use crate::tables::{head, post};

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
    /// `head.unitsPerEm`, in 16..=16384.
    units_per_em: u16,
    /// `post`'s underline fields, if the face has a readable `post`.
    underline: Option<DecorationLine>,
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
        // The tables are read here, not on demand, so the accessors below
        // cannot fail: a face that opens has every value they report.
        // That costs reading `head` and `post` even for a caller that
        // wants only one of them, a few dozen bytes each.
        let directory = TableDirectory::read(&source, settings.collection_index)?;
        let head = head::read(&source, &directory)?;
        let underline = post::read(&source, &directory)?;
        Ok(Self {
            source,
            units_per_em: head.units_per_em,
            underline,
        })
    }

    /// The source this font reads from.
    pub const fn source(&self) -> &S {
        &self.source
    }

    /// `head.unitsPerEm`: font units per em.
    #[must_use]
    pub const fn units_per_em(&self) -> u16 {
        self.units_per_em
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
    pub const fn underline(&self) -> Option<DecorationLine> {
        self.underline
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
        px / f32::from(self.units_per_em)
    }
}

/// How many faces `source` holds: the collection's count, or 1 for a
/// single font.
///
/// # Errors
///
/// As [`Font::from_source`], for the header alone.
pub fn face_count<S: FontSource + ?Sized>(source: &S) -> Result<u32, FontError> {
    Ok(Header::read(source)?.face_count())
}
