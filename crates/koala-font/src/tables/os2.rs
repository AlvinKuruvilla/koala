//! [`OS/2` — OS/2 and Windows metrics table](https://learn.microsoft.com/en-us/typography/opentype/spec/os2).
//!
//! Koala uses the typographic ascender, descender and line gap, which
//! CSS prefers to `hhea`'s, and the strikeout line.
//!
//! "Six versions of the OS/2 table have been defined: versions 0 to 5."
//! Each keeps every field of the one before at the same offset and
//! appends more:
//!
//! ```text
//!    0 ┌───────────────────────────────┐
//!      │ version .. usLastCharIndex    │  Os2's own fields; every version
//!   68 ├───────────────────────────────┤
//!      │ sTypoAscender .. usWinDescent │  line_metrics; Apple's version 0
//!   78 ├───────────────────────────────┤    may stop before these
//!      │ ulCodePageRange1, 2           │  version_1
//!   86 ├───────────────────────────────┤
//!      │ sxHeight .. usMaxContext      │  version_2 (versions 2 to 4 add
//!   96 ├───────────────────────────────┤    no fields after version 2)
//!      │ usLower/UpperOpticalPointSize │  version_5
//!  100 └───────────────────────────────┘
//! ```
//!
//! A group is read when the version says it exists and the table is long
//! enough to hold it. The length check is the spec's: Apple's version 0
//! "stops at the usLastCharIndex field and does not include the last
//! five fields", and "Applications should check the table length for a
//! version 0 OS/2 table before reading these fields." It is applied to
//! every group, since it covers a truncated table of any version too. A
//! version above 5 is read as a version 5: `ttf-parser` drops such a
//! table instead, but every version so far has only appended fields.
//!
//! The table is optional: without it the caller falls back to `hhea`'s
//! metrics and places the strikeout itself.

use crate::directory::TableDirectory;
use crate::error::FontError;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::tables::read_table_up_to;
use crate::types::fword::FWord;
use crate::types::tag::Tag;
use crate::types::ufword::UFWord;

const TAG: Tag = Tag::from_literal(*b"OS/2");

/// A version 5 table's length, the longest defined.
const MAX_LEN: usize = 100;

