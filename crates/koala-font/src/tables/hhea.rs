//! [`hhea` — Horizontal header table](https://learn.microsoft.com/en-us/typography/opentype/spec/hhea).
//!
//! 36 bytes, all fixed fields. Koala uses `ascender`, `descender` and
//! `lineGap`, the vertical metrics of last resort when a font has no
//! `OS/2` table.

use crate::directory::TableDirectory;
use crate::error::FontError;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::tables::read_table_start;
use crate::types::fword::FWord;
use crate::types::tag::Tag;
use crate::types::ufword::UFWord;

const TAG: Tag = Tag::from_literal(*b"hhea");

/// The table's length: every field is fixed.
const LEN: usize = 36;

/// The `hhea` table, every field but the four reserved `int16`s, which
/// are "set to 0".
///
/// A face only opens with an `hhea` whose `majorVersion` is 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhea {
    /// `majorVersion`: "Major version number of the horizontal header
    /// table — set to 1."
    pub major_version: u16,
    /// `minorVersion`: "Minor version number of the horizontal header
    /// table — set to 0."
    pub minor_version: u16,
    /// `ascender`: "Typographic ascent".
    pub ascender: FWord,
    /// `descender`: "Typographic descent".
    pub descender: FWord,
    /// `lineGap`: "Typographic line gap. Negative lineGap values are
    /// treated as zero in some legacy platform implementations."
    pub line_gap: FWord,
    /// `advanceWidthMax`: "Maximum advance width value in 'hmtx' table."
    pub advance_width_max: UFWord,
    /// `minLeftSideBearing`: "Minimum left sidebearing value in 'hmtx'
    /// table for glyphs with contours (empty glyphs should be ignored)."
    pub min_left_side_bearing: FWord,
    /// `minRightSideBearing`: "Minimum right sidebearing value;
    /// calculated as min(aw - (lsb + xMax - xMin)) for glyphs with
    /// contours (empty glyphs should be ignored)."
    pub min_right_side_bearing: FWord,
    /// `xMaxExtent`: "Max(lsb + (xMax - xMin))."
    pub x_max_extent: FWord,
    /// `caretSlopeRise`: "Used to calculate the slope of the cursor
    /// (rise/run); 1 for vertical."
    pub caret_slope_rise: i16,
    /// `caretSlopeRun`: "0 for vertical."
    pub caret_slope_run: i16,
    /// `caretOffset`: "The amount by which a slanted highlight on a glyph
    /// needs to be shifted to produce the best appearance. Set to 0 for
    /// non-slanted fonts"
    pub caret_offset: i16,
    /// `metricDataFormat`: "0 for current format."
    pub metric_data_format: i16,
    /// `numberOfHMetrics`: "Number of hMetric entries in 'hmtx' table"
    pub number_of_h_metrics: u16,
}

/// Reads the face's `hhea` table. Every face needs one.
///
/// # Errors
///
/// [`FontError::MissingTable`] if the face has no `hhea` table or one
/// with a major version other than 1; [`FontError::TableTooShort`] if it
/// is shorter than its 36 bytes; [`FontError::Source`] if the source
/// fails or ends early.
pub fn read<S: FontSource + ?Sized>(
    source: &S,
    directory: &TableDirectory,
) -> Result<Hhea, FontError> {
    let range = directory.find(TAG).ok_or(FontError::MissingTable(TAG))?;
    let bytes = read_table_start::<LEN, _>(source, range)?.ok_or(FontError::TableTooShort {
        tag: TAG,
        length: range.length,
    })?;
    parse(&bytes).ok_or(FontError::MissingTable(TAG))
}

/// Decodes an `hhea` table, or `None` for a major version this crate
/// cannot read.
fn parse(bytes: &[u8; LEN]) -> Option<Hhea> {
    let hhea = parse_fields(bytes).expect("`bytes` is sized for the fields");

    // "If the major version is not recognized, the implementation must
    // not read the table as it can make no assumptions regarding
    // interpretation of the binary data. The implementation should treat
    // the table as missing." (OpenType font file, Table Version Numbers)
    (hhea.major_version == 1).then_some(hhea)
}

/// Every field, in the order the spec lists them.
fn parse_fields(bytes: &[u8]) -> Option<Hhea> {
    let mut reader = Reader::new(bytes);
    Some(Hhea {
        major_version: reader.read()?,
        minor_version: reader.read()?,
        ascender: reader.read()?,
        descender: reader.read()?,
        line_gap: reader.read()?,
        advance_width_max: reader.read()?,
        min_left_side_bearing: reader.read()?,
        min_right_side_bearing: reader.read()?,
        x_max_extent: reader.read()?,
        caret_slope_rise: reader.read()?,
        caret_slope_run: reader.read()?,
        caret_offset: reader.read()?,
        metric_data_format: {
            // Four reserved int16s, "set to 0", come first.
            reader.skip(8)?;
            reader.read()?
        },
        number_of_h_metrics: reader.read()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{LEN, parse};
    use crate::types::fword::FWord;
    use crate::types::ufword::UFWord;

    /// Helvetica.ttc face 0's `hhea`, as `xxd -s 0x924 -l 36` dumps it.
    #[rustfmt::skip]
    const HELVETICA: [u8; LEN] = [
        0x00, 0x01, 0x00, 0x00, // majorVersion 1, minorVersion 0
        0x06, 0x29,             // ascender 1577
        0xFE, 0x29,             // descender -471
        0x00, 0x00,             // lineGap 0
        0x0C, 0x01,             // advanceWidthMax 3073
        0xF8, 0x65,             // minLeftSideBearing -1947
        0xFC, 0xED,             // minRightSideBearing -787
        0x0B, 0x91,             // xMaxExtent 2961
        0x00, 0x01, 0x00, 0x00, // caretSlopeRise 1, caretSlopeRun 0
        0x00, 0x00,             // caretOffset
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
        0x00, 0x00,             // metricDataFormat
        0x08, 0xCC,             // numberOfHMetrics 2252
    ];

    /// Every field lands where the spec puts it; `numberOfHMetrics`
    /// sits after the reserved fields, so it checks the skip too.
    #[test]
    fn reads_every_field_in_order() {
        let hhea = parse(&HELVETICA).expect("version 1");
        assert_eq!(
            (hhea.ascender, hhea.descender, hhea.line_gap),
            (FWord::from(1577), FWord::from(-471), FWord::from(0))
        );
        assert_eq!(hhea.advance_width_max, UFWord::from(3073));
        assert_eq!(hhea.min_left_side_bearing, FWord::from(-1947));
        assert_eq!(hhea.min_right_side_bearing, FWord::from(-787));
        assert_eq!(hhea.x_max_extent, FWord::from(2961));
        assert_eq!((hhea.caret_slope_rise, hhea.caret_slope_run), (1, 0));
        assert_eq!(hhea.number_of_h_metrics, 2252);
    }

    #[test]
    fn an_unknown_major_version_is_a_missing_table() {
        let mut bytes = HELVETICA;
        bytes
            .first_chunk_mut::<2>()
            .expect("the table has a version")
            .copy_from_slice(&2_u16.to_be_bytes());
        assert_eq!(parse(&bytes), None);
    }
}
