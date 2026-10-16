//! `Fixed`.

/// `Fixed`: "32-bit signed fixed-point number (16.16)".
///
/// The top 16 bits are the signed whole part and the bottom 16 count
/// 65536ths, so the value is the 32-bit integer divided by 65536.
/// `post.italicAngle` of -12.5 degrees is stored as `0xFFF3_8000`, which
/// is -819200 read as an integer.
///
/// The value comes out through [`Fixed::to_f64`]. An `f32` would not do:
/// its 24-bit mantissa cannot hold all 32 significant bits, while every
/// `Fixed` is exact in an `f64`. The raw integer is reachable only through
/// [`Fixed::to_bits`] and [`Fixed::from_bits`], named as `f32`'s are.
///
/// Not for version fields. `maxp`, `post` and `vhea` declare their
/// versions `Version16Dot16`, which packs the minor version as decimal
/// digits in the high nibbles: `post` version 2.5 is `0x0002_5000`, which
/// as a `Fixed` would read 2.3125.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Fixed(i32);

impl Fixed {
    /// The value of four bytes in big-endian order, the byte order
    /// OpenType stores every integer in.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 4]) -> Self {
        Self(i32::from_be_bytes(bytes))
    }

    /// The number this represents.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        f64::from(self.0) / 65536.0
    }

    /// The `Fixed` whose stored 32-bit integer is `bits`.
    #[must_use]
    pub const fn from_bits(bits: i32) -> Self {
        Self(bits)
    }

    /// The stored 32-bit integer: 65536 times the number.
    #[must_use]
    pub const fn to_bits(self) -> i32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Fixed;

    #[test]
    fn decodes_whole_and_fractional_parts() {
        for (bytes, value) in [
            ([0x00, 0x01, 0x00, 0x00], 1.0),
            ([0x00, 0x02, 0x80, 0x00], 2.5),
            ([0xFF, 0xF3, 0x80, 0x00], -12.5),
            ([0x00, 0x00, 0x00, 0x01], 1.0 / 65536.0),
        ] {
            assert_eq!(Fixed::from_be_bytes(bytes).to_f64(), value);
        }
    }

    /// The extremes need all 32 bits; `f64` holds them exactly.
    #[test]
    fn extremes_are_exact() {
        assert_eq!(
            Fixed::from_bits(i32::MAX).to_f64(),
            32767.0 + 65535.0 / 65536.0
        );
        assert_eq!(Fixed::from_bits(i32::MIN).to_f64(), -32768.0);
    }

    /// `post` version 2.5, a `Version16Dot16`, misread as a `Fixed`.
    #[test]
    fn version_fields_are_not_fixed() {
        assert_eq!(Fixed::from_bits(0x0002_5000).to_f64(), 2.3125);
    }
}
