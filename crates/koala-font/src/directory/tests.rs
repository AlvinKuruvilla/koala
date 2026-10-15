//! Table directories and collection headers built byte by byte, one per
//! case the spec allows or a hostile file can produce.

use std::io;

use super::{Header, TableDirectory, TableRange};
use crate::error::FontError;
use crate::types::tag::Tag;

const HEAD: Tag = Tag::from_literal(*b"head");
const POST: Tag = Tag::from_literal(*b"post");

/// A table directory with these `(tag, offset, length)` records. The
/// search fields are zero: nothing reads them.
fn table_directory(sfnt_version: [u8; 4], records: &[([u8; 4], u32, u32)]) -> Vec<u8> {
    let mut bytes = sfnt_version.to_vec();
    let num_tables = u16::try_from(records.len()).expect("tests use a few records");
    bytes.extend(num_tables.to_be_bytes());
    bytes.extend([0; 6]);
    for &(tag, offset, length) in records {
        bytes.extend(tag);
        bytes.extend([0; 4]); // checksum
        bytes.extend(offset.to_be_bytes());
        bytes.extend(length.to_be_bytes());
    }
    bytes
}

/// A collection of `directories`, laid out after the header the way
/// Helvetica.ttc is: a version 2.0 header carries 12 bytes of DSIG fields
/// between the offsets and the first directory.
fn collection(major: u16, directories: &[Vec<u8>]) -> Vec<u8> {
    let num_fonts = u32::try_from(directories.len()).expect("tests use a few faces");
    let dsig_len = if major == 2 { 12 } else { 0 };
    let mut next = 12 + 4 * directories.len() + dsig_len;

    let mut bytes = b"ttcf".to_vec();
    bytes.extend(major.to_be_bytes());
    bytes.extend(0_u16.to_be_bytes());
    bytes.extend(num_fonts.to_be_bytes());
    for directory in directories {
        bytes.extend(u32::try_from(next).expect("tests are small").to_be_bytes());
        next += directory.len();
    }
    bytes.extend(vec![0; dsig_len]);
    for directory in directories {
        bytes.extend(directory);
    }
    bytes
}

#[test]
fn finds_tables_in_a_single_font() {
    let font = table_directory(
        [0, 1, 0, 0],
        &[(*b"head", 0x100, 54), (*b"post", 0x200, 32)],
    );
    let directory = TableDirectory::read(font.as_slice(), 0).expect("a valid directory");
    assert_eq!(
        directory.find(HEAD),
        Some(TableRange {
            offset: 0x100,
            length: 54
        })
    );
    assert_eq!(
        directory.find(POST),
        Some(TableRange {
            offset: 0x200,
            length: 32
        })
    );
    assert_eq!(directory.find(Tag::from_literal(*b"OS/2")), None);
}

#[test]
fn accepts_cff_outlines() {
    let font = table_directory(*b"OTTO", &[(*b"head", 0x100, 54)]);
    let directory = TableDirectory::read(font.as_slice(), 0).expect("'OTTO' is an sfntVersion");
    assert_eq!(
        directory.find(HEAD),
        Some(TableRange {
            offset: 0x100,
            length: 54
        })
    );
}

#[test]
fn accepts_apples_truetype_signature() {
    let font = table_directory(*b"true", &[(*b"head", 0x100, 54)]);
    let directory =
        TableDirectory::read(font.as_slice(), 0).expect("'true' is Apple's sfntVersion");
    assert_eq!(
        directory.find(HEAD),
        Some(TableRange {
            offset: 0x100,
            length: 54
        })
    );
}

/// A record with an invalid tag is dropped; the records around it stay.
#[test]
fn skips_records_with_invalid_tags() {
    let font = table_directory([0, 1, 0, 0], &[(*b"he d", 0x80, 1), (*b"head", 0x100, 54)]);
    let directory = TableDirectory::read(font.as_slice(), 0).expect("a valid directory");
    assert_eq!(directory.tables.len(), 1);
    assert_eq!(
        directory.find(HEAD),
        Some(TableRange {
            offset: 0x100,
            length: 54
        })
    );
}

