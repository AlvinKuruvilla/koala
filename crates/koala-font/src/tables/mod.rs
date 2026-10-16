//! The tables the metrics slice reads, each as a struct holding every
//! field of the table's fixed part, named after the spec's fields.
//!
//! A struct is a transcription of its table's field list: the spec's
//! `unitsPerEm` is [`Head::units_per_em`], `sTypoAscender` is
//! [`Os2LineMetrics::s_typo_ascender`]. Turning fields into the values
//! callers want, such as [`VerticalMetrics`](crate::VerticalMetrics),
//! happens in [`Font`](crate::Font), where it can be read beside the
//! code that chooses between tables.
//!
//! Only each table's fixed part is read. What follows it (`post`'s glyph
//! names) is per-glyph data the metrics do not need.

mod head;
mod hhea;
mod os2;
mod post;

pub use head::Head;
pub use hhea::Hhea;
pub use os2::{Os2, Os2LineMetrics, Os2Version1, Os2Version2, Os2Version5};
pub use post::Post;

pub(crate) use head::read as read_head;
pub(crate) use hhea::read as read_hhea;
pub(crate) use os2::read as read_os2;
pub(crate) use post::read as read_post;

use crate::directory::TableRange;
use crate::error::FontError;
use crate::source::FontSource;

/// The first `N` bytes of the table at `range`, or `None` if the table
/// directory gives it fewer than `N`.
///
/// # Errors
///
/// [`FontError::Source`] if the source fails or ends before the bytes
/// the table directory says are there.
fn read_table_start<const N: usize, S: FontSource + ?Sized>(
    source: &S,
    range: TableRange,
) -> Result<Option<[u8; N]>, FontError> {
    // A `u32` length that does not fit a `usize` is longer than any `N`.
    if usize::try_from(range.length).is_ok_and(|length| length < N) {
        return Ok(None);
    }
    let mut bytes = [0; N];
    source.read_exact_at(&mut bytes, u64::from(range.offset))?;
    Ok(Some(bytes))
}

/// The first `max` bytes of the table at `range`, or all of it if it is
/// shorter: for a table whose version decides its length.
///
/// # Errors
///
/// [`FontError::Source`] if the source fails or ends before the bytes
/// the table directory says are there.
fn read_table_up_to<S: FontSource + ?Sized>(
    source: &S,
    range: TableRange,
    max: usize,
) -> Result<Vec<u8>, FontError> {
    let length = usize::try_from(range.length).map_or(max, |length| length.min(max));
    let mut bytes = vec![0; length];
    source.read_exact_at(&mut bytes, u64::from(range.offset))?;
    Ok(bytes)
}
