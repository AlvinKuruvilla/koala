//! Display List - a sequence of drawing commands
//!
//! [CSS 2.1 Appendix E](https://www.w3.org/TR/CSS2/zindex.html)
//!
//! The display list is the output of the painting phase. It contains all the
//! drawing commands needed to render a page, in the correct z-order.

use crate::ColorValue;

/// A single drawing command.
///
/// [CSS 2.1 Appendix E.2 Painting order](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
///
/// Commands are added to the display list in painting order (back to front).
#[derive(Debug, Clone)]
pub enum DisplayCommand {
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
    },
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
    pub fn new() -> Self {
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
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if the display list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
