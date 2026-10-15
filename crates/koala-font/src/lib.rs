//! OpenType font parsing for Koala.
//!
//! `koala-font` reads OpenType and TrueType font files (and faces of
//! font collections) from bytes the caller supplies; it does no I/O.
//! It replaces `fontdue` one slice at a time, and `fontdue` keeps
//! rasterizing glyphs until this crate can.
//!
//! The first slice reads the font-wide metrics CSS needs to place text
//! and its decorations: the units per em, the ascent, descent and line
//! gap, and the underline and strikeout lines. What to read and why is
//! in `project-memory/koala-font-metrics-reading-guide.md`.
