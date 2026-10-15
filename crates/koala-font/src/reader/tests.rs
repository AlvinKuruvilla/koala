//! The reader's contract, written ahead of the reader.
//!
//! The API these assume: `Reader::new(&[u8])`, `read_array::<N>()`
//! returning the next N bytes as `Option<[u8; N]>`, and `skip(n)`
//! returning `Option<()>`. A read or skip that runs past the end returns
//! `None` and consumes nothing. Typed values come from each type's
//! `from_be_bytes`, so these hold however typed reads are layered on top.

use quickcheck::quickcheck;

use super::Reader;
use crate::types::fixed::Fixed;
use crate::types::fword::FWord;

#[test]
fn reads_big_endian_fields_in_order() {
    let bytes = [0x00, 0x01, 0xFF, 0xFE, 0x00, 0x01, 0x80, 0x00];
    let mut reader = Reader::new(&bytes);
    assert_eq!(reader.read_array().map(u16::from_be_bytes), Some(1));
    assert_eq!(
        reader.read_array().map(FWord::from_be_bytes),
        Some(FWord::from(-2))
    );
    assert_eq!(
        reader
            .read_array()
            .map(Fixed::from_be_bytes)
            .map(Fixed::to_f64),
        Some(1.5)
    );
    assert_eq!(reader.read_array::<1>(), None);
}

/// `read` decodes the same bytes `read_array` plus the type's own
/// `from_be_bytes` would, inferring the width from the type.
#[test]
fn typed_reads_infer_their_width() {
    let bytes = [0x00, 0x01, 0xFF, 0xFE, 0x00, 0x01, 0x80, 0x00];
    let mut reader = Reader::new(&bytes);
    let units: u16 = reader.read().expect("8 bytes hold a u16");
    assert_eq!(units, 1);
    assert_eq!(reader.read::<FWord, _>(), Some(FWord::from(-2)));
    assert_eq!(reader.read::<Fixed, _>().map(Fixed::to_f64), Some(1.5));
    assert_eq!(reader.read::<u8, _>(), None);
}

/// The table directory's header, read as the spec lists its fields:
/// `sfntVersion` (uint32), `numTables`, `searchRange`, `entrySelector`,
/// `rangeShift` (uint16 each). Ten tables: searchRange is 16 times the
/// largest power of two not above it (8), entrySelector its log2 (3),
/// rangeShift 16 times numTables minus searchRange.
#[test]
fn reads_a_table_directory_header_in_spec_order() {
    let bytes = [
        0x00, 0x01, 0x00, 0x00, // sfntVersion: TrueType outlines
        0x00, 0x0A, // numTables
        0x00, 0x80, // searchRange
        0x00, 0x03, // entrySelector
        0x00, 0x20, // rangeShift
    ];
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        reader.read_array().map(u32::from_be_bytes),
        Some(0x0001_0000)
    );
    assert_eq!(reader.read_array().map(u16::from_be_bytes), Some(10));
    assert_eq!(reader.read_array().map(u16::from_be_bytes), Some(128));
    assert_eq!(reader.read_array().map(u16::from_be_bytes), Some(3));
    assert_eq!(reader.read_array().map(u16::from_be_bytes), Some(32));
    assert_eq!(reader.read_array::<1>(), None);
}

/// A read that does not fit takes nothing, so what is left can still be
/// read.
#[test]
fn failed_read_consumes_nothing() {
    let bytes = [1, 2, 3];
    let mut reader = Reader::new(&bytes);
    assert_eq!(reader.read_array::<4>(), None);
    assert_eq!(reader.read_array::<3>(), Some([1, 2, 3]));
    assert_eq!(reader.read_array::<1>(), None);
}

#[test]
fn empty_read_always_succeeds() {
    let mut reader = Reader::new(&[]);
    assert_eq!(reader.read_array::<0>(), Some([]));
}

/// `head` starts with nine fields Koala does not use before `unitsPerEm`
/// at byte 18: skipping them lands on it.
#[test]
fn skip_passes_over_unused_fields() {
    // Eighteen bytes of fields Koala skips, then unitsPerEm = 2048.
    let head = [[0; 18].as_slice(), &[0x08, 0x00]].concat();
    let mut reader = Reader::new(&head);
    assert_eq!(reader.skip(18), Some(()));
    assert_eq!(reader.read_array().map(u16::from_be_bytes), Some(2048));
    assert_eq!(reader.skip(1), None);
}

/// A skip that does not fit moves nothing either.
#[test]
fn failed_skip_consumes_nothing() {
    let bytes = [7, 8];
    let mut reader = Reader::new(&bytes);
    assert_eq!(reader.skip(3), None);
    assert_eq!(reader.read_array::<2>(), Some([7, 8]));
}

/// A length read from a hostile font can be anything; once the offset is
/// past zero, `offset + usize::MAX` overflows.
#[test]
fn huge_skip_does_not_overflow() {
    let bytes = [7, 8];
    let mut reader = Reader::new(&bytes);
    assert_eq!(reader.read_array::<1>(), Some([7]));
    assert_eq!(reader.skip(usize::MAX), None);
    assert_eq!(reader.read_array::<1>(), Some([8]));
}

quickcheck! {
    /// Whatever the bytes and however they are read, nothing panics, and
    /// the successful reads and skips together never take more bytes
    /// than there are.
    fn reads_never_overrun(bytes: Vec<u8>, steps: Vec<(bool, u8)>) -> bool {
        let mut reader = Reader::new(&bytes);
        let mut taken = 0;
        for (skip, size) in steps {
            let size = usize::from(size % 8);
            let ok = if skip {
                reader.skip(size).is_some()
            } else {
                read_dynamic(&mut reader, size)
            };
            if ok {
                taken += size;
            }
        }
        taken <= bytes.len()
    }
}

/// `read_array` takes its length as a constant; this picks one at run
/// time so the property test can vary it.
fn read_dynamic(reader: &mut Reader<'_>, size: usize) -> bool {
    match size {
        0 => reader.read_array::<0>().is_some(),
        1 => reader.read_array::<1>().is_some(),
        2 => reader.read_array::<2>().is_some(),
        3 => reader.read_array::<3>().is_some(),
        4 => reader.read_array::<4>().is_some(),
        5 => reader.read_array::<5>().is_some(),
        6 => reader.read_array::<6>().is_some(),
        _ => reader.read_array::<7>().is_some(),
    }
}
