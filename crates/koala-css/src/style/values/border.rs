//! CSS Border values
//!
//! [CSS Backgrounds and Borders Level 3](https://www.w3.org/TR/css-backgrounds-3/)

use serde::Serialize;

use super::color::ColorValue;
use super::length::LengthValue;

/// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
///
/// "The 'box-shadow' property attaches one or more drop-shadows to the box."
///
/// `<shadow> = inset? && <length>{2,4} && <color>?`
///
/// - 2 required lengths: offset-x, offset-y
/// - 2 optional lengths: blur-radius (default 0, >= 0), spread-radius (default 0)
/// - `inset` keyword: inner shadow (optional)
/// - color defaults to `currentColor`
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoxShadow {
    /// Horizontal offset. Positive = right.
    pub offset_x: f32,
    /// Vertical offset. Positive = down.
    pub offset_y: f32,
    /// Blur radius. Must be >= 0. Default 0.
    pub blur_radius: f32,
    /// Spread radius. Default 0.
    pub spread_radius: f32,
    /// Shadow color. Defaults to the element's `color` (currentColor).
    pub color: ColorValue,
    /// If true, shadow is drawn inside the box (inset shadow).
    pub inset: bool,
}

/// [§ 5 'border-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-radius)
///
/// "The two length or percentage values of the 'border-*-radius' properties
/// define the radii of a quarter ellipse that defines the shape of the corner
/// of the outer border edge."
///
/// This implementation supports circular corners only (single radius per corner).
/// Elliptical radii (horizontal / vertical) are not yet supported.
///
/// Initial value: 0 (no rounding)
/// Inherited: no
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct BorderRadius {
    /// [§ 5.1 'border-top-left-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-top-left-radius)
    pub top_left: f32,
    /// [§ 5.2 'border-top-right-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-top-right-radius)
    pub top_right: f32,
    /// [§ 5.3 'border-bottom-right-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-bottom-right-radius)
    pub bottom_right: f32,
    /// [§ 5.4 'border-bottom-left-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-bottom-left-radius)
    pub bottom_left: f32,
}

/// [§ 4 Borders](https://www.w3.org/TR/css-backgrounds-3/#borders)
///
/// Border value representing width, style, and color.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BorderValue {
    /// [§ 4.3 'border-width'](https://www.w3.org/TR/css-backgrounds-3/#border-width)
    pub width: LengthValue,
    /// [§ 4.2 'border-style'](https://www.w3.org/TR/css-backgrounds-3/#border-style)
    pub style: String,
    /// [§ 4.1 'border-color'](https://www.w3.org/TR/css-backgrounds-3/#border-color)
    pub color: ColorValue,
}

impl BorderValue {
    /// The width that layout and painting use.
    ///
    /// [§ 3.3 Line Thickness](https://www.w3.org/TR/css-backgrounds-3/#border-width)
    ///
    /// "Computed value: absolute length, snapped as a border width; zero if
    /// the border style is none or hidden"
    ///
    /// Read the width through this, never through `width` directly: the
    /// style and width can come from separate declarations in any order
    /// (`border-width: 5px; border-style: none`), so the zeroing cannot
    /// happen when either is parsed.
    #[must_use]
    pub fn computed_width(&self) -> LengthValue {
        if matches!(self.style.as_str(), "none" | "hidden") {
            LengthValue::Px(0.0)
        } else {
            self.width
        }
    }
}
