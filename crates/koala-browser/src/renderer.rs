//! Software renderer for headless screenshot generation.
//!
//! Executes a DisplayList to a pixel buffer using fontdue for text rasterization.
//!
//! # Architecture
//!
//! The renderer is the final stage in the pipeline:
//!
//! ```text
//! Style → Layout → Paint → Render
//!                    ↓        ↓
//!              DisplayList → Pixels
//! ```
//!
//! The renderer knows nothing about CSS, layout, or the DOM. It simply executes
//! drawing commands from the display list.

use anyhow::Result;
use fontdue::{Font, FontSettings};
use image::{ImageBuffer, Rgba, RgbaImage};
use koala_css::{ColorValue, DisplayCommand, DisplayList};
use std::path::Path;

/// Common system font paths to search for a default font.
const FONT_SEARCH_PATHS: &[&str] = &[
    // macOS
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/SFNS.ttf",
    "/Library/Fonts/Arial.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    // Linux
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    // Windows
    "C:\\Windows\\Fonts\\arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
];

/// Software renderer that executes a display list to a pixel buffer.
///
/// The renderer is stateless with respect to CSS - it only knows how to
/// execute drawing commands (fill rectangles, draw text).
pub struct Renderer {
    /// RGBA pixel buffer
    buffer: RgbaImage,
    /// Width in pixels
    width: u32,
    /// Height in pixels
    height: u32,
    /// Font for text rendering (None if no font found)
    font: Option<Font>,
}

impl Renderer {
    /// Create a new renderer with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        // Create white background
        let buffer = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));

        // Try to load a system font
        let font = Self::load_system_font();

        if font.is_none() {
            eprintln!("Warning: No system font found. Text will not be rendered.");
            eprintln!("Searched paths:");
            for path in FONT_SEARCH_PATHS {
                eprintln!("  - {path}");
            }
        }

        Self {
            buffer,
            width,
            height,
            font,
        }
    }

    /// Try to load a font from common system paths.
    pub fn load_system_font() -> Option<Font> {
        for path in FONT_SEARCH_PATHS {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                    eprintln!("Loaded font: {path}");
                    return Some(font);
                }
            }
        }
        None
    }

    /// Execute a display list, drawing all commands to the pixel buffer.
    ///
    /// Commands are executed in order (back to front), which is the correct
    /// painting order established by the Painter.
    pub fn render(&mut self, display_list: &DisplayList) {
        for command in display_list.commands() {
            self.execute_command(command);
        }
    }

    /// Execute a single display command.
    fn execute_command(&mut self, command: &DisplayCommand) {
        match command {
            DisplayCommand::FillRect {
                x,
                y,
                width,
                height,
                color,
            } => {
                self.fill_rect(*x, *y, *width, *height, color);
            }
            DisplayCommand::DrawText {
                x,
                y,
                text,
                font_size,
                color,
            } => {
                self.draw_text(text, *x, *y, *font_size, color);
            }
        }
    }

    /// Fill a rectangle with the given color.
    fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: &ColorValue) {
        let rgba = Rgba([color.r, color.g, color.b, color.a]);
        let x = x as i32;
        let y = y as i32;
        let width = width as u32;
        let height = height as u32;

        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx as i32;
                let py = y + dy as i32;
                if px >= 0 && py >= 0 && (px as u32) < self.width && (py as u32) < self.height {
                    self.buffer.put_pixel(px as u32, py as u32, rgba);
                }
            }
        }
    }

    /// Draw text at the given position.
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: &ColorValue) {
        // Skip if no font is available
        let font = match &self.font {
            Some(f) => f,
            None => return,
        };

        let rgba = Rgba([color.r, color.g, color.b, color.a]);
        let mut cursor_x = x;
        let cursor_y = y;

        for ch in text.chars() {
            // Skip control characters and newlines for now
            if ch.is_control() {
                if ch == '\n' {
                    // TODO: Handle line breaks properly
                }
                continue;
            }

            // Rasterize the character
            let (metrics, bitmap) = font.rasterize(ch, font_size);

            // Calculate position (fontdue gives us the bitmap offset)
            let glyph_x = cursor_x as i32 + metrics.xmin;
            let glyph_y =
                cursor_y as i32 + (font_size as i32 - metrics.ymin - metrics.height as i32);

            // Draw the glyph
            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let alpha = bitmap[gy * metrics.width + gx];
                    if alpha > 0 {
                        let px = glyph_x + gx as i32;
                        let py = glyph_y + gy as i32;

                        if px >= 0
                            && py >= 0
                            && (px as u32) < self.width
                            && (py as u32) < self.height
                        {
                            // Alpha blend the glyph onto the background
                            let bg = self.buffer.get_pixel(px as u32, py as u32);
                            let blended = alpha_blend(rgba, *bg, alpha);
                            self.buffer.put_pixel(px as u32, py as u32, blended);
                        }
                    }
                }
            }

            // Advance cursor
            cursor_x += metrics.advance_width;
        }
    }

    /// Save the rendered image to a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.buffer.save(path)?;
        Ok(())
    }
}

/// Alpha blend a foreground color onto a background color.
fn alpha_blend(fg: Rgba<u8>, bg: Rgba<u8>, alpha: u8) -> Rgba<u8> {
    let a = alpha as f32 / 255.0;
    let inv_a = 1.0 - a;

    Rgba([
        (fg[0] as f32 * a + bg[0] as f32 * inv_a) as u8,
        (fg[1] as f32 * a + bg[1] as f32 * inv_a) as u8,
        (fg[2] as f32 * a + bg[2] as f32 * inv_a) as u8,
        255,
    ])
}
