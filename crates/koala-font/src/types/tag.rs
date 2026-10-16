//! `Tag`.

use std::fmt;

/// `Tag`: "Array of four uint8s (length = 32 bits) used to identify a
/// table, design-variation axis, script, language system, feature, or
/// baseline", such as `head` or `OS/2`.
///
/// Only a valid tag can be made. The spec's rule:
///
/// "Each byte within the array must have a value in the range 0x20 to
/// 0x7E. ... It must have one to four non-space characters, padded with
/// trailing spaces (byte value 0x20). A space character must not be
/// followed by a non-space character."
///
/// [`Tag::new`] checks it at run time, for bytes read from a file.
/// [`Tag::from_literal`] checks it at compile time, for the tags code
/// names:
///
/// ```
/// # use koala_font::Tag;
/// const CVT: Tag = Tag::from_literal(*b"cvt ");
/// assert_eq!(CVT.to_string(), "cvt ");
/// ```
///
/// A literal that breaks the rule does not compile. Here a space is
/// followed by a non-space:
///
/// ```compile_fail
/// # use koala_font::Tag;
/// const BAD: Tag = Tag::from_literal(*b"he d");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag([u8; 4]);

impl Tag {
    /// The tag spelled by `bytes`, if they follow the spec's rule.
    ///
    /// # Errors
    ///
    /// [`InvalidTag`] naming the first rule `bytes` break.
    pub const fn new(bytes: [u8; 4]) -> Result<Self, InvalidTag> {
        // The rule allows exactly four shapes: one to four characters
        // (printable and not a space, 0x21 to 0x7E), then spaces to fill
        // the four bytes. The first arm is four or three characters, the
        // second two or one. Matching the shapes directly checks every
        // byte with no loop; which rule a non-match breaks is worked out
        // only then.
        match bytes {
            [0x21..=0x7E, 0x21..=0x7E, 0x21..=0x7E, 0x21..=0x7E | b' ']
            | [0x21..=0x7E, 0x21..=0x7E | b' ', b' ', b' '] => Ok(Self(bytes)),
            _ => Err(InvalidTag::diagnose(bytes)),
        }
    }

    /// The tag spelled by `bytes`, for a literal. Called in a `const`,
    /// a literal that breaks the spec's rule is a compile error.
    ///
    /// # Panics
    ///
    /// If `bytes` break the rule; see [`Tag::new`].
    #[must_use]
    pub const fn from_literal(bytes: [u8; 4]) -> Self {
        match Self::new(bytes) {
            Ok(tag) => tag,
            Err(_) => panic!("not a valid OpenType tag"),
        }
    }

    /// The tag's four bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 4] {
        self.0
    }
}

/// The four characters, trailing spaces included: `cvt ` stays `cvt `.
impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Every byte is printable ASCII, so each is one character.
        for &byte in &self.0 {
            write!(f, "{}", char::from(byte))?;
        }
        Ok(())
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag(\"{self}\")")
    }
}

/// Which part of the spec's rule for tags some bytes break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidTag {
    /// "Each byte within the array must have a value in the range 0x20 to
    /// 0x7E."
    #[error("byte {index} of the tag is {byte:#04x}, outside 0x20 to 0x7e")]
    ByteOutOfRange {
        /// Where the byte is.
        index: usize,
        /// The byte.
        byte: u8,
    },
    /// "A space character must not be followed by a non-space character."
    #[error("byte {index} of the tag follows a space but is not one")]
    SpaceBeforeCharacter {
        /// Where the non-space is.
        index: usize,
    },
    /// "It must have one to four non-space characters": all four are
    /// spaces.
    #[error("the tag is all spaces")]
    NoCharacters,
}

impl InvalidTag {
    /// Which rule `bytes`, already known not to be a valid tag, break
    /// first. Runs only for malformed tags.
    const fn diagnose(bytes: [u8; 4]) -> Self {
        // A slice pattern walks the bytes: `.get()` is not available in a
        // `const fn`, and indexing is denied crate-wide.
        let mut rest: &[u8] = &bytes;
        let mut index = 0;
        let mut seen_space = false;
        while let [byte, tail @ ..] = rest {
            match *byte {
                byte @ (..0x20 | 0x7F..) => return Self::ByteOutOfRange { index, byte },
                b' ' => seen_space = true,
                _ if seen_space => return Self::SpaceBeforeCharacter { index },
                _ => {}
            }
            rest = tail;
            index += 1;
        }
        // Every byte is in range and no character follows a space, yet the
        // bytes did not match: the only shape left is four spaces.
        Self::NoCharacters
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidTag, Tag};

    #[test]
    fn accepts_tags_the_spec_allows() {
        for bytes in [*b"head", *b"OS/2", *b"cvt ", *b"a   ", *b"~!~!"] {
            assert_eq!(Tag::new(bytes).map(Tag::as_bytes), Ok(bytes));
        }
    }

    #[test]
    fn rejects_each_broken_rule() {
        assert_eq!(
            Tag::new([b'h', 0x1F, b'a', b'd']),
            Err(InvalidTag::ByteOutOfRange {
                index: 1,
                byte: 0x1F
            })
        );
        assert_eq!(
            Tag::new([b'h', b'e', b'a', 0x7F]),
            Err(InvalidTag::ByteOutOfRange {
                index: 3,
                byte: 0x7F
            })
        );
        assert_eq!(
            Tag::new(*b"he d"),
            Err(InvalidTag::SpaceBeforeCharacter { index: 3 })
        );
        assert_eq!(
            Tag::new(*b" abc"),
            Err(InvalidTag::SpaceBeforeCharacter { index: 1 })
        );
        assert_eq!(Tag::new(*b"    "), Err(InvalidTag::NoCharacters));
    }

    /// The four-shape match agrees with the rule written out the long way,
    /// on every combination of bytes taken from the edges of each class
    /// (control, space, first and last character, past the range): 8^4 =
    /// 4096 cases.
    #[test]
    fn match_agrees_with_the_rule() {
        let samples = [0x00, 0x1F, 0x20, 0x21, 0x41, 0x7E, 0x7F, 0xFF];
        for a in samples {
            for b in samples {
                for c in samples {
                    for d in samples {
                        let bytes = [a, b, c, d];
                        let in_range = bytes.iter().all(|&x| (0x20..=0x7E).contains(&x));
                        let characters = bytes.iter().take_while(|&&x| x != b' ').count();
                        let padded = bytes.iter().skip(characters).all(|&x| x == b' ');
                        let valid = in_range && characters >= 1 && padded;
                        assert_eq!(Tag::new(bytes).is_ok(), valid, "{bytes:02x?}");
                    }
                }
            }
        }
    }

    #[test]
    fn displays_its_characters() {
        const OS2: Tag = Tag::from_literal(*b"OS/2");
        assert_eq!(OS2.to_string(), "OS/2");
        assert_eq!(format!("{OS2:?}"), "Tag(\"OS/2\")");
    }
}
