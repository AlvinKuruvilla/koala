//! CSS Computed Style representation and value parsing
//!
//! This module implements CSS value types and computed style representation per:
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//! - [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)
//! - [CSS Display Module Level 3](https://www.w3.org/TR/css-display-3/)
//! - [CSS Writing Modes Level 4](https://www.w3.org/TR/css-writing-modes-4/)
//! - [CSS Logical Properties Level 1](https://drafts.csswg.org/css-logical-1/)

pub mod computed;
mod display;
pub mod substitute;
mod values;
mod writing_mode;

// Re-export all public types
pub use computed::ComputedStyle;
pub use display::{DisplayValue, InnerDisplayType, OuterDisplayType};
pub use values::{AutoLength, BorderValue, BoxShadow, ColorValue, DEFAULT_FONT_SIZE_PX, LengthValue};
pub use writing_mode::{PhysicalSide, WritingMode};