/// The `OS/2` table, every field, grouped by the version that added it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Os2 {
    /// `version`: "The version number for the OS/2 table: 0x0000 to
    /// 0x0005."
    pub version: u16,
    /// `xAvgCharWidth`: "the arithmetic average of the escapement (width)
    /// of all non-zero width glyphs in the font."
    pub x_avg_char_width: FWord,
    /// `usWeightClass`: "Indicates the visual weight (degree of blackness
    /// or thickness of strokes) of the characters in the font. Values
    /// from 1 to 1000 are valid."
    pub us_weight_class: u16,
    /// `usWidthClass`: "Indicates a relative change from the normal
    /// aspect ratio (width to height ratio) as specified by a font
    /// designer for the glyphs in a font."
    pub us_width_class: u16,
    /// `fsType`: "Indicates font embedding licensing rights for the
    /// font."
    pub fs_type: u16,
    /// `ySubscriptXSize`: "The recommended horizontal size in font design
    /// units for subscripts for this font."
    pub y_subscript_x_size: FWord,
    /// `ySubscriptYSize`: "The recommended vertical size in font design
    /// units for subscripts for this font."
    pub y_subscript_y_size: FWord,
    /// `ySubscriptXOffset`: "The recommended horizontal offset in font
    /// design units for subscripts for this font."
    pub y_subscript_x_offset: FWord,
    /// `ySubscriptYOffset`: "The recommended vertical offset in font
    /// design units from the baseline for subscripts for this font."
    pub y_subscript_y_offset: FWord,
    /// `ySuperscriptXSize`: "The recommended horizontal size in font
    /// design units for superscripts for this font."
    pub y_superscript_x_size: FWord,
    /// `ySuperscriptYSize`: "The recommended vertical size in font design
    /// units for superscripts for this font."
    pub y_superscript_y_size: FWord,
    /// `ySuperscriptXOffset`: "The recommended horizontal offset in font
    /// design units for superscripts for this font."
    pub y_superscript_x_offset: FWord,
    /// `ySuperscriptYOffset`: "The recommended vertical offset in font
    /// design units from the baseline for superscripts for this font."
    pub y_superscript_y_offset: FWord,
    /// `yStrikeoutSize`: "Thickness of the strikeout stroke in font
    /// design units."
    pub y_strikeout_size: FWord,
    /// `yStrikeoutPosition`: "The position of the top of the strikeout
    /// stroke relative to the baseline in font design units."
    pub y_strikeout_position: FWord,
    /// `sFamilyClass`: "This field provides a classification of
    /// font-family design."
    pub s_family_class: i16,
    /// `panose`: "This 10-byte array of numbers is used to describe the
    /// visual characteristics of a given typeface."
    pub panose: [u8; 10],
    /// `ulUnicodeRange1`, bits 0 to 31 of the Unicode blocks "encompassed
    /// by the font file".
    pub ul_unicode_range1: u32,
    /// `ulUnicodeRange2`, bits 32 to 63.
    pub ul_unicode_range2: u32,
    /// `ulUnicodeRange3`, bits 64 to 95.
    pub ul_unicode_range3: u32,
    /// `ulUnicodeRange4`, bits 96 to 127.
    pub ul_unicode_range4: u32,
    /// `achVendID`: "The four character identifier for the vendor of the
    /// given typeface." The spec types it `Tag`, but it stays bytes here:
    /// fonts fill it with NULs or other bytes a [`Tag`] refuses, and a
    /// bad vendor ID is no reason to drop the table.
    pub ach_vend_id: [u8; 4],
    /// `fsSelection`: "Contains information concerning the nature of the
    /// font patterns". Bit 7, `USE_TYPO_METRICS`, says whether the line
    /// metrics below should be used for line spacing.
    pub fs_selection: u16,
    /// `usFirstCharIndex`: "The minimum Unicode index (character code) in
    /// this font".
    pub us_first_char_index: u16,
    /// `usLastCharIndex`: "The maximum Unicode index (character code) in
    /// this font".
    pub us_last_char_index: u16,
    /// The five fields after `usLastCharIndex`, or `None` if the table
    /// stops before them.
    pub line_metrics: Option<Os2LineMetrics>,
    /// The fields version 1 adds, or `None` for version 0 or a table that
    /// stops before them.
    pub version_1: Option<Os2Version1>,
    /// The fields version 2 adds, or `None` for an earlier version or a
    /// table that stops before them.
    pub version_2: Option<Os2Version2>,
    /// The fields version 5 adds, or `None` for an earlier version or a
    /// table that stops before them.
    pub version_5: Option<Os2Version5>,
}

/// The last five fields of a version 0 `OS/2` table, which Apple's
/// version 0 may leave out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Os2LineMetrics {
    /// `sTypoAscender`: "The typographic ascender for this font. This
    /// field should be combined with the sTypoDescender and sTypoLineGap
    /// values to determine default line spacing."
    pub s_typo_ascender: FWord,
    /// `sTypoDescender`: "The typographic descender for this font."
    pub s_typo_descender: FWord,
    /// `sTypoLineGap`: "The typographic line gap for this font."
    pub s_typo_line_gap: FWord,
    /// `usWinAscent`: "The “Windows ascender” metric. This should be used
    /// to specify the height above the baseline for a clipping region."
    pub us_win_ascent: UFWord,
    /// `usWinDescent`: "The “Windows descender” metric. This should be
    /// used to specify the vertical extent below the baseline for a
    /// clipping region."
    pub us_win_descent: UFWord,
}

