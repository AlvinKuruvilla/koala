//! CSS Box Model types.
//!
//! [CSS Box Model Module Level 3](https://www.w3.org/TR/css-box-3/)

/// [§ 3. The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
///
/// "Each box has a content area and optional surrounding padding, border,
/// and margin areas."
#[derive(Debug, Clone, Default)]
pub struct BoxDimensions {
    /// Content area dimensions
    pub content: Rect,
    /// Padding edge (content + padding)
    pub padding: EdgeSizes,
    /// Border edge (content + padding + border)
    pub border: EdgeSizes,
    /// Margin edge (content + padding + border + margin)
    pub margin: EdgeSizes,
}

/// A rectangle positioned in 2D space.
///
/// [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    /// Horizontal position of the top-left corner.
    pub x: f32,
    /// Vertical position of the top-left corner.
    pub y: f32,
    /// Width of the rectangle.
    pub width: f32,
    /// Height of the rectangle.
    pub height: f32,
}

/// Edge sizes for padding, border, or margin.
///
/// [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
#[derive(Debug, Clone, Copy, Default)]
pub struct EdgeSizes {
    /// Top edge size.
    pub top: f32,
    /// Right edge size.
    pub right: f32,
    /// Bottom edge size.
    pub bottom: f32,
    /// Left edge size.
    pub left: f32,
}

impl BoxDimensions {
    // [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
    //
    // "Each box has a content area and optional surrounding padding, border,
    // and margin areas... These areas are determined by their respective edges."
    //
    // ┌─────────────────────────────────────────┐
    // │              margin-top                 │
    // │   ┌─────────────────────────────────┐   │
    // │   │          border-top             │   │
    // │   │   ┌─────────────────────────┐   │   │
    // │   │   │      padding-top        │   │   │
    // │   │   │   ┌─────────────────┐   │   │   │
    // │ m │ b │ p │                 │ p │ b │ m │
    // │ a │ o │ a │     CONTENT     │ a │ o │ a │
    // │ r │ r │ d │                 │ d │ r │ r │
    // │ g │ d │ d │                 │ d │ d │ g │
    // │ i │ e │ i │                 │ i │ e │ i │
    // │ n │ r │ n │                 │ n │ r │ n │
    // │   │   │ g │                 │ g │   │   │
    // │   │   │   └─────────────────┘   │   │   │
    // │   │   │      padding-bottom     │   │   │
    // │   │   └─────────────────────────┘   │   │
    // │   │          border-bottom          │   │
    // │   └─────────────────────────────────┘   │
    // │              margin-bottom              │
    // └─────────────────────────────────────────┘
    //
    // The boxes from innermost to outermost:
    //   1. Content box  - the actual content (text, images, etc.)
    //   2. Padding box  - content + padding
    //   3. Border box   - content + padding + border
    //   4. Margin box   - content + padding + border + margin (outermost)

    /// [§ 3.1 Margins](https://www.w3.org/TR/css-box-3/#margins)
    ///
    /// "The margin box is the outermost box, and contains all four areas."
    ///
    /// # Formulas
    ///
    /// To find the margin box from the content box, we expand outward through
    /// all three layers (padding, border, margin):
    ///
    /// ```text
    /// x = content.x - padding.left - border.left - margin.left
    /// y = content.y - padding.top - border.top - margin.top
    ///
    /// width = content.width
    ///       + padding.left + padding.right
    ///       + border.left + border.right
    ///       + margin.left + margin.right
    ///
    /// height = content.height
    ///        + padding.top + padding.bottom
    ///        + border.top + border.bottom
    ///        + margin.top + margin.bottom
    /// ```
    #[must_use]
    pub fn margin_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left - self.border.left - self.margin.left,
            y: self.content.y - self.padding.top - self.border.top - self.margin.top,
            width: self.content.width
                + self.padding.left
                + self.padding.right
                + self.border.left
                + self.border.right
                + self.margin.left
                + self.margin.right,
            height: self.content.height
                + self.padding.top
                + self.padding.bottom
                + self.border.top
                + self.border.bottom
                + self.margin.top
                + self.margin.bottom,
        }
    }

    /// [§ 3.2 Padding](https://www.w3.org/TR/css-box-3/#paddings)
    ///
    /// "The padding box contains both the content and padding areas."
    ///
    /// # Formulas
    ///
    /// To find the padding box from the content box, we expand outward through
    /// only the padding layer:
    ///
    /// ```text
    /// x = content.x - padding.left
    /// y = content.y - padding.top
    ///
    /// width = content.width + padding.left + padding.right
    /// height = content.height + padding.top + padding.bottom
    /// ```
    #[must_use]
    pub fn padding_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left,
            y: self.content.y - self.padding.top,
            width: self.content.width + self.padding.left + self.padding.right,
            height: self.content.height + self.padding.top + self.padding.bottom,
        }
    }

    /// [§ 3.3 Borders](https://www.w3.org/TR/css-box-3/#borders)
    ///
    /// "The border box contains content, padding, and border areas."
    ///
    /// # Formulas
    ///
    /// To find the border box from the content box, we expand outward through
    /// two layers (padding, border):
    ///
    /// ```text
    /// x = content.x - padding.left - border.left
    /// y = content.y - padding.top - border.top
    ///
    /// width = content.width
    ///       + padding.left + padding.right
    ///       + border.left + border.right
    ///
    /// height = content.height
    ///        + padding.top + padding.bottom
    ///        + border.top + border.bottom
    /// ```
    #[must_use]
    pub fn border_box(&self) -> Rect {
        Rect {
            x: self.content.x - self.padding.left - self.border.left,
            y: self.content.y - self.padding.top - self.border.top,
            width: self.content.width
                + self.padding.left
                + self.padding.right
                + self.border.left
                + self.border.right,
            height: self.content.height
                + self.padding.top
                + self.padding.bottom
                + self.border.top
                + self.border.bottom,
        }
    }
    /// [§ 3 The CSS Box Model](https://www.w3.org/TR/css-box-3/#box-model)
    /// "The content box contains the actual content of the element."
    #[must_use]
    pub const fn content_box(&self) -> Rect {
        self.content
    }
}
