//! A face of a font, and how to open one.

use crate::directory::Header;
use crate::error::FontError;
use crate::metrics::{DecorationLine, VerticalMetrics};
use crate::source::FontSource;

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
    Ok(Header::read(source)?.face_count())
}
