//! [OpenType font file](https://learn.microsoft.com/en-us/typography/opentype/spec/otff): where a face's tables are.
//!
//! Reads the TTC header (for a collection) and the face's table
//! directory, and answers "where is table `tag`, and how long is it?".
//! Read in the spec: Data Types, Organization of an OpenType Font, Table
//! Directory, and Font Collections → TTC Header.
//!
//! A file starts with either a table directory (a single font) or a TTC
//! header pointing at one table directory per face. Helvetica.ttc, the
//! renderer's first choice on macOS, is a version 2.0 collection of six
//! faces:
//!
//! ```text
//!   0x00  'ttcf' 0002 0000 00000006   ttcTag, majorVersion, minorVersion, numFonts
//!   0x0c  00000030 0000017c ...       tableDirectoryOffsets[6]
//!   0x24  DSIG tag, length, offset    version 2.0 only; zero when unsigned
//!   0x30  00010000 0014 ...           face 0's table directory: 20 tables
//! ```
//!
//! Face 0's directory starts after the DSIG fields, not right after the
//! offsets array, so a directory's position always comes from
//! `tableDirectoryOffsets` and never from arithmetic on `numFonts`.
//!
//! A table record whose tag breaks the spec's rule for tags (`Tag::new`
//! fails) is skipped, not an error: lookups only ever ask for valid tags,
//! so such a record could never be found anyway.
//!
//! # Trade-offs
//!
//! - **One read for all records**: [`TableDirectory::read`] fetches the
//!   whole `tableRecords` array at once and keeps every valid record.
//!   Reading record by record would allocate nothing but make one source
//!   call per table, which for a file is a system call each. The cost is
//!   that a font claiming 65,535 tables makes the read 1 MiB; real fonts
//!   have 10 to 30.
//! - **Linear lookup**: "The records in the array must be sorted in
//!   ascending order by tag", so a binary search would work on fonts that
//!   obey that, but over about 20 records it saves nothing, and a scan
//!   does not depend on the font obeying it.

use std::iter;

use crate::error::FontError;
use crate::reader::Reader;
use crate::source::FontSource;
use crate::types::tag::Tag;

/// `sfntVersion` "0x00010000": "OpenType fonts that contain TrueType
/// outlines should use the value of 0x00010000".
const TRUETYPE_OUTLINES: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

/// Apple's `sfntVersion` for TrueType outlines, which the OpenType spec
/// does not list. The [TrueType Reference Manual, chapter 6](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6.html)
/// calls the field the scaler type: "The values 'true' (0x74727565) and
/// 0x00010000 are recognized by OS X and iOS as referring to TrueType
/// fonts." macOS ships fonts that use it, such as
/// `/System/Library/Fonts/Supplemental/NISC18030.ttf`.
const APPLE_TRUETYPE_OUTLINES: [u8; 4] = *b"true";

/// `sfntVersion` "0x4F54544F ('OTTO')", for CFF outlines.
const CFF_OUTLINES: [u8; 4] = *b"OTTO";

/// `ttcTag`: "Font Collection ID string: 'ttcf'".
const COLLECTION: [u8; 4] = *b"ttcf";

/// The `TableDirectory` header's size: `sfntVersion` (uint32), then
/// `numTables`, `searchRange`, `entrySelector`, `rangeShift` (uint16
/// each).
const DIRECTORY_HEADER_LEN: u64 = 12;

/// A `TableRecord`'s size: `tableTag`, `checksum`, `offset`, `length`,
/// four bytes each.
const RECORD_LEN: usize = 16;

/// What a font file holds, from the start of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Header {
    /// One font: the file starts with its table directory.
    Single,
    /// A TTC header followed by `num_fonts` table directories.
    Collection {
        /// `numFonts`: "Number of fonts in TTC".
        num_fonts: u32,
    },
}

impl Header {
    /// Reads the signature at the start of `source`, and for a
    /// collection the header fields after it.
    ///
    /// # Errors
    ///
    /// [`FontError::UnrecognizedSignature`] if the file starts with
    /// neither an `sfntVersion` nor `ttcf`;
    /// [`FontError::UnsupportedCollectionVersion`] for a TTC header that
    /// is neither version 1 nor 2; [`FontError::Source`] if the source
    /// fails or ends inside the header.
    pub fn read<S: FontSource + ?Sized>(source: &S) -> Result<Self, FontError> {
        let mut signature = [0; 4];
        source.read_exact_at(&mut signature, 0)?;
        match signature {
            TRUETYPE_OUTLINES | APPLE_TRUETYPE_OUTLINES | CFF_OUTLINES => Ok(Self::Single),
            COLLECTION => {
                let mut fields = [0; 8];
                source.read_exact_at(&mut fields, 4)?;
                let (major, num_fonts) =
                    parse_collection_fields(&fields).expect("`fields` is sized for them");

                // Version 2.0 adds `dsigTag`, `dsigLength` and `dsigOffset`
                // after `tableDirectoryOffsets` and changes nothing before
                // them, so both versions read the same. A later major
                // version may move fields, so it is refused rather than
                // guessed at. The minor version is ignored: a minor
                // revision does not move existing fields.
                if !matches!(major, 1 | 2) {
                    return Err(FontError::UnsupportedCollectionVersion { major });
                }
                Ok(Self::Collection { num_fonts })
            }
            _ => Err(FontError::UnrecognizedSignature(signature)),
        }
    }

    /// How many faces the file holds.
    pub const fn face_count(self) -> u32 {
        match self {
            Self::Single => 1,
            Self::Collection { num_fonts } => num_fonts,
        }
    }

