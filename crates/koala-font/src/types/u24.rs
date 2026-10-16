//! `uint24`.

/// `uint24`: "24-bit unsigned integer."
///
/// Kept in a `u32` whose top byte is always zero. The two ways to make
/// one, [`U24::from_be_bytes`] and [`TryFrom<u32>`], both keep it so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct U24(u32);

impl U24 {
    /// The smallest `uint24`, 0.
    pub const MIN: Self = Self(0);

    /// The largest `uint24`, 2^24 - 1.
    pub const MAX: Self = Self(0x00FF_FFFF);

    /// The value of three bytes in big-endian order, the byte order
    /// OpenType stores every integer in.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 3]) -> Self {
        let [high, middle, low] = bytes;
        Self(u32::from_be_bytes([0, high, middle, low]))
    }

    /// This value as three bytes in big-endian order.
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 3] {
        let [_, high, middle, low] = self.0.to_be_bytes();
        [high, middle, low]
    }

    /// This value as a `u32`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<U24> for u32 {
    fn from(value: U24) -> Self {
        value.0
    }
}

impl TryFrom<u32> for U24 {
    type Error = U24OutOfRange;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value <= Self::MAX.0 {
            Ok(Self(value))
        } else {
            Err(U24OutOfRange(value))
        }
    }
}

/// A `u32` too large to be a `uint24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0} does not fit in 24 bits (the largest uint24 is 16777215)")]
pub struct U24OutOfRange(pub u32);

#[cfg(test)]
mod tests {
    use super::{U24, U24OutOfRange};
    use quickcheck::quickcheck;

    #[test]
    fn bytes_round_trip_at_the_edges() {
        for (bytes, value) in [
            ([0x00, 0x00, 0x00], 0),
            ([0x00, 0x00, 0x01], 1),
            ([0x01, 0x02, 0x03], 0x0001_0203),
            ([0xFF, 0xFF, 0xFF], 0x00FF_FFFF),
        ] {
            let decoded = U24::from_be_bytes(bytes);
            assert_eq!(decoded.get(), value);
            assert_eq!(decoded.to_be_bytes(), bytes);
        }
    }

    #[test]
    fn try_from_rejects_values_past_24_bits() {
        assert_eq!(U24::try_from(0x00FF_FFFF), Ok(U24::MAX));
        assert_eq!(U24::try_from(0x0100_0000), Err(U24OutOfRange(0x0100_0000)));
        assert_eq!(U24::try_from(u32::MAX), Err(U24OutOfRange(u32::MAX)));
    }

    quickcheck! {
        fn every_24_bit_value_survives_bytes(value: u32) -> bool {
            let value = value & 0x00FF_FFFF;
            let u24 = U24::try_from(value).expect("masked to 24 bits");
            U24::from_be_bytes(u24.to_be_bytes()) == u24 && u32::from(u24) == value
        }
    }
}
