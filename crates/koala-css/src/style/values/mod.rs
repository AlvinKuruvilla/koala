//! CSS Value types and parsing
//!
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//! - [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)
//! - [CSS Backgrounds and Borders Level 3](https://www.w3.org/TR/css-backgrounds-3/)
//! - [CSS Fonts Module Level 4](https://www.w3.org/TR/css-fonts-4/)

mod border;
mod color;
mod font;
mod length;

pub use border::{BorderValue, BoxShadow};
pub use color::{ColorValue, parse_color_value, parse_single_color};
pub use font::{parse_font_family, parse_font_weight, parse_line_height};
pub use length::{
    AutoLength, DEFAULT_FONT_SIZE_PX, LengthValue, parse_auto_length_value, parse_length_value,
    parse_single_auto_length, parse_single_length,
};
