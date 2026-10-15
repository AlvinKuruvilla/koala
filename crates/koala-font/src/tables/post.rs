//! [`post` — PostScript Table](https://learn.microsoft.com/en-us/typography/opentype/spec/post): `underlinePosition` and
//! `underlineThickness` from the header. The glyph names after it are
//! not read.
//!
//! The header fields up to `underlineThickness`, the only ones read:
//!
//! ```text
//!   0  Version16Dot16  version
//!   4  Fixed           italicAngle
//!   8  FWORD           underlinePosition   "Suggested y-coordinate of the top of the underline."
//!  10  FWORD           underlineThickness
//!  12  (isFixedPitch, memory usage hints, then per-version glyph names; not read)
//! ```
//!
//! Every version starts with this header, so the version only decides
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
use crate::metrics::DecorationLine;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::tables::read_table_start;
use crate::types::fword::FWord;
use crate::types::tag::Tag;
use crate::types::version16dot16::Version16Dot16;

pub const TAG: Tag = Tag::from_literal(*b"post");

/// Bytes from the start of the table through `underlineThickness`.
const LEN: usize = 12;

/// Reads the face's underline metrics from `post`, or `None` if it has
/// no `post` table, one shorter than its header, or one with a major
/// version other than 1 to 4.
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
) -> Result<Option<DecorationLine>, FontError> {
    let Some(range) = directory.find(TAG) else {
        return Ok(None);
    };
    Ok(read_table_start::<LEN, _>(source, range)?.and_then(|bytes| parse(&bytes)))
}

/// Decodes the `post` header's underline fields, or `None` for a major
/// version this crate cannot read.
pub fn parse(bytes: &[u8; LEN]) -> Option<DecorationLine> {
    let (version, underline) = parse_fields(bytes).expect("`bytes` is sized for the fields");

    // "If the major version is not recognized, the implementation must
    // not read the table ... The implementation should treat the table
    // as missing." (OpenType font file, Table Version Numbers)
    matches!(version.major(), 1..=4).then_some(underline)
}

/// `version`, `underlinePosition` and `underlineThickness`.
fn parse_fields(bytes: &[u8]) -> Option<(Version16Dot16, DecorationLine)> {
    let mut reader = Reader::new(bytes);
    let version = reader.read()?;
    // italicAngle
    reader.skip(4)?;
    let position: FWord = reader.read()?;
    let thickness: FWord = reader.read()?;
    Some((
        version,
        DecorationLine {
            position,
            thickness,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{LEN, parse};
    use crate::metrics::DecorationLine;
    use crate::types::fword::FWord;

    /// A `post` header with these fields and a zero italic angle.
    fn post(version: u32, position: i16, thickness: i16) -> [u8; LEN] {
        [
            version.to_be_bytes().as_slice(),
            &[0; 4], // italicAngle
            &position.to_be_bytes(),
            &thickness.to_be_bytes(),
        ]
        .concat()
        .try_into()
        .expect("the fields add up to LEN bytes")
    }

    /// Helvetica.ttc face 0's values: a version 2.0 table, underline 155
    /// units below the baseline and 101 thick.
    #[test]
    fn reads_the_underline() {
        assert_eq!(
            parse(&post(0x0002_0000, -155, 101)),
            Some(DecorationLine {
                position: FWord::from(-155),
                thickness: FWord::from(101),
            })
        );
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
            assert!(parse(&post(version, -100, 50)).is_some(), "{version:#x}");
        }
    }

    #[test]
    fn an_unknown_major_version_is_a_missing_table() {
        for version in [0x0000_5000, 0x0005_0000] {
            assert_eq!(parse(&post(version, -100, 50)), None, "{version:#x}");
        }
    }
}
