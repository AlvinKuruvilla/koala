//! The tables the metrics slice reads, one module per table, each named
//! after the spec page that defines it.
//!
//! Each module reads only the start of its table: the fields it uses and
//! those before them, never the whole table. A table's `read` fetches
//! those bytes through [`read_table_start`], and its `parse` decodes them;
//! the split keeps the decoding testable on bytes alone.

pub mod head;
mod hhea;
mod os2;
pub mod post;

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
pub fn read_table_start<const N: usize, S: FontSource + ?Sized>(
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