    /// Where face `index`'s table directory starts, from the start of the
    /// file.
    ///
    /// # Errors
    ///
    /// [`FontError::FaceIndexOutOfRange`] if there is no such face;
    /// [`FontError::Source`] if the source fails or ends inside
    /// `tableDirectoryOffsets`.
    fn directory_offset<S: FontSource + ?Sized>(
        self,
        source: &S,
        index: u32,
    ) -> Result<u32, FontError> {
        let count = self.face_count();
        if index >= count {
            return Err(FontError::FaceIndexOutOfRange { index, count });
        }
        match self {
            Self::Single => Ok(0),
            Self::Collection { .. } => {
                // "Offset32 tableDirectoryOffsets[numFonts]", right after
                // the 12 bytes of tag, versions and count. In `u64`,
                // `12 + 4 × u32::MAX` cannot overflow.
                let mut offset = [0; 4];
                source.read_exact_at(&mut offset, 12 + 4 * u64::from(index))?;
                Ok(Reader::new(&offset)
                    .read()
                    .expect("`offset` is sized for it"))
            }
        }
    }
}

/// Where a table's bytes are in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRange {
    /// "Offset from beginning of font file", even within a collection.
    pub offset: u32,
    /// "Length of this table", in bytes.
    pub length: u32,
}

/// One face's table directory: the valid-tagged records, in file order.
#[derive(Debug)]
pub struct TableDirectory {
    tables: Vec<(Tag, TableRange)>,
}

impl TableDirectory {
    /// Reads the table directory of face `collection_index`.
    ///
    /// # Errors
    ///
    /// Any of [`Header::read`]'s; [`FontError::FaceIndexOutOfRange`] if
    /// the face does not exist; [`FontError::UnrecognizedSignature`] if
    /// the face's directory does not start with an `sfntVersion`;
    /// [`FontError::Source`] if the source fails or ends inside the
    /// directory.
    pub fn read<S: FontSource + ?Sized>(
        source: &S,
        collection_index: u32,
    ) -> Result<Self, FontError> {
        let offset = Header::read(source)?.directory_offset(source, collection_index)?;
        let offset = u64::from(offset);

        let mut header = [0; 12];
        source.read_exact_at(&mut header, offset)?;
        let (sfnt_version, num_tables) =
            parse_directory_header(&header).expect("`header` is sized for it");

        // For a single font this repeats the check `Header::read` made on
        // the same bytes. For a collection it is the first check that
        // the offset points at a table directory at all.
        if !matches!(
            sfnt_version,
            TRUETYPE_OUTLINES | APPLE_TRUETYPE_OUTLINES | CFF_OUTLINES
        ) {
            return Err(FontError::UnrecognizedSignature(sfnt_version));
        }

        // "TableRecord tableRecords[numTables]", right after the header.
        let mut records = vec![0; usize::from(num_tables) * RECORD_LEN];
        source.read_exact_at(&mut records, offset + DIRECTORY_HEADER_LEN)?;

        // `records` holds exactly `num_tables` records, so the reader
        // runs out at the end of the last one and nowhere else.
        let mut reader = Reader::new(&records);
        let tables = iter::from_fn(|| parse_record(&mut reader))
            .filter_map(|(tag, range)| Some((Tag::new(tag).ok()?, range)))
            .collect();
        Ok(Self { tables })
    }

    /// Where table `tag` is, or `None` if the face has no such table.
    ///
    /// "For table tags defined in this specification, a font resource
    /// should have at most one table record using a given tag. If a font
    /// resource does contain more than one table of a given type,
    /// behaviour is unpredictable". This returns the first.
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) in the number of tables.
    pub fn find(&self, tag: Tag) -> Option<TableRange> {
        self.tables
            .iter()
            .find(|&&(record_tag, _)| record_tag == tag)
            .map(|&(_, range)| range)
    }
}

// The parsers below return `None` only when `reader` runs out. The fixed
// headers are read into buffers sized for exactly their fields, so their
// callers `expect`; the records parser returning `None` ends the records.

/// The TTC header after `ttcTag`: `majorVersion` and `numFonts`.
fn parse_collection_fields(bytes: &[u8]) -> Option<(u16, u32)> {
    let mut reader = Reader::new(bytes);
    // "uint16 majorVersion", "uint16 minorVersion", "uint32 numFonts"
    let major = reader.read()?;
    reader.skip(2)?;
    let num_fonts = reader.read()?;
    Some((major, num_fonts))
}

/// The `TableDirectory` header: `sfntVersion` and `numTables`.
fn parse_directory_header(bytes: &[u8]) -> Option<([u8; 4], u16)> {
    let mut reader = Reader::new(bytes);
    // "uint32 sfntVersion", "uint16 numTables". The rest of the header
    // (`searchRange`, `entrySelector`, `rangeShift`) only serves a binary
    // search, which `find` does not do. `sfntVersion` stays bytes: it is
    // compared with byte signatures, and reported as bytes if it matches
    // none.
    let sfnt_version = reader.read_array()?;
    let num_tables = reader.read()?;
    Some((sfnt_version, num_tables))
}

/// The next `TableRecord`, with its tag not yet validated.
fn parse_record(reader: &mut Reader<'_>) -> Option<([u8; 4], TableRange)> {
    // "Tag tableTag", "uint32 checksum", "Offset32 offset", "uint32 length".
    // Checksums are not verified: a wrong one does not make the table
    // unreadable, and checking means reading the whole table.
    let tag = reader.read_array()?;
    reader.skip(4)?;
    let offset = reader.read()?;
    let length = reader.read()?;
    Some((tag, TableRange { offset, length }))
}

#[cfg(test)]
mod tests;
