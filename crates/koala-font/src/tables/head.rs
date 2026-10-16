//! [`head` — Font header table](https://learn.microsoft.com/en-us/typography/opentype/spec/head).
//!
//! "This table gives global information about the font." It is 54 bytes,
//! all fixed fields. Koala uses `unitsPerEm`, and `magicNumber` as a
//! check that the table directory pointed at a real `head`.

use crate::directory::TableDirectory;
use crate::error::FontError;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::tables::read_table_start;
use crate::types::fixed::Fixed;
use crate::types::tag::Tag;

const TAG: Tag = Tag::from_literal(*b"head");

/// The table's length: every field is fixed.
const LEN: usize = 54;

/// `magicNumber`: "Set to 0x5F0F3CF5."
const MAGIC_NUMBER: u32 = 0x5F0F_3CF5;

/// The `head` table, every field.
///
/// A face only opens with a `head` whose `majorVersion` is 1,
/// `magicNumber` is 0x5F0F3CF5, and `unitsPerEm` is in 16..=16384.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    /// `majorVersion`: "Major version number of the font header table —
    /// set to 1."
    pub major_version: u16,
    /// `minorVersion`: "Minor version number of the font header table —
    /// set to 0."
    pub minor_version: u16,
    /// `fontRevision`: "Set by font manufacturer."
    pub font_revision: Fixed,
    /// `checksumAdjustment`: "If the font is used as a component in a
    /// font collection file, the value of this field will be invalidated
    /// by changes to the file structure and font table directory, and
    /// must be ignored."
    pub checksum_adjustment: u32,
    /// `magicNumber`: "Set to 0x5F0F3CF5."
    pub magic_number: u32,
    /// `flags`: bit 0 "Baseline for font at y=0", and others the spec
    /// lists.
    pub flags: u16,
    /// `unitsPerEm`: "Set to a value from 16 to 16384."
    pub units_per_em: u16,
    /// `created` (`LONGDATETIME`): "Number of seconds since 12:00
    /// midnight that started January 1st, 1904, in GMT/UTC time zone."
    pub created: i64,
    /// `modified` (`LONGDATETIME`), counted as `created` is.
    pub modified: i64,
    /// `xMin`: "Minimum x coordinate across all glyph bounding boxes."
    pub x_min: i16,
    /// `yMin`: "Minimum y coordinate across all glyph bounding boxes."
    pub y_min: i16,
    /// `xMax`: "Maximum x coordinate across all glyph bounding boxes."
    pub x_max: i16,
    /// `yMax`: "Maximum y coordinate across all glyph bounding boxes."
    pub y_max: i16,
    /// `macStyle`: "Bit 0: Bold (if set to 1); Bit 1: Italic (if set to
    /// 1)", and others the spec lists.
    pub mac_style: u16,
    /// `lowestRecPPEM`: "Smallest readable size in pixels."
    pub lowest_rec_ppem: u16,
    /// `fontDirectionHint`: "Deprecated (Set to 2)."
    pub font_direction_hint: i16,
    /// `indexToLocFormat`: "0 for short offsets (Offset16), 1 for long
    /// (Offset32)."
    pub index_to_loc_format: i16,
    /// `glyphDataFormat`: "0 for current format."
    pub glyph_data_format: i16,
}

/// Reads the face's `head` table. Every face needs one: without
/// `unitsPerEm`, no value in font units can be scaled.
///
/// # Errors
///
/// [`FontError::MissingTable`] if the face has no `head` table or one
/// with a major version other than 1; [`FontError::TableTooShort`] if it
/// is shorter than its 54 bytes; [`FontError::Source`] if the source
/// fails or ends early; and [`parse`]'s errors.
pub fn read<S: FontSource + ?Sized>(
    source: &S,
    directory: &TableDirectory,
) -> Result<Head, FontError> {
    let range = directory.find(TAG).ok_or(FontError::MissingTable(TAG))?;
    let bytes = read_table_start::<LEN, _>(source, range)?.ok_or(FontError::TableTooShort {
        tag: TAG,
        length: range.length,
    })?;
    parse(&bytes)?.ok_or(FontError::MissingTable(TAG))
}

/// Decodes a `head` table, or `None` for a major version this crate
/// cannot read.
///
/// # Errors
///
/// [`FontError::HeadMagicNumberMismatch`] if `magicNumber` is wrong;
/// [`FontError::UnitsPerEmOutOfRange`] if `unitsPerEm` is outside
/// 16..=16384.
fn parse(bytes: &[u8; LEN]) -> Result<Option<Head>, FontError> {
    let head = parse_fields(bytes).expect("`bytes` is sized for the fields");

    // "If the major version is not recognized, the implementation must
    // not read the table as it can make no assumptions regarding
    // interpretation of the binary data. The implementation should treat
    // the table as missing." (OpenType font file, Table Version Numbers)
    if head.major_version != 1 {
        return Ok(None);
    }

    // A record whose offset is wrong still points at 54 readable bytes
    // more often than not; the magic number is what tells them apart.
    if head.magic_number != MAGIC_NUMBER {
        return Err(FontError::HeadMagicNumberMismatch(head.magic_number));
    }

    // "Set to a value from 16 to 16384. Any value in this range is
    // valid." Outside it the face is refused rather than read: every
    // distance in font units is divided by `unitsPerEm`, and a 0 would
    // turn each into infinity or NaN.
    if !(16..=16384).contains(&head.units_per_em) {
        return Err(FontError::UnitsPerEmOutOfRange(head.units_per_em));
    }

    Ok(Some(head))
}

