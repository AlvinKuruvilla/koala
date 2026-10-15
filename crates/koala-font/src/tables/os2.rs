//! [`OS/2` — OS/2 and Windows metrics table](https://learn.microsoft.com/en-us/typography/opentype/spec/os2): the typographic
//! ascender, descender and line gap that CSS prefers, and the strikeout
//! line.
//!
//! A version 0 table may stop early: the spec warns that Apple's version
//! 0 "stops at the usLastCharIndex field and does not include the last
//! five fields", so the table's length decides which fields exist.