#[test]
fn a_single_font_has_only_face_zero() {
    let font = table_directory([0, 1, 0, 0], &[]);
    assert_eq!(
        Header::read(font.as_slice()).expect("valid").face_count(),
        1
    );
    let error = TableDirectory::read(font.as_slice(), 1).expect_err("face 1 does not exist");
    assert!(matches!(
        error,
        FontError::FaceIndexOutOfRange { index: 1, count: 1 }
    ));
}

/// Version 2.0, as Helvetica.ttc is: the second face's directory is found
/// through its offset, past the DSIG fields and the first directory.
#[test]
fn finds_a_face_in_a_version_2_collection() {
    let font = collection(
        2,
        &[
            table_directory([0, 1, 0, 0], &[(*b"head", 0x100, 54)]),
            table_directory([0, 1, 0, 0], &[(*b"head", 0x300, 54)]),
        ],
    );
    assert_eq!(
        Header::read(font.as_slice()).expect("valid"),
        Header::Collection { num_fonts: 2 }
    );
    let directory = TableDirectory::read(font.as_slice(), 1).expect("face 1 exists");
    assert_eq!(
        directory.find(HEAD),
        Some(TableRange {
            offset: 0x300,
            length: 54
        })
    );
}

#[test]
fn finds_a_face_in_a_version_1_collection() {
    let font = collection(1, &[table_directory(*b"OTTO", &[(*b"post", 0x40, 32)])]);
    let directory = TableDirectory::read(font.as_slice(), 0).expect("face 0 exists");
    assert_eq!(
        directory.find(POST),
        Some(TableRange {
            offset: 0x40,
            length: 32
        })
    );
}

#[test]
fn a_collection_index_must_be_below_num_fonts() {
    let font = collection(2, &[table_directory([0, 1, 0, 0], &[])]);
    let error = TableDirectory::read(font.as_slice(), 1).expect_err("face 1 does not exist");
    assert!(matches!(
        error,
        FontError::FaceIndexOutOfRange { index: 1, count: 1 }
    ));
}

#[test]
fn refuses_an_unknown_collection_version() {
    let font = collection(3, &[table_directory([0, 1, 0, 0], &[])]);
    let error = Header::read(font.as_slice()).expect_err("version 3 is unknown");
    assert!(matches!(
        error,
        FontError::UnsupportedCollectionVersion { major: 3 }
    ));
}

#[test]
fn refuses_an_unknown_signature() {
    let error = Header::read(b"%PDF-1.7".as_slice()).expect_err("not a font");
    assert!(matches!(error, FontError::UnrecognizedSignature(bytes) if bytes == *b"%PDF"));
}

/// A collection offset that lands on something other than a table
/// directory is caught there, not read as one.
#[test]
fn refuses_a_collection_offset_that_misses_its_directory() {
    let mut font = collection(1, &[table_directory([0, 1, 0, 0], &[])]);
    // Point face 0 back at the TTC header itself.
    font.get_mut(12..16)
        .expect("the header holds one offset")
        .copy_from_slice(&0_u32.to_be_bytes());
    let error = TableDirectory::read(font.as_slice(), 0).expect_err("offset 0 is the header");
    assert!(matches!(error, FontError::UnrecognizedSignature(bytes) if bytes == *b"ttcf"));
}

/// `numTables` promising more records than the file holds is a short
/// read, not a panic or a partial directory.
#[test]
fn truncated_records_are_a_short_read() {
    let mut font = table_directory([0, 1, 0, 0], &[(*b"head", 0x100, 54)]);
    font.truncate(font.len() - 1);
    let error = TableDirectory::read(font.as_slice(), 0).expect_err("the record is cut short");
    assert!(
        matches!(error, FontError::Source(source) if source.kind() == io::ErrorKind::UnexpectedEof)
    );
}
