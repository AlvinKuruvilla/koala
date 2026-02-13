//! Display List - a sequence of drawing commands
//!
//! [CSS 2.1 Appendix E](https://www.w3.org/TR/CSS2/zindex.html)
//!
//! The display list is the output of the painting phase. It contains all the
//! drawing commands needed to render a page, in the correct z-order.

use crate::ColorValue;
use crate::layout::inline::{FontStyle, TextDecorationLine};
use crate::style::BorderRadius;

/// A single drawing command.
///
/// [CSS 2.1 Appendix E.2 Painting order](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
///
/// Commands are added to the display list in painting order (back to front).
#[derive(Debug, Clone)]
pub enum DisplayCommand {
    /// Draw a box shadow (outer or inset).
    ///
    /// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
    ///
    /// Outer shadows are painted before the background (painting order step 1).
    /// Inset shadows are painted after the border (painting order step 3).
    DrawBoxShadow {
        /// X coordinate of the border box's top-left corner.
        border_box_x: f32,
        /// Y coordinate of the border box's top-left corner.
        border_box_y: f32,
        /// Width of the border box.
        border_box_width: f32,
        /// Height of the border box.
        border_box_height: f32,
        /// Horizontal offset of the shadow.
        offset_x: f32,
        /// Vertical offset of the shadow.
        offset_y: f32,
        /// Blur radius. 0 = sharp edge.
        blur_radius: f32,
        /// Spread distance. Positive = larger shadow, negative = smaller.
        spread_radius: f32,
        /// Shadow color.
        color: ColorValue,
        /// If true, this is an inset (inner) shadow.
        inset: bool,
    },

    /// Fill a rectangle with a solid color.
    ///
    /// Used for backgrounds and solid borders.
    FillRect {
        /// X coordinate of the rectangle's top-left corner.
        x: f32,
        /// Y coordinate of the rectangle's top-left corner.
        y: f32,
        /// Width of the rectangle in pixels.
        width: f32,
        /// Height of the rectangle in pixels.
        height: f32,
        /// Fill color.
        color: ColorValue,
        /// [§ 5 'border-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-radius)
        ///
        /// Corner radii for rounded rectangles. Default (all zeros) = sharp corners.
        border_radius: BorderRadius,
    },

    /// Draw an image (replaced element content) at a position.
    ///
    /// [CSS 2.1 Appendix E.2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
    /// Step 5: "the replaced content of replaced inline-level elements"
    ///
    /// The `src` string is used as a key to look up the loaded image data
    /// in the renderer's image store.
    DrawImage {
        /// X coordinate of the image's top-left corner.
        x: f32,
        /// Y coordinate of the image's top-left corner.
        y: f32,
        /// Rendered width of the image in pixels.
        width: f32,
        /// Rendered height of the image in pixels.
        height: f32,
        /// The `src` attribute value, used as lookup key for image data.
        src: String,
        /// [§ 3.2 'opacity'](https://www.w3.org/TR/css-color-4/#transparency)
        ///
        /// Opacity multiplier for the image (0.0 = fully transparent, 1.0 = fully opaque).
        opacity: f32,
    },

    /// Draw text at a position.
    ///
    /// [CSS 2.1 Appendix E.2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
    /// Step 7: "the element's text"
    DrawText {
        /// X coordinate of the text baseline origin.
        x: f32,
        /// Y coordinate of the text baseline origin.
        y: f32,
        /// The text content to draw.
        text: String,
        /// Font size in pixels.
        font_size: f32,
        /// Text color.
        color: ColorValue,
        /// [§ 3.2 'font-weight'](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
        ///
        /// Numeric weight (400 = normal, 700 = bold).
        font_weight: u16,
        /// [§ 3.3 'font-style'](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
        font_style: FontStyle,
        /// [§ 3 'text-decoration-line'](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
        text_decoration: TextDecorationLine,
    },

    /// Push a clip rectangle onto the clip stack.
    ///
    /// [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    ///
    /// All subsequent drawing commands are clipped to the intersection of
    /// all active clip rectangles. Used for `overflow: hidden`.
    PushClip {
        /// X coordinate of the clip rectangle.
        x: f32,
        /// Y coordinate of the clip rectangle.
        y: f32,
        /// Width of the clip rectangle.
        width: f32,
        /// Height of the clip rectangle.
        height: f32,
    },

    /// Pop the most recent clip rectangle from the clip stack.
    PopClip,
}

/// A list of drawing commands in painting order.
///
/// [CSS 2.1 Appendix E.2 Painting order](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
///
/// The display list contains all commands needed to render a page. Commands
/// are stored in back-to-front order, so the renderer can simply iterate
/// and execute each command.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    commands: Vec<DisplayCommand>,
}

impl DisplayList {
    /// Create an empty display list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Add a command to the display list.
    pub fn push(&mut self, command: DisplayCommand) {
        self.commands.push(command);
    }

    /// Get the commands in painting order.
    #[must_use]
    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
    }

    /// Get the number of commands.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if the display list is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
