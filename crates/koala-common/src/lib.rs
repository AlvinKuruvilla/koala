//! Common utilities for the Koala renderer.
//!
//! This crate provides shared infrastructure used by all renderer components:
//! - **Warning System** - colored terminal output for unsupported features
//! - **URL Resolution** - resolve relative URLs against a base URL
//! - **Image Types** - shared image data structures

/// Counting global allocator for heap accounting in bench / dev builds.
pub mod alloc_count;
/// Decoded image data types shared across renderer components.
pub mod image;
/// URL resolution utilities.
pub mod url;
/// Warning system with colored terminal output.
pub mod warning;
