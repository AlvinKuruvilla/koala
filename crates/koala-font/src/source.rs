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
        // An offset that does not fit in a `usize` (possible on a 32-bit
        // target), or a range whose end overflows, lies past the end of
        // any slice, so both fail the same way a short range does.
        let bytes = usize::try_from(offset)
            .ok()
            .and_then(|start| self.get(start..start.checked_add(buf.len())?))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "reading {} bytes at offset {offset} runs past the end of {} bytes of font data",
                        buf.len(),
                        self.len(),
                    ),
                )
            })?;
        buf.copy_from_slice(bytes);
        Ok(())
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

#[cfg(test)]
mod tests {
    use std::io;

    use super::FontSource;

    const BYTES: &[u8] = &[1, 2, 3, 4];

    #[test]
    fn reads_the_requested_range() {
        let mut buf = [0; 2];
        BYTES.read_exact_at(&mut buf, 1).expect("bytes 1..3 exist");
        assert_eq!(buf, [2, 3]);
    }

    /// A range ending exactly at the end, and an empty one starting there,
    /// are both inside the data.
    #[test]
    fn reads_up_to_the_end() {
        let mut buf = [0; 2];
        BYTES.read_exact_at(&mut buf, 2).expect("bytes 2..4 exist");
        assert_eq!(buf, [3, 4]);
        BYTES
            .read_exact_at(&mut [], 4)
            .expect("an empty read at the end fits");
    }

    /// A short range fails like a short file read, and leaves `buf` as it
    /// was.
    #[test]
    fn past_the_end_is_unexpected_eof() {
        let mut buf = [9; 2];
        let error = BYTES
            .read_exact_at(&mut buf, 3)
            .expect_err("byte 4 does not exist");
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(buf, [9, 9]);
    }

    /// An offset from a hostile font can be anything; it must neither
    /// truncate to a small `usize` nor overflow when the length is added.
    #[test]
    fn huge_offsets_are_unexpected_eof() {
        let mut buf = [0; 1];
        for offset in [u64::MAX, u64::from(u32::MAX) + 1] {
            let error = BYTES
                .read_exact_at(&mut buf, offset)
                .expect_err("far past the end");
            assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        }
    }
}
