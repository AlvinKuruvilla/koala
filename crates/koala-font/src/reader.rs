//! Reading big-endian fields out of bytes fetched from a font.

use crate::types::f2dot14::F2Dot14;
use crate::types::fixed::Fixed;
use crate::types::fword::FWord;
use crate::types::u24::U24;
use crate::types::ufword::UFWord;
use crate::types::version16dot16::Version16Dot16;

/// A value that `N` big-endian bytes always decode to, so
/// [`Reader::read`] can produce it.
///
/// The byte count is a parameter and not an associated constant because
/// stable Rust cannot write `[u8; Self::SIZE]` in a trait signature.
/// ttf-parser's `FromData` gets around that by taking a slice and checking
/// its length at run time; a parameter keeps the length in the type, and
/// since each type implements the trait for one `N` only, the compiler
/// infers it.
///
/// [`Tag`](crate::Tag) has no impl: not every four bytes are a valid tag,
/// so it is read with `read_array` and checked with `Tag::new`.
pub trait FromBeBytes<const N: usize> {
    fn from_be_bytes(bytes: [u8; N]) -> Self;
}

// Every type here already has an inherent `from_be_bytes` of the right
// width; the impls only forward to it.
macro_rules! forward_from_be_bytes {
    ($($ty:ty => $n:literal),* $(,)?) => {
        $(
            impl FromBeBytes<$n> for $ty {
                fn from_be_bytes(bytes: [u8; $n]) -> Self {
                    <$ty>::from_be_bytes(bytes)
                }
            }
        )*
    };
}

forward_from_be_bytes! {
    u8 => 1,
    i8 => 1,
    u16 => 2,
    i16 => 2,
    U24 => 3,
    u32 => 4,
    i32 => 4,
    F2Dot14 => 2,
    FWord => 2,
    UFWord => 2,
    Fixed => 4,
    Version16Dot16 => 4,
}

/// Reads big-endian fields one after another from a buffer already
/// fetched from a [`FontSource`](crate::FontSource), and never panics on
/// reaching its end.
///
/// `offset` is where the next read starts. Every method keeps
/// `offset <= data.len()`; at `offset == data.len()` everything has been
/// read, and only zero-byte reads still succeed.
pub struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Starts reading at the first byte of `data`.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        // Offset 0 is within any slice, the empty one included.
        Self { data, offset: 0 }
    }

    /// Takes the next `N` bytes, or returns `None` and takes nothing if
    /// fewer than `N` are left.
    ///
    /// # Time complexity
    ///
    /// *O*(`N`), for the copy.
    pub fn read_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        // The invariant makes this `get` always succeed; it is `get` and
        // not indexing only because the crate denies indexing.
        let bytes = *self.data.get(self.offset..)?.first_chunk::<N>()?;
        // The chunk fit, so `offset + N <= data.len()`: no overflow, and
        // the invariant still holds.
        self.offset += N;
        Some(bytes)
    }

    /// Reads the next value of type `T`, or returns `None` and takes
    /// nothing if fewer than `N` bytes are left.
    ///
    /// `N` is inferred from `T`, so a turbofish needs a placeholder for
    /// it: `reader.read::<FWord, _>()`. A type annotation on the binding
    /// avoids that.
    ///
    /// # Time complexity
    ///
    /// *O*(`N`), for the copy.
    pub fn read<T: FromBeBytes<N>, const N: usize>(&mut self) -> Option<T> {
        self.read_array().map(T::from_be_bytes)
    }

    /// Moves past the next `count` bytes, or returns `None` and moves
    /// nothing if fewer than `count` are left.
    ///
    /// # Time complexity
    ///
    /// *O*(1).
    pub fn skip(&mut self, count: usize) -> Option<()> {
        // `count` comes from font data, so `offset + count` can overflow;
        // checking it keeps a hostile length from panicking in debug
        // builds or wrapping to a small offset in release.
        let end = self.offset.checked_add(count)?;
        if end > self.data.len() {
            return None;
        }
        self.offset = end;
        Some(())
    }
}

#[cfg(test)]
mod tests;
