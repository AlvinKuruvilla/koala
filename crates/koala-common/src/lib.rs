//! Common utilities for the Koala renderer.
//!
//! This crate provides shared infrastructure used by all renderer components:
//! - **Warning System** - colored terminal output for unsupported features
//! - **URL Resolution** - resolve relative URLs against a base URL
//! - **Image Types** - shared image data structures
//! - **Network Utilities** - HTTP fetch helpers

/// Decoded image data types shared across renderer components.
pub mod image;
/// HTTP fetch utilities for document, stylesheet, and image loading.
pub mod net;
/// URL resolution utilities.
pub mod url;
/// Warning system with colored terminal output.
pub mod warning;
