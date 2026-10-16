//! OpenType data types that Rust has no primitive for, one per file.
//!
//! [OpenType font file, Data Types](https://learn.microsoft.com/en-us/typography/opentype/spec/otff#data-types)
//!
//! The rest of the spec's integer types (`uint8` to `int32`, `FWORD`,
//! `UFWORD`, `Offset16`, `Offset32`) are the Rust integers of the same
//! width and signedness, with the spec's name in the doc comment of the
//! field that holds one.

pub mod f2dot14;
pub mod fixed;
pub mod fword;
pub mod tag;
pub mod u24;
pub mod ufword;
pub mod version16dot16;
