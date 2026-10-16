//! A font file is untrusted input: whatever its bytes, opening it gives a
//! font or an error, and nothing panics.
//!
//! Random bytes rarely get past the first four, so these tests damage a
//! real font instead: cut short at an arbitrary length, or with arbitrary
//! bytes overwritten. That reaches the table directory and the tables,
//! where offsets and lengths come from the file.

mod common;

use koala_font::{Font, FontSettings, FontSource, face_count};
use quickcheck::quickcheck;

/// The smallest system font, so each case is cheap to copy.
fn sample_font() -> Option<Vec<u8>> {
    common::system_font_files()
        .into_iter()
        .filter_map(|path| std::fs::read(path).ok())
        .min_by_key(Vec::len)
}

/// Open face `index` of `bytes` and read everything; only a panic fails.
fn open_and_read(bytes: Vec<u8>, index: u32) {
    let _ = face_count(bytes.as_slice());
    if let Ok(font) = Font::from_source(
        bytes,
        FontSettings {
            collection_index: index,
        },
    ) {
        read_everything(&font);
    }
}

fn read_everything<S: FontSource>(font: &Font<S>) {
    let _ = font.units_per_em();
    let _ = font.hhea_metrics();
    let _ = font.typo_metrics();
    let _ = font.underline();
    let _ = font.strikeout();
    let _ = font.scale_factor(16.0);
}

quickcheck! {
    fn truncated_font_does_not_panic(cut: usize, index: u8) -> bool {
        let Some(mut bytes) = sample_font() else { return true };
        bytes.truncate(cut % (bytes.len() + 1));
        open_and_read(bytes, u32::from(index % 4));
        true
    }

    fn overwritten_font_does_not_panic(writes: Vec<(usize, u8)>, index: u8) -> bool {
        let Some(mut bytes) = sample_font() else { return true };
        let len = bytes.len();
        // Most writes land in the first kilobyte, where the headers, the
        // table directory and the small tables are.
        for (at, value) in writes {
            let at = if at % 4 == 0 { at % len } else { at % len.min(1024) };
            if let Some(byte) = bytes.get_mut(at) {
                *byte = value;
            }
        }
        open_and_read(bytes, u32::from(index % 4));
        true
    }

    fn arbitrary_bytes_do_not_panic(bytes: Vec<u8>, index: u8) -> bool {
        open_and_read(bytes, u32::from(index % 4));
        true
    }
}
