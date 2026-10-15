//! [`head` — Font header table](https://learn.microsoft.com/en-us/typography/opentype/spec/head): `unitsPerEm`, and
//! `magicNumber` as a check that this is the table it claims to be.
//!
//! The fields up to `unitsPerEm`, the only ones read:
//!
//! ```text
//!   0  uint16  majorVersion        "set to 1"
//!   2  uint16  minorVersion
//!   4  Fixed   fontRevision
//!   8  uint32  checksumAdjustment
//!  12  uint32  magicNumber         "Set to 0x5F0F3CF5."
//!  16  uint16  flags
//!  18  uint16  unitsPerEm          "Set to a value from 16 to 16384."
//!  20  (created, modified, the bounding box, ... not read)
//! ```

use crate::directory::TableDirectory;
use crate::error::FontError;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::tables::read_table_start;
use crate::types::tag::Tag;

pub const TAG: Tag = Tag::from_literal(*b"head");

/// Bytes from the start of the table through `unitsPerEm`.
const LEN: usize = 20;

/// `magicNumber`: "Set to 0x5F0F3CF5."
const MAGIC_NUMBER: u32 = 0x5F0F_3CF5;

/// The `head` fields the metrics slice uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    /// `unitsPerEm`, checked to be in 16..=16384.
    pub units_per_em: u16,
}

/// Reads the face's `head` table. Every face needs one: without
/// `unitsPerEm`, no value in font units can be scaled.
///
/// # Errors
///
/// [`FontError::MissingTable`] if the face has no `head` table or one
/// with a major version other than 1; [`FontError::TableTooShort`] if it
/// stops before `unitsPerEm`; and [`parse`]'s errors.
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

/// Decodes the start of a `head` table, or `None` for a major version
/// this crate cannot read.
///
/// # Errors
///
/// [`FontError::HeadMagicNumberMismatch`] if `magicNumber` is wrong;
/// [`FontError::UnitsPerEmOutOfRange`] if `unitsPerEm` is outside
/// 16..=16384.
pub fn parse(bytes: &[u8; LEN]) -> Result<Option<Head>, FontError> {
    let (major_version, magic_number, units_per_em) =
        parse_fields(bytes).expect("`bytes` is sized for the fields");

    // "If the major version is not recognized, the implementation must
    // not read the table as it can make no assumptions regarding
    // interpretation of the binary data. The implementation should treat
    // the table as missing." (OpenType font file, Table Version Numbers)
    if major_version != 1 {
        return Ok(None);
    }

    // A record whose offset is wrong still points at 20 readable bytes
    // more often than not; the magic number is what tells them apart.
    if magic_number != MAGIC_NUMBER {
        return Err(FontError::HeadMagicNumberMismatch(magic_number));
    }

    // "Set to a value from 16 to 16384. Any value in this range is
    // valid." Outside it the face is refused rather than read: every
    // distance in font units is divided by `unitsPerEm`, and a 0 would
    // turn each into infinity or NaN.
    if !(16..=16384).contains(&units_per_em) {
        return Err(FontError::UnitsPerEmOutOfRange(units_per_em));
    }

    Ok(Some(Head { units_per_em }))
}

/// `majorVersion`, `magicNumber` and `unitsPerEm`.
fn parse_fields(bytes: &[u8]) -> Option<(u16, u32, u16)> {
    let mut reader = Reader::new(bytes);
    let major_version = reader.read()?;
    // minorVersion, fontRevision, checksumAdjustment
    reader.skip(2 + 4 + 4)?;
    let magic_number = reader.read()?;
    // flags
    reader.skip(2)?;
    let units_per_em = reader.read()?;
    Some((major_version, magic_number, units_per_em))
}

#[cfg(test)]
mod tests {
    use super::{Head, LEN, parse};
    use crate::error::FontError;

    /// The start of a `head` table with these fields and zeros elsewhere.
    fn head(major_version: u16, magic_number: u32, units_per_em: u16) -> [u8; LEN] {
        [
            major_version.to_be_bytes().as_slice(),
            &[0; 10], // minorVersion, fontRevision, checksumAdjustment
            &magic_number.to_be_bytes(),
            &[0; 2], // flags
            &units_per_em.to_be_bytes(),
        ]
        .concat()
        .try_into()
        .expect("the fields add up to LEN bytes")
    }

    /// Helvetica.ttc face 0's values.
    #[test]
    fn reads_units_per_em() {
        let parsed = parse(&head(1, 0x5F0F_3CF5, 2048)).expect("a valid head");
        assert_eq!(parsed, Some(Head { units_per_em: 2048 }));
    }

    #[test]
    fn an_unknown_major_version_is_a_missing_table() {
        assert_eq!(
            parse(&head(2, 0x5F0F_3CF5, 2048)).expect("not an error"),
            None
        );
    }

    #[test]
    fn refuses_a_wrong_magic_number() {
        let error = parse(&head(1, 0x1234_5678, 2048)).expect_err("wrong magic");
        assert!(matches!(
            error,
            FontError::HeadMagicNumberMismatch(0x1234_5678)
        ));
    }

    #[test]
    fn units_per_em_must_be_16_to_16384() {
        for valid in [16, 1000, 16384] {
            assert!(parse(&head(1, 0x5F0F_3CF5, valid)).is_ok(), "{valid}");
        }
        for invalid in [0, 15, 16385, u16::MAX] {
            let error = parse(&head(1, 0x5F0F_3CF5, invalid)).expect_err("out of range");
            assert!(matches!(error, FontError::UnitsPerEmOutOfRange(value) if value == invalid));
        }
    }
}
