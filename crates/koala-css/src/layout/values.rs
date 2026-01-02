//! Unresolved and auto value types for CSS layout.
//!
//! [§ 6 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)

use crate::style::{AutoLength, LengthValue};

use super::box_model::{EdgeSizes, Rect};

/// [§ 6 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
///
/// "The computed value is the result of resolving the specified value...
/// as far as possible without laying out the document."
///
/// [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
///
/// "The computed value of a <length> where... the viewport size is needed
/// to resolve the value, is the specified value."
///
/// Edge sizes storing unresolved length values.
/// These are resolved to pixels during layout when viewport is available.
#[derive(Debug, Clone, Default)]
pub struct UnresolvedEdgeSizes {
    /// Top edge (unresolved).
    pub top: Option<LengthValue>,
    /// Right edge (unresolved).
    pub right: Option<LengthValue>,
    /// Bottom edge (unresolved).
    pub bottom: Option<LengthValue>,
    /// Left edge (unresolved).
    pub left: Option<LengthValue>,
}

impl UnresolvedEdgeSizes {
    /// [§ 6.1 Used Values](https://www.w3.org/TR/css-cascade-4/#used)
    ///
    /// "The used value is the result of taking the computed value and
    /// completing any remaining calculations to make it the absolute
    /// theoretical value used in the layout of the document."
    ///
    /// Resolve to concrete pixel values using viewport dimensions.
    pub fn resolve(&self, viewport: Rect) -> EdgeSizes {
        EdgeSizes {
            top: self
                .top
                .as_ref()
                .map(|l| {
                    l.to_px_with_viewport(viewport.width as f64, viewport.height as f64) as f32
                })
                .unwrap_or(0.0),
            right: self
                .right
                .as_ref()
                .map(|l| {
                    l.to_px_with_viewport(viewport.width as f64, viewport.height as f64) as f32
                })
                .unwrap_or(0.0),
            bottom: self
                .bottom
                .as_ref()
                .map(|l| {
                    l.to_px_with_viewport(viewport.width as f64, viewport.height as f64) as f32
                })
                .unwrap_or(0.0),
            left: self
                .left
                .as_ref()
                .map(|l| {
                    l.to_px_with_viewport(viewport.width as f64, viewport.height as f64) as f32
                })
                .unwrap_or(0.0),
        }
    }
}

/// [§ 6 Computed Values](https://www.w3.org/TR/css-cascade-4/#computed)
///
/// Edge sizes storing unresolved auto-or-length values.
/// Used for margins where 'auto' has special meaning (centering).
#[derive(Debug, Clone, Default)]
pub struct UnresolvedAutoEdgeSizes {
    /// Top edge (unresolved, can be auto).
    pub top: Option<AutoLength>,
    /// Right edge (unresolved, can be auto).
    pub right: Option<AutoLength>,
    /// Bottom edge (unresolved, can be auto).
    pub bottom: Option<AutoLength>,
    /// Left edge (unresolved, can be auto).
    pub left: Option<AutoLength>,
}

impl UnresolvedAutoEdgeSizes {
    /// [§ 6.1 Used Values](https://www.w3.org/TR/css-cascade-4/#used)
    ///
    /// Resolve to AutoOr values using viewport dimensions.
    /// 'auto' is preserved for later resolution during width/margin calculation.
    pub fn resolve(&self, viewport: Rect) -> AutoEdgeSizes {
        AutoEdgeSizes {
            top: self
                .top
                .as_ref()
                .map(|al| Self::resolve_auto_length(al, viewport))
                .unwrap_or(AutoOr::Length(0.0)),
            right: self
                .right
                .as_ref()
                .map(|al| Self::resolve_auto_length(al, viewport))
                .unwrap_or(AutoOr::Length(0.0)),
            bottom: self
                .bottom
                .as_ref()
                .map(|al| Self::resolve_auto_length(al, viewport))
                .unwrap_or(AutoOr::Length(0.0)),
            left: self
                .left
                .as_ref()
                .map(|al| Self::resolve_auto_length(al, viewport))
                .unwrap_or(AutoOr::Length(0.0)),
        }
    }

    /// [§ 6.1 Used Values](https://www.w3.org/TR/css-cascade-4/#used)
    ///
    /// Resolve a single AutoLength to AutoOr using viewport dimensions.
    pub fn resolve_auto_length(al: &AutoLength, viewport: Rect) -> AutoOr {
        match al {
            // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
            //
            // 'auto' is preserved - it will be resolved during width calculation.
            AutoLength::Auto => AutoOr::Auto,
            // Resolve length using viewport for vw/vh units.
            AutoLength::Length(len) => AutoOr::Length(
                len.to_px_with_viewport(viewport.width as f64, viewport.height as f64) as f32,
            ),
        }
    }
}

/// [§ 4.4 Automatic values](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
///
/// "Some properties can take the keyword 'auto' as a value. This keyword
/// allows the user agent to compute the value based on other properties."
///
/// This enum represents a value that can either be 'auto' or a specific length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutoOr {
    /// The value is 'auto' and must be resolved during layout.
    Auto,
    /// The value is a specific length in pixels.
    Length(f32),
}

impl Default for AutoOr {
    fn default() -> Self {
        AutoOr::Auto
    }
}

impl AutoOr {
    /// Check if the value is 'auto'.
    pub fn is_auto(&self) -> bool {
        matches!(self, AutoOr::Auto)
    }

    /// Get the length value, or a default if 'auto'.
    pub fn to_px_or(&self, default: f32) -> f32 {
        match self {
            AutoOr::Length(v) => *v,
            AutoOr::Auto => default,
        }
    }
}

/// [§ 8 Box model](https://www.w3.org/TR/CSS2/box.html)
///
/// Edge values where each side can be 'auto' or a specific length.
/// Used for margins where 'auto' has special meaning (centering).
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoEdgeSizes {
    /// Top edge value.
    pub top: AutoOr,
    /// Right edge value.
    pub right: AutoOr,
    /// Bottom edge value.
    pub bottom: AutoOr,
    /// Left edge value.
    pub left: AutoOr,
}
