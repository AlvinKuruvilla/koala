//! `koala-font` against `ttf-parser`, on every face of every system font.
//!
//! `ttf-parser` is the oracle: wherever both read a value, they must
//! agree. Each face is read twice, from bytes in memory and from the file
//! through positioned reads, and the file read must stay small.

mod common;

use std::cell::Cell;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;

use koala_font::{
    DecorationLine, FWord, Font, FontSettings, FontSource, VerticalMetrics, face_count,
};

/// The most a face's metrics may read from its file.
///
/// Derived, then given headroom: a collection header (12 bytes plus 4
/// per face), a table directory (12 bytes plus 16 per table, and fonts
/// carry about 10-30 tables), `head` (54), `hhea` (36), `OS/2` (at most
/// 100) and the `post` header (32) come to well under a kilobyte. Going
/// over 4 KiB means some read took a whole large table, or the file,
/// which is what reading through a source exists to avoid.
const READ_BUDGET: u64 = 4096;

/// A font file read through positioned reads, counting the bytes asked
/// for.
struct CountingFile {
    file: File,
    bytes_read: Cell<u64>,
}

impl FontSource for CountingFile {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        self.bytes_read.set(self.bytes_read.get() + buf.len() as u64);
        self.file.read_exact_at(buf, offset)
    }
}

/// What `ttf-parser` reads for a face, from its raw tables.
///
/// Raw tables, not `Face::underline_metrics()` and friends: those apply
/// variable-font adjustments (`MVAR`), which this slice leaves out.
struct Expected {
    units_per_em: u16,
    hhea: VerticalMetrics,
    /// `None` when `ttf-parser` dropped the table: it does so for a
    /// version above 5, or a version 0 table under 78 bytes. `koala-font`
    /// reports what such a table has, so those faces are not compared.
    os2: Option<(VerticalMetrics, DecorationLine)>,
    underline: Option<DecorationLine>,
}

impl Expected {
    fn of(face: &ttf_parser::Face<'_>) -> Self {
        let tables = face.tables();
        Self {
            units_per_em: tables.head.units_per_em,
            hhea: VerticalMetrics {
                ascender: FWord::from(tables.hhea.ascender),
                descender: FWord::from(tables.hhea.descender),
                line_gap: FWord::from(tables.hhea.line_gap),
            },
            os2: tables.os2.map(|os2| {
                let strikeout = os2.strikeout_metrics();
                (
                    VerticalMetrics {
                        ascender: FWord::from(os2.typographic_ascender()),
                        descender: FWord::from(os2.typographic_descender()),
                        line_gap: FWord::from(os2.typographic_line_gap()),
                    },
                    DecorationLine {
                        position: FWord::from(strikeout.position),
                        thickness: FWord::from(strikeout.thickness),
                    },
                )
            }),
            underline: tables.post.map(|post| DecorationLine {
                position: FWord::from(post.underline_metrics.position),
                thickness: FWord::from(post.underline_metrics.thickness),
            }),
        }
    }

    /// Each way `font` differs from `self`, described.
    fn differences<S: FontSource>(&self, font: &Font<S>) -> Vec<String> {
        let mut differences = Vec::new();
        let mut check = |what: &str, ours: String, theirs: String| {
            if ours != theirs {
                differences.push(format!("{what}: koala-font {ours}, ttf-parser {theirs}"));
            }
        };
        check("unitsPerEm", format!("{}", font.units_per_em()), format!("{}", self.units_per_em));
        check("hhea", format!("{:?}", font.hhea_metrics()), format!("{:?}", self.hhea));
        check("underline", format!("{:?}", font.underline()), format!("{:?}", self.underline));
        if let Some((typo, strikeout)) = self.os2 {
            check("typo", format!("{:?}", font.typo_metrics()), format!("{:?}", Some(typo)));
            check("strikeout", format!("{:?}", font.strikeout()), format!("{:?}", Some(strikeout)));
        }
        differences
    }
}

#[test]
fn metrics_match_ttf_parser_on_every_system_font() {
    let files = common::system_font_files();
    assert!(!files.is_empty(), "no font files found; nothing was tested");

    let mut faces = 0;
    let mut failures = Vec::new();
    for path in &files {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
        match face_count(bytes.as_slice()) {
            Ok(ours) if ours == count => {}
            other => failures.push(format!("{}: face count {other:?}, expected {count}", path.display())),
        }

        let shared: Arc<[u8]> = Arc::from(bytes.as_slice());
        for index in 0..count {
            // A face ttf-parser cannot read gives nothing to compare with.
            let Ok(face) = ttf_parser::Face::parse(&bytes, index) else { continue };
            faces += 1;
            let expected = Expected::of(&face);
            let settings = FontSettings { collection_index: index };
            let name = format!("{} face {index}", path.display());

            match Font::from_source(Arc::clone(&shared), settings) {
                Ok(font) => failures.extend(
                    expected.differences(&font).into_iter().map(|d| format!("{name} (memory): {d}")),
                ),
                Err(error) => failures.push(format!("{name} (memory): {error}")),
            }

            match Font::from_source(counting_file(path), settings) {
                Ok(font) => {
                    failures.extend(
                        expected.differences(&font).into_iter().map(|d| format!("{name} (file): {d}")),
                    );
                    let read = font.source().bytes_read.get();
                    if read > READ_BUDGET {
                        failures.push(format!("{name} (file): read {read} bytes, budget {READ_BUDGET}"));
                    }
                }
                Err(error) => failures.push(format!("{name} (file): {error}")),
            }
        }
    }

    assert!(faces > 0, "ttf-parser read no faces; nothing was compared");
    assert!(
        failures.is_empty(),
        "{} problems across {faces} faces in {} files:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

fn counting_file(path: &Path) -> CountingFile {
    CountingFile {
        file: File::open(path).expect("the file was just read"),
        bytes_read: Cell::new(0),
    }
}
