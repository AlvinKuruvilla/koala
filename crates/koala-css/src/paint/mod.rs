//! CSS Painting
//!
//! [CSS 2.1 Appendix E - Elaborate description of Stacking Contexts](https://www.w3.org/TR/CSS2/zindex.html)
//!
//! This module implements the painting phase, which converts a layout tree into
//! a display list of drawing commands. The display list can then be executed by
//! any renderer (software, GPU, etc.).
//!
//! # Architecture
//!
//! The painting phase is separate from layout and rendering:
//!
//! ```text
//! Style → Layout → Paint → Render
//!                    ↓
//!              DisplayList
//! ```
//!
//! This separation allows:
//! - Different renderers (software, GPU, print) to share painting logic
//! - Caching of display lists for unchanged content
//! - Correct z-order painting per CSS 2.1 Appendix E

mod display_list;
mod painter;

pub use display_list::{DisplayCommand, DisplayList};
pub use painter::Painter;
