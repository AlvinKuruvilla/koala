//! `Version16Dot16`.

use std::fmt;

/// `Version16Dot16`: "Packed 32-bit value with major and minor version
/// numbers."
///
/// [OpenType font file, Table Version Numbers](https://learn.microsoft.com/en-us/typography/opentype/spec/otff#table-version-numbers)
///
/// "the upper 16 bits comprise a major version number, and the lower 16
/// bits, a minor version. Non-zero minor version numbers are represented
/// using digits 0 to 9 in the highest-order nibbles of the lower 16
/// bits." So `post` version 2.5 is `0x0002_5000`: the minor half holds
/// the digit 5, not the fraction a [`Fixed`](super::fixed::Fixed) would
/// read (2.3125).
///
/// Used only by `maxp`, `post` and `vhea`.
///
/// The derived ordering is the ordering of versions: the major halves
/// compare first, and minor digits written from the highest nibble down
/// compare as decimal digits after the point do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version16Dot16(u32);

impl Version16Dot16 {
    /// The value of four bytes in big-endian order, the byte order
    /// OpenType stores every integer in.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_be_bytes(bytes))
    }

    /// The version whose packed 32-bit value is `bits`, as the spec
    /// writes them: `Version16Dot16::from_bits(0x0002_5000)` is 2.5.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// The packed 32-bit value.
    #[must_use]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// The major version, the upper 16 bits. The spec's rule for tables:
    /// "If the major version is not recognized, the implementation must
    /// not read the table".
    #[must_use]
    pub const fn major(self) -> u16 {
        let [high, low, _, _] = self.0.to_be_bytes();
        u16::from_be_bytes([high, low])
    }

    /// The minor version as stored: the lower 16 bits, digits in the
    /// high nibbles (`0x5000` for the .5 of 2.5).
    #[must_use]
    pub const fn minor_bits(self) -> u16 {
        let [_, _, high, low] = self.0.to_be_bytes();
        u16::from_be_bytes([high, low])
    }
}

/// `major.minor` as the spec writes versions: `2.5`, `0.5`, `3.0`.
impl fmt::Display for Version16Dot16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.", self.major())?;
        let mut minor = self.minor_bits();
        if minor == 0 {
            return write!(f, "0");
        }
        // Digits from the highest nibble down, stopping once only zero
        // nibbles remain: 0x5000 is "5", 0x1200 is "12".
        while minor != 0 {
            write!(f, "{:x}", minor >> 12)?;
            minor <<= 4;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Version16Dot16;

    /// The spec's own examples: `post`'s four versions, `maxp` 0.5 and
    /// `vhea` 1.1.
    #[test]
    fn reads_the_spec_examples() {
        for (bits, major, shown) in [
            (0x0001_0000, 1, "1.0"),
            (0x0002_0000, 2, "2.0"),
            (0x0002_5000, 2, "2.5"),
            (0x0003_0000, 3, "3.0"),
            (0x0000_5000, 0, "0.5"),
            (0x0001_1000, 1, "1.1"),
        ] {
            let version = Version16Dot16::from_be_bytes(u32::to_be_bytes(bits));
            assert_eq!(version.major(), major);
            assert_eq!(version.to_string(), shown);
        }
    }

    #[test]
    fn orders_as_versions() {
        let versions =
            [0x0001_0000, 0x0002_0000, 0x0002_5000, 0x0003_0000].map(Version16Dot16::from_bits);
        assert!(versions.is_sorted());
    }
}
