//! `F2DOT14`.

/// `F2DOT14`: "16-bit signed fixed number with the low 14 bits of
/// fraction (2.14)."
///
/// "The F2DOT14 format consists of a signed, 2's complement integer and
/// an unsigned fraction. To compute the actual value, take the integer
/// and add the fraction." Equivalently, the 16-bit integer divided by
/// 16384, which covers -2.0 to just under 2.0. Composite glyphs use it for
/// their scale factors, and variable fonts for axis coordinates.
///
/// The value comes out through [`F2Dot14::to_f32`], which is exact: 16
/// significant bits fit `f32`'s 24-bit mantissa. The raw integer is
/// reachable only through [`F2Dot14::to_bits`] and [`F2Dot14::from_bits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct F2Dot14(i16);

impl F2Dot14 {
    /// The value of two bytes in big-endian order, the byte order
    /// OpenType stores every integer in.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self(i16::from_be_bytes(bytes))
    }

    /// The number this represents.
    #[must_use]
    pub fn to_f32(self) -> f32 {
        f32::from(self.0) / 16384.0
    }

    /// The `F2Dot14` whose stored 16-bit integer is `bits`.
    #[must_use]
    pub const fn from_bits(bits: i16) -> Self {
        Self(bits)
    }

    /// The stored 16-bit integer: 16384 times the number.
    #[must_use]
    pub const fn to_bits(self) -> i16 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::F2Dot14;

    /// The spec's table of 2.14 examples. It prints rounded decimals
    /// (1.999939); the exact values are its integer plus its fraction.
    #[test]
    fn decodes_the_spec_examples() {
        for (bytes, value) in [
            ([0x7F, 0xFF], 1.0 + 16383.0 / 16384.0),
            ([0x70, 0x00], 1.75),
            ([0x00, 0x01], 1.0 / 16384.0),
            ([0x00, 0x00], 0.0),
            ([0xFF, 0xFF], -1.0 + 16383.0 / 16384.0),
            ([0x80, 0x00], -2.0),
        ] {
            assert_eq!(F2Dot14::from_be_bytes(bytes).to_f32(), value);
        }
    }
}
