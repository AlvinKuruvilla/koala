//! CSS Value types and parsing
//!
//! - [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/)
//! - [CSS Color Level 4](https://www.w3.org/TR/css-color-4/)
//! - [CSS Backgrounds and Borders Level 3](https://www.w3.org/TR/css-backgrounds-3/)
//! - [CSS Fonts Module Level 4](https://www.w3.org/TR/css-fonts-4/)
//! - [CSS Text Module Level 3](https://www.w3.org/TR/css-text-3/)
//! - [CSS Text Decoration Level 3](https://www.w3.org/TR/css-text-decoration-3/)
//! - [CSS 2.1 Visual Formatting Model](https://www.w3.org/TR/CSS2/visuren.html)

mod border;
mod color;
mod float;
mod font;
mod length;
mod position;
mod text;

pub use border::{BorderRadius, BorderValue, BoxShadow};
pub use color::{ColorValue, parse_color_value, parse_single_color};
pub use float::{ClearSide, FloatSide};
pub use font::{FontStyle, parse_font_family, parse_font_weight, parse_line_height};
pub use length::{
    AutoLength, DEFAULT_FONT_SIZE_PX, LengthValue, parse_auto_length_value, parse_length_value,
    parse_single_auto_length, parse_single_length,
};
pub use position::PositionType;
pub use text::{TextAlign, TextDecorationLine};
