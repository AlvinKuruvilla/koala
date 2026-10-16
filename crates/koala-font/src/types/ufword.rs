//! `UFWORD`.

/// `UFWORD`: "uint16 that describes a quantity in font design units."
///
/// The unsigned twin of [`FWord`](super::fword::FWord), with the same
/// rule: the value comes out only scaled, or through the explicitly named
/// [`UFWord::font_units`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct UFWord(u16);

impl UFWord {
    /// The value of two bytes in big-endian order, the byte order
    /// OpenType stores every integer in.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self(u16::from_be_bytes(bytes))
    }

    /// This quantity in pixels, given the pixels per font unit
    /// (`Font::scale_factor`).
    #[must_use]
    pub fn scaled(self, factor: f32) -> f32 {
        f32::from(self.0) * factor
    }

    /// This quantity in font units, unscaled.
    #[must_use]
    pub const fn font_units(self) -> u16 {
        self.0
    }
}

impl From<u16> for UFWord {
    fn from(font_units: u16) -> Self {
        Self(font_units)
    }
}

#[cfg(test)]
mod tests {
    use super::UFWord;

    #[test]
    fn reads_big_endian_unsigned() {
        assert_eq!(UFWord::from_be_bytes([0x07, 0xD0]).font_units(), 2000);
        assert_eq!(UFWord::from_be_bytes([0xFE, 0x0C]).font_units(), 65036);
    }

    /// 2048 units at 16px in a 2048-unit em is exactly one em.
    #[test]
    fn scales_by_the_factor() {
        assert_eq!(UFWord::from(2048).scaled(16.0 / 2048.0), 16.0);
    }
}
