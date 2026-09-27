//! Common utilities for the Koala renderer.
//!
//! This crate provides shared infrastructure used by all renderer components:
//! - **Diagnostics** - problems met while loading a page, collected per load
//! - **URL Resolution** - resolve relative URLs against a base URL
//! - **Image Types** - shared image data structures

/// Problems met while loading a page, collected per load.
pub mod diagnostics;
/// Counting global allocator for heap accounting in bench / dev builds.
pub mod alloc_count;
/// Decoded image data types shared across renderer components.
pub mod image;
/// URL resolution utilities.
pub mod url;
/// The process-wide quiet flag that silences diagnostic output.
pub mod warning;
