//! Why a font could not be opened.

use std::io;

use crate::types::tag::Tag;

/// Why a font could not be opened. Each is a reason to fall back to
/// another font.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// The source failed, or ended where the font's structure says more
    /// bytes follow.
    #[error("reading the font failed: {0}")]
    Source(#[from] io::Error),
    /// The first four bytes are neither an OpenType `sfntVersion` nor
    /// `ttcf`. They are kept as bytes: a file that is not a font can
    /// start with anything, and `sfntVersion` 0x00010000 is not a tag.
    #[error("not an OpenType font or font collection: it starts with bytes {0:02x?}")]
    UnrecognizedSignature([u8; 4]),
    /// The requested face is past the end of the collection.
    #[error("face {index} was requested, but the font holds {count}")]
    FaceIndexOutOfRange {
        /// The face asked for.
        index: u32,
        /// How many faces the font holds.
        count: u32,
    },
    /// A table every face needs is not in the table directory.
    #[error("the font has no '{0}' table")]
    MissingTable(Tag),
    /// A table is shorter than the fields read from it.
    #[error("the '{tag}' table is {length} bytes, too short for its fields")]
    TableTooShort {
        /// The table.
        tag: Tag,
        /// Its length in the table directory.
        length: u32,
    },
}