/// Every field, in the order the spec lists them.
fn parse_fields(bytes: &[u8]) -> Option<Head> {
    let mut reader = Reader::new(bytes);
    Some(Head {
        major_version: reader.read()?,
        minor_version: reader.read()?,
        font_revision: reader.read()?,
        checksum_adjustment: reader.read()?,
        magic_number: reader.read()?,
        flags: reader.read()?,
        units_per_em: reader.read()?,
        created: reader.read()?,
        modified: reader.read()?,
        x_min: reader.read()?,
        y_min: reader.read()?,
        x_max: reader.read()?,
        y_max: reader.read()?,
        mac_style: reader.read()?,
        lowest_rec_ppem: reader.read()?,
        font_direction_hint: reader.read()?,
        index_to_loc_format: reader.read()?,
        glyph_data_format: reader.read()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{LEN, parse};
    use crate::error::FontError;

    /// Helvetica.ttc face 0's `head`, as `xxd -s 0xa4c -l 54` dumps it.
    #[rustfmt::skip]
    const HELVETICA: [u8; LEN] = [
        0x00, 0x01, 0x00, 0x00, // majorVersion 1, minorVersion 0
        0x00, 0x00, 0x00, 0x00, // fontRevision
        0x61, 0x78, 0xCD, 0x6A, // checksumAdjustment
        0x5F, 0x0F, 0x3C, 0xF5, // magicNumber
        0x00, 0x11,             // flags
        0x08, 0x00,             // unitsPerEm 2048
        0x00, 0x00, 0x00, 0x00, 0x5F, 0x4D, 0x8F, 0x00, // created
        0x00, 0x00, 0x00, 0x00, 0xDE, 0xE0, 0x2E, 0xD6, // modified
        0xF8, 0x65, 0xFC, 0x27, // xMin -1947, yMin -985
        0x0B, 0x91, 0x08, 0xF9, // xMax 2961, yMax 2297
        0x00, 0x00,             // macStyle
        0x00, 0x09,             // lowestRecPPEM 9
        0x00, 0x01,             // fontDirectionHint 1
        0x00, 0x01,             // indexToLocFormat 1
        0x00, 0x00,             // glyphDataFormat 0
    ];

    /// Every field lands where the spec puts it.
    #[test]
    fn reads_every_field_in_order() {
        let head = parse(&HELVETICA).expect("valid").expect("version 1");
        assert_eq!(head.checksum_adjustment, 0x6178_CD6A);
        assert_eq!(head.flags, 0x0011);
        assert_eq!(head.created, 0x5F4D_8F00);
        assert_eq!(head.modified, 0xDEE0_2ED6);
        assert_eq!(
            (head.x_min, head.y_min, head.x_max, head.y_max),
            (-1947, -985, 2961, 2297)
        );
        assert_eq!(head.lowest_rec_ppem, 9);
        assert_eq!(head.font_direction_hint, 1);
        assert_eq!(head.index_to_loc_format, 1);
        assert_eq!(head.glyph_data_format, 0);
    }

    /// `HELVETICA` with the bytes at `offset` replaced.
    fn with(offset: usize, replacement: &[u8]) -> [u8; LEN] {
        let mut bytes = HELVETICA;
        bytes
            .get_mut(offset..offset + replacement.len())
            .expect("the replacement lies inside the table")
            .copy_from_slice(replacement);
        bytes
    }

    #[test]
    fn reads_units_per_em() {
        let head = parse(&HELVETICA).expect("valid").expect("version 1");
        assert_eq!(head.units_per_em, 2048);
    }

    #[test]
    fn an_unknown_major_version_is_a_missing_table() {
        assert_eq!(parse(&with(0, &[0x00, 0x02])).expect("not an error"), None);
    }

    #[test]
    fn refuses_a_wrong_magic_number() {
        let error = parse(&with(12, &[0x12, 0x34, 0x56, 0x78])).expect_err("wrong magic");
        assert!(matches!(
            error,
            FontError::HeadMagicNumberMismatch(0x1234_5678)
        ));
    }

    #[test]
    fn units_per_em_must_be_16_to_16384() {
        for valid in [16_u16, 1000, 16384] {
            assert!(parse(&with(18, &valid.to_be_bytes())).is_ok(), "{valid}");
        }
        for invalid in [0_u16, 15, 16385, u16::MAX] {
            let error = parse(&with(18, &invalid.to_be_bytes())).expect_err("out of range");
            assert!(matches!(error, FontError::UnitsPerEmOutOfRange(value) if value == invalid));
        }
    }
}
