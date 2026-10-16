//! A face of a font, and how to open one.

use crate::directory::{Header, TableDirectory};
use crate::error::FontError;
use crate::metrics::{DecorationLine, VerticalMetrics};
use crate::source::FontSource;
use crate::tables::{self, Head, Hhea, Os2, Post};

/// How to open a font.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FontSettings {
    /// Which face of a font collection to open. `0` for a file that
    /// holds a single font.
    pub collection_index: u32,
}

/// One face of a font: its font-wide tables, and the source to read
/// more from.
pub struct Font<S> {
    source: S,
    head: Head,
    hhea: Hhea,
    os2: Option<Os2>,
    post: Option<Post>,
}

impl<S: FontSource> Font<S> {
    /// Open the face `settings.collection_index` of the font in `source`.
    ///
    /// # Errors
    ///
    /// See [`FontError`]: the source could not be read, it is not an
    /// OpenType font or collection, the face does not exist, or a table
    /// every face needs is missing, too short or invalid.
    pub fn from_source(source: S, settings: FontSettings) -> Result<Self, FontError> {
        // The tables are read here, not on demand, so the accessors below
        // cannot fail: a face that opens has every value they report.
        // That costs reading all four even for a caller that wants one,
        // about 220 bytes.
        let directory = TableDirectory::read(&source, settings.collection_index)?;
        Ok(Self {
            head: tables::read_head(&source, &directory)?,
            hhea: tables::read_hhea(&source, &directory)?,
            os2: tables::read_os2(&source, &directory)?,
            post: tables::read_post(&source, &directory)?,
            source,
        })
    }

    /// The source this font reads from.
    pub const fn source(&self) -> &S {
        &self.source
    }

    /// The `head` table. Every face that opens has one.
    pub const fn head(&self) -> &Head {
        &self.head
    }

    /// The `hhea` table. Every face that opens has one.
    pub const fn hhea(&self) -> &Hhea {
        &self.hhea
    }

    /// The `OS/2` table, or `None` if the face has none or one too short
    /// for the fields every version has.
    pub const fn os2(&self) -> Option<&Os2> {
        self.os2.as_ref()
    }

    /// The `post` table's header, or `None` if the face has none or one
    /// this crate cannot read.
    pub const fn post(&self) -> Option<&Post> {
        self.post.as_ref()
    }

    /// `head.unitsPerEm`: font units per em, in 16..=16384.
    #[must_use]
    pub const fn units_per_em(&self) -> u16 {
        self.head.units_per_em
    }

    /// `hhea`'s `ascender`, `descender` and `lineGap`.
    #[must_use]
    pub const fn hhea_metrics(&self) -> VerticalMetrics {
        VerticalMetrics {
            ascender: self.hhea.ascender,
            descender: self.hhea.descender,
            line_gap: self.hhea.line_gap,
        }
    }

    /// `OS/2`'s `sTypoAscender`, `sTypoDescender` and `sTypoLineGap`, or
    /// `None` if the font has no `OS/2` table or its table stops before
    /// them.
    #[must_use]
    pub fn typo_metrics(&self) -> Option<VerticalMetrics> {
        let line_metrics = self.os2.as_ref()?.line_metrics?;
        Some(VerticalMetrics {
            ascender: line_metrics.s_typo_ascender,
            descender: line_metrics.s_typo_descender,
            line_gap: line_metrics.s_typo_line_gap,
        })
    }

    /// `post`'s `underlinePosition` and `underlineThickness`, or `None`
    /// if the font has no readable `post` table.
    #[must_use]
    pub fn underline(&self) -> Option<DecorationLine> {
        let post = self.post.as_ref()?;
        Some(DecorationLine {
            position: post.underline_position,
            thickness: post.underline_thickness,
        })
    }

    /// `OS/2`'s `yStrikeoutPosition` and `yStrikeoutSize`, or `None` if
    /// the font has no `OS/2` table.
    #[must_use]
    pub fn strikeout(&self) -> Option<DecorationLine> {
        let os2 = self.os2.as_ref()?;
        Some(DecorationLine {
            position: os2.y_strikeout_position,
            thickness: os2.y_strikeout_size,
        })
    }

    /// Pixels per font unit at a font size of `px` pixels per em.
    #[must_use]
    pub fn scale_factor(&self, px: f32) -> f32 {
        px / f32::from(self.head.units_per_em)
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