/// The fields `OS/2` version 1 adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Os2Version1 {
    /// `ulCodePageRange1`, bits 0 to 31 of "the code pages encompassed by
    /// the font file".
    pub ul_code_page_range1: u32,
    /// `ulCodePageRange2`, bits 32 to 63.
    pub ul_code_page_range2: u32,
}

/// The fields `OS/2` version 2 adds; versions 3 and 4 add none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Os2Version2 {
    /// `sxHeight`: "the distance between the baseline and the approximate
    /// height of non-ascending lowercase letters measured in font design
    /// units."
    pub sx_height: FWord,
    /// `sCapHeight`: "the distance between the baseline and the
    /// approximate height of uppercase letters measured in font design
    /// units."
    pub s_cap_height: FWord,
    /// `usDefaultChar`: "the Unicode code point, in UTF-16 encoding, of a
    /// character that can be used for a default glyph if a requested
    /// character is not supported in the font."
    pub us_default_char: u16,
    /// `usBreakChar`: "the Unicode code point, in UTF-16 encoding, of a
    /// character that can be used as a default break character."
    pub us_break_char: u16,
    /// `usMaxContext`: "The maximum length of a target glyph context for
    /// any feature in this font."
    pub us_max_context: u16,
}

/// The fields `OS/2` version 5 adds: "This field is used for fonts with
/// multiple optical styles." Both are in TWIPs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Os2Version5 {
    /// `usLowerOpticalPointSize`.
    pub us_lower_optical_point_size: u16,
    /// `usUpperOpticalPointSize`.
    pub us_upper_optical_point_size: u16,
}

/// Reads the face's `OS/2` table, or `None` if it has none or one too
/// short for the fields every version has.
///
/// # Errors
///
/// [`FontError::Source`] if the source fails or ends before the bytes
/// the table directory says are there.
pub fn read<S: FontSource + ?Sized>(
    source: &S,
    directory: &TableDirectory,
) -> Result<Option<Os2>, FontError> {
    let Some(range) = directory.find(TAG) else {
        return Ok(None);
    };
    Ok(parse(&read_table_up_to(source, range, MAX_LEN)?))
}

/// Decodes the start of an `OS/2` table: the fields every version has,
/// and each later group the version declares and `bytes` reaches.
fn parse(bytes: &[u8]) -> Option<Os2> {
    let mut reader = Reader::new(bytes);
    let mut os2 = Os2 {
        version: reader.read()?,
        x_avg_char_width: reader.read()?,
        us_weight_class: reader.read()?,
        us_width_class: reader.read()?,
        fs_type: reader.read()?,
        y_subscript_x_size: reader.read()?,
        y_subscript_y_size: reader.read()?,
        y_subscript_x_offset: reader.read()?,
        y_subscript_y_offset: reader.read()?,
        y_superscript_x_size: reader.read()?,
        y_superscript_y_size: reader.read()?,
        y_superscript_x_offset: reader.read()?,
        y_superscript_y_offset: reader.read()?,
        y_strikeout_size: reader.read()?,
        y_strikeout_position: reader.read()?,
        s_family_class: reader.read()?,
        panose: reader.read_array()?,
        ul_unicode_range1: reader.read()?,
        ul_unicode_range2: reader.read()?,
        ul_unicode_range3: reader.read()?,
        ul_unicode_range4: reader.read()?,
        ach_vend_id: reader.read_array()?,
        fs_selection: reader.read()?,
        us_first_char_index: reader.read()?,
        us_last_char_index: reader.read()?,
        line_metrics: None,
        version_1: None,
        version_2: None,
        version_5: None,
    };

    // Each group is read only after the one before it was: `bytes` is a
    // prefix of the table, so a group it does not reach means no later
    // group is reached either, and the reader stops wherever that group
    // ran out, not at the next group's start.
    os2.line_metrics = Os2LineMetrics::parse(&mut reader);
    if os2.line_metrics.is_some() && os2.version >= 1 {
        os2.version_1 = Os2Version1::parse(&mut reader);
    }
    if os2.version_1.is_some() && os2.version >= 2 {
        os2.version_2 = Os2Version2::parse(&mut reader);
    }
    if os2.version_2.is_some() && os2.version >= 5 {
        os2.version_5 = Os2Version5::parse(&mut reader);
    }
    Some(os2)
}

