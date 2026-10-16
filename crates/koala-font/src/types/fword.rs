//! `FWORD`.

/// `FWORD`: "int16 that describes a quantity in font design units."
///
/// A distance in font units is not a distance in pixels until it is
/// scaled, so the value comes out only through [`FWord::scaled`] or the
/// explicitly named [`FWord::font_units`]: a call site always says which
/// unit it works in.
///
/// There is no arithmetic on font units. Sums such as ascent minus
/// descent are taken after scaling, in `f32`, where they cannot overflow
/// as `32767 - (-32768)` would in an `i16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FWord(i16);

impl FWord {
    /// The value of two bytes in big-endian order, the byte order
    /// OpenType stores every integer in.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self(i16::from_be_bytes(bytes))
    }

    /// This quantity in pixels, given the pixels per font unit
    /// (`Font::scale_factor`).
    #[must_use]
    pub fn scaled(self, factor: f32) -> f32 {
        f32::from(self.0) * factor
    }

    /// This quantity in font units, unscaled.
    #[must_use]
    pub const fn font_units(self) -> i16 {
        self.0
    }
}

impl From<i16> for FWord {
    fn from(font_units: i16) -> Self {
        Self(font_units)
    }
}

#[cfg(test)]
mod tests {
    use super::FWord;

    #[test]
    fn reads_big_endian_two_s_complement() {
        assert_eq!(FWord::from_be_bytes([0x07, 0xD0]).font_units(), 2000);
        assert_eq!(FWord::from_be_bytes([0xFE, 0x0C]).font_units(), -500);
        assert_eq!(FWord::from_be_bytes([0x80, 0x00]).font_units(), i16::MIN);
    }

    /// 1638 units at 16px in a 2048-unit em: 1638 * 16 / 2048 = 12.796875,
    /// exact in `f32`.
    #[test]
    fn scales_by_the_factor() {
        let factor = 16.0 / 2048.0;
        assert_eq!(FWord::from(1638).scaled(factor), 12.796_875);
        assert_eq!(FWord::from(-410).scaled(factor), -3.203_125);
    }
}
