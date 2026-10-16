//! [`post` — PostScript Table](https://learn.microsoft.com/en-us/typography/opentype/spec/post).
//!
//! A 32-byte header, then glyph names whose layout depends on the
//! version. Only the header is read; Koala uses `underlinePosition` and
//! `underlineThickness`.
//!
//! Every version starts with the same header, so the version only decides
//! whether the table is read at all. The OpenType spec defines 1.0, 2.0,
//! 2.5 and 3.0. Apple's [TrueType Reference Manual](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6post.html)
//! adds a fourth major version with the same header: "There are five
//! possibilities: format 1, format 2, format 2.5, format 3 and format 4",
//! and in format 4 "the 'post' table header is followed by an array of
//! uint16 values". So major versions 1 to 4 are read.
//!
//! The table is optional: a face without one, or with one this crate
//! cannot read, has no underline metrics, and the caller places the
//! underline itself.

use crate::directory::TableDirectory;
use crate::error::FontError;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::tables::read_table_start;
use crate::types::fixed::Fixed;
use crate::types::fword::FWord;
use crate::types::tag::Tag;
use crate::types::version16dot16::Version16Dot16;

const TAG: Tag = Tag::from_literal(*b"post");

/// The header's length.
const LEN: usize = 32;

/// The `post` table's header, every field. The glyph names after it are
/// not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    /// `version`: 1.0, 2.0, 2.5 or 3.0, or Apple's 4.0.
    pub version: Version16Dot16,
    /// `italicAngle`: "Italic angle in counter-clockwise degrees from the
    /// vertical. Zero for upright text, negative for text that leans to
    /// the right (forward)."
    pub italic_angle: Fixed,
    /// `underlinePosition`: "Suggested y-coordinate of the top of the
    /// underline."
    pub underline_position: FWord,
    /// `underlineThickness`: "Suggested values for the underline
    /// thickness."
    pub underline_thickness: FWord,
    /// `isFixedPitch`: "Set to 0 if the font is proportionally spaced,
    /// non-zero if the font is not proportionally spaced (i.e.
    /// monospaced)."
    pub is_fixed_pitch: u32,
    /// `minMemType42`: "Minimum memory usage when an OpenType font is
    /// downloaded."
    pub min_mem_type42: u32,
    /// `maxMemType42`: "Maximum memory usage when an OpenType font is
    /// downloaded."
    pub max_mem_type42: u32,
    /// `minMemType1`: "Minimum memory usage when an OpenType font is
    /// downloaded as a Type 1 font."
    pub min_mem_type1: u32,
    /// `maxMemType1`: "Maximum memory usage when an OpenType font is
    /// downloaded as a Type 1 font."
    pub max_mem_type1: u32,
}

/// Reads the face's `post` header, or `None` if it has no `post` table,
/// one shorter than its header, or one with a major version other than
/// 1 to 4.
///
/// A table too short for its header counts as missing rather than as an
/// error, as `ttf-parser` treats it: `post` is optional, and the face is
/// usable without it.
///
/// # Errors
///
/// [`FontError::Source`] if the source fails or ends before the bytes
/// the table directory says are there.
pub fn read<S: FontSource + ?Sized>(
    source: &S,
    directory: &TableDirectory,
) -> Result<Option<Post>, FontError> {
    let Some(range) = directory.find(TAG) else {
        return Ok(None);
    };
    Ok(read_table_start::<LEN, _>(source, range)?.and_then(|bytes| parse(&bytes)))
}

/// Decodes a `post` header, or `None` for a major version this crate
/// cannot read.
fn parse(bytes: &[u8; LEN]) -> Option<Post> {
    let post = parse_fields(bytes).expect("`bytes` is sized for the fields");

    // "If the major version is not recognized, the implementation must
    // not read the table ... The implementation should treat the table
    // as missing." (OpenType font file, Table Version Numbers)
    matches!(post.version.major(), 1..=4).then_some(post)
}

/// Every header field, in the order the spec lists them.
fn parse_fields(bytes: &[u8]) -> Option<Post> {
    let mut reader = Reader::new(bytes);
    Some(Post {
        version: reader.read()?,
        italic_angle: reader.read()?,
        underline_position: reader.read()?,
        underline_thickness: reader.read()?,
        is_fixed_pitch: reader.read()?,
        min_mem_type42: reader.read()?,
        max_mem_type42: reader.read()?,
        min_mem_type1: reader.read()?,
        max_mem_type1: reader.read()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{LEN, parse};
    use crate::types::fword::FWord;

    /// Helvetica.ttc face 0's `post` header, as `xxd -s 0x226bbc -l 32`
    /// dumps it, with the version replaced.
    fn helvetica(version: u32) -> [u8; LEN] {
        [
            version.to_be_bytes().as_slice(),
            &[0; 4],                   // italicAngle
            &[0xFF, 0x65, 0x00, 0x65], // underlinePosition -155, underlineThickness 101
            &[0; 20],                  // isFixedPitch, the four memory fields
        ]
        .concat()
        .try_into()
        .expect("the fields add up to LEN bytes")
    }

    /// A version 2.0 table: underline 155 units below the baseline and
    /// 101 thick.
    #[test]
    fn reads_the_underline() {
        let post = parse(&helvetica(0x0002_0000)).expect("version 2.0");
        assert_eq!(post.underline_position, FWord::from(-155));
        assert_eq!(post.underline_thickness, FWord::from(101));
        assert_eq!(post.is_fixed_pitch, 0);
    }

    /// The four OpenType versions and Apple's format 4 share the header.
    #[test]
    fn reads_every_known_version() {
        for version in [
            0x0001_0000,
            0x0002_0000,
            0x0002_5000,
            0x0003_0000,
            0x0004_0000,
        ] {
            assert!(parse(&helvetica(version)).is_some(), "{version:#x}");
        }
    }

    #[test]
    fn an_unknown_major_version_is_a_missing_table() {
        for version in [0x0000_5000, 0x0005_0000] {
            assert_eq!(parse(&helvetica(version)), None, "{version:#x}");
        }
    }
}