impl Os2LineMetrics {
    fn parse(reader: &mut Reader<'_>) -> Option<Self> {
        Some(Self {
            s_typo_ascender: reader.read()?,
            s_typo_descender: reader.read()?,
            s_typo_line_gap: reader.read()?,
            us_win_ascent: reader.read()?,
            us_win_descent: reader.read()?,
        })
    }
}

impl Os2Version1 {
    fn parse(reader: &mut Reader<'_>) -> Option<Self> {
        Some(Self {
            ul_code_page_range1: reader.read()?,
            ul_code_page_range2: reader.read()?,
        })
    }
}

impl Os2Version2 {
    fn parse(reader: &mut Reader<'_>) -> Option<Self> {
        Some(Self {
            sx_height: reader.read()?,
            s_cap_height: reader.read()?,
            us_default_char: reader.read()?,
            us_break_char: reader.read()?,
            us_max_context: reader.read()?,
        })
    }
}

impl Os2Version5 {
    fn parse(reader: &mut Reader<'_>) -> Option<Self> {
        Some(Self {
            us_lower_optical_point_size: reader.read()?,
            us_upper_optical_point_size: reader.read()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::types::fword::FWord;
    use crate::types::ufword::UFWord;

    /// Helvetica.ttc face 0's `OS/2`, a 96-byte version 3 table, as
    /// `xxd -s 0xcc0 -l 96` dumps it.
    #[rustfmt::skip]
    const HELVETICA: [u8; 96] = [
        0x00, 0x03,             // version 3
        0x03, 0x88,             // xAvgCharWidth 904
        0x01, 0x90, 0x00, 0x05, // usWeightClass 400, usWidthClass 5
        0x00, 0x00,             // fsType
        0x05, 0x99, 0x05, 0x33, 0x00, 0x00, 0x01, 0x1E, // subscript sizes, offsets
        0x05, 0x99, 0x05, 0x33, 0x00, 0x00, 0x03, 0xD0, // superscript sizes, offsets
        0x00, 0x66,             // yStrikeoutSize 102
        0x02, 0x00,             // yStrikeoutPosition 512
        0x00, 0x00,             // sFamilyClass
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // panose
        0xE0, 0x00, 0x02, 0xFF, 0x50, 0x00, 0x78, 0x5B, // ulUnicodeRange1, 2
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // ulUnicodeRange3, 4
        0x70, 0x79, 0x72, 0x73, // achVendID "pyrs"
        0x00, 0x40,             // fsSelection
        0x00, 0x09, 0xFB, 0x02, // usFirstCharIndex, usLastCharIndex
        0x06, 0x00,             // sTypoAscender 1536
        0xFE, 0xA4,             // sTypoDescender -348
        0x00, 0x3D,             // sTypoLineGap 61
        0x07, 0x9A, 0x01, 0xCD, // usWinAscent 1946, usWinDescent 461
        0x20, 0x00, 0x01, 0x9F, 0x4F, 0x01, 0x00, 0x00, // ulCodePageRange1, 2
        0x04, 0x2F, 0x05, 0xBD, // sxHeight 1071, sCapHeight 1469
        0x00, 0x00, 0x00, 0x20, 0x00, 0x00, // usDefaultChar, usBreakChar, usMaxContext
    ];

    /// `HELVETICA` cut to `len` bytes, with its version replaced.
    fn helvetica(version: u16, len: usize) -> Vec<u8> {
        let mut bytes = HELVETICA.to_vec();
        bytes
            .first_chunk_mut::<2>()
            .expect("the table has a version")
            .copy_from_slice(&version.to_be_bytes());
        bytes.truncate(len);
        bytes
    }

    /// Every group lands where the spec puts it.
    #[test]
    fn reads_every_field_in_order() {
        let os2 = parse(&HELVETICA).expect("a version 3 table");
        assert_eq!(os2.us_weight_class, 400);
        assert_eq!(os2.y_superscript_y_offset, FWord::from(976));
        assert_eq!(os2.y_strikeout_size, FWord::from(102));
        assert_eq!(os2.y_strikeout_position, FWord::from(512));
        assert_eq!(os2.ul_unicode_range2, 0x5000_785B);
        assert_eq!(&os2.ach_vend_id, b"pyrs");
        assert_eq!(os2.fs_selection, 0x0040);
        assert_eq!(os2.us_last_char_index, 0xFB02);

        let line_metrics = os2.line_metrics.expect("version 3 has them");
        assert_eq!(line_metrics.s_typo_ascender, FWord::from(1536));
        assert_eq!(line_metrics.s_typo_descender, FWord::from(-348));
        assert_eq!(line_metrics.s_typo_line_gap, FWord::from(61));
        assert_eq!(line_metrics.us_win_ascent, UFWord::from(1946));
        assert_eq!(line_metrics.us_win_descent, UFWord::from(461));

        assert_eq!(
            os2.version_1
                .expect("version 3 has them")
                .ul_code_page_range1,
            0x2000_019F
        );
        let version_2 = os2.version_2.expect("version 3 has them");
        assert_eq!(version_2.s_cap_height, FWord::from(1469));
        assert_eq!(version_2.us_break_char, 0x0020);
        assert_eq!(os2.version_5, None);
    }

    /// Apple's version 0 ends at `usLastCharIndex`, after the strikeout
    /// and before the line metrics.
    #[test]
    fn a_short_version_0_has_no_line_metrics() {
        let os2 = parse(&helvetica(0, 68)).expect("the fields every version has");
        assert_eq!(os2.y_strikeout_position, FWord::from(512));
        assert_eq!(os2.line_metrics, None);
    }

    /// Bytes past what the version defines are not read as fields.
    #[test]
    fn a_version_declares_its_groups() {
        let os2 = parse(&helvetica(0, 96)).expect("a version 0 table");
        assert!(os2.line_metrics.is_some());
        assert_eq!(os2.version_1, None);

        let os2 = parse(&helvetica(1, 96)).expect("a version 1 table");
        assert!(os2.version_1.is_some());
        assert_eq!(os2.version_2, None);
    }

    /// A group needs every one of its bytes, and a table that stops
    /// inside one group has none of the later ones.
    #[test]
    fn a_group_needs_every_byte() {
        assert!(
            parse(&helvetica(3, 78))
                .expect("valid")
                .line_metrics
                .is_some()
        );
        assert_eq!(parse(&helvetica(3, 77)).expect("valid").line_metrics, None);
        assert_eq!(parse(&helvetica(3, 77)).expect("valid").version_1, None);
        assert!(parse(&helvetica(3, 86)).expect("valid").version_1.is_some());
        assert_eq!(parse(&helvetica(3, 95)).expect("valid").version_2, None);
        assert_eq!(parse(&helvetica(3, 67)), None);
    }

    /// A version above 5 is read as version 5: its first 100 bytes are
    /// the fields version 5 defines.
    #[test]
    fn a_later_version_is_read_as_version_5() {
        let mut bytes = helvetica(6, 96);
        bytes.extend([0x00, 0x0A, 0x00, 0x48]);
        let version_5 = parse(&bytes)
            .expect("valid")
            .version_5
            .expect("read as version 5");
        assert_eq!(version_5.us_lower_optical_point_size, 10);
        assert_eq!(version_5.us_upper_optical_point_size, 72);
    }
}
