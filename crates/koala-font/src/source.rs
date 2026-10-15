//! Where a font's bytes come from.

use std::io;
use std::sync::Arc;

/// Where a font's bytes come from.
///
/// The signature is `std::os::unix::fs::FileExt::read_exact_at`'s, so a
/// file can be a source through positioned reads, without loading it.
/// Bytes already in memory are a source as they are.
pub trait FontSource {
    /// Fill `buf` with the bytes starting at `offset`.
    ///
    /// # Errors
    ///
    /// If fewer than `buf.len()` bytes exist from `offset` on, an error
    /// of kind [`io::ErrorKind::UnexpectedEof`]; any other error the
    /// underlying storage reports.
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
}

impl FontSource for [u8] {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let _ = (buf, offset);
        todo!("copy the requested range, or UnexpectedEof past the end")
    }
}

impl FontSource for Vec<u8> {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.as_slice().read_exact_at(buf, offset)
    }
}

impl FontSource for Arc<[u8]> {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        (**self).read_exact_at(buf, offset)
    }
}
