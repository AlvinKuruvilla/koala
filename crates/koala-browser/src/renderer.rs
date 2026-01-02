//! Software renderer for headless screenshot generation.
//!
//! Renders a LayoutBox tree to a pixel buffer using fontdue for text rasterization.

use crate::LoadedDocument;
use fontdue::{Font, FontSettings};
use anyhow::Result;
use image::{ImageBuffer, Rgba, RgbaImage};
use koala_css::{BoxType, ComputedStyle, LayoutBox};
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

/// Software renderer that paints a LayoutBox tree to a pixel buffer.
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
                eprintln!("  - {}", path);
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
    fn load_system_font() -> Option<Font> {
        for path in FONT_SEARCH_PATHS {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                    eprintln!("Loaded font: {}", path);
                    return Some(font);
                }
            }
        }
        None
    }

    /// Render the layout tree to the pixel buffer.
    pub fn render(&mut self, layout: &LayoutBox, doc: &LoadedDocument) {
        self.render_box(layout, doc);
    }

    /// Recursively render a layout box and its children.
    fn render_box(&mut self, layout_box: &LayoutBox, doc: &LoadedDocument) {
        let dims = &layout_box.dimensions;

        // Get style for this box if it has a node
        let style = match &layout_box.box_type {
            BoxType::Principal(node_id) => doc.styles.get(node_id),
            _ => None,
        };

        // Calculate the padding box (background area)
        let padding_x = dims.content.x - dims.padding.left;
        let padding_y = dims.content.y - dims.padding.top;
        let padding_width = dims.content.width + dims.padding.left + dims.padding.right;
        let padding_height = dims.content.height + dims.padding.top + dims.padding.bottom;

        // Draw background color
        if let Some(style) = style {
            if let Some(bg) = &style.background_color {
                self.fill_rect(
                    padding_x as i32,
                    padding_y as i32,
                    padding_width as u32,
                    padding_height as u32,
                    Rgba([bg.r, bg.g, bg.b, bg.a]),
                );
            }
        }

        // Draw text for anonymous inline boxes
        if let BoxType::AnonymousInline(text) = &layout_box.box_type {
            // Get text color from parent or default to black
            let text_color = style
                .and_then(|s| s.color.as_ref())
                .map(|c| Rgba([c.r, c.g, c.b, c.a]))
                .unwrap_or(Rgba([0, 0, 0, 255]));

            // Get font size from style or default
            let font_size = style
                .and_then(|s| s.font_size.as_ref())
                .map(|fs| fs.to_px() as f32)
                .unwrap_or(16.0);

            self.draw_text(
                text,
                dims.content.x as i32,
                dims.content.y as i32,
                font_size,
                text_color,
            );
        }

        // Render children
        for child in &layout_box.children {
            // Pass parent style to children for text color inheritance
            self.render_box_with_inherited_style(child, doc, style);
        }
    }

    /// Render a box with inherited style from parent.
    fn render_box_with_inherited_style(
        &mut self,
        layout_box: &LayoutBox,
        doc: &LoadedDocument,
        parent_style: Option<&ComputedStyle>,
    ) {
        let dims = &layout_box.dimensions;

        // Get style for this box if it has a node
        let style = match &layout_box.box_type {
            BoxType::Principal(node_id) => doc.styles.get(node_id),
            _ => None,
        };

        // Use own style or inherit from parent
        let effective_style = style.or(parent_style);

        // Calculate the padding box (background area)
        let padding_x = dims.content.x - dims.padding.left;
        let padding_y = dims.content.y - dims.padding.top;
        let padding_width = dims.content.width + dims.padding.left + dims.padding.right;
        let padding_height = dims.content.height + dims.padding.top + dims.padding.bottom;

        // Draw background color
        if let Some(style) = style {
            if let Some(bg) = &style.background_color {
                self.fill_rect(
                    padding_x as i32,
                    padding_y as i32,
                    padding_width as u32,
                    padding_height as u32,
                    Rgba([bg.r, bg.g, bg.b, bg.a]),
                );
            }
        }

        // Draw text for anonymous inline boxes
        if let BoxType::AnonymousInline(text) = &layout_box.box_type {
            // Get text color from effective style or default to black
            let text_color = effective_style
                .and_then(|s| s.color.as_ref())
                .map(|c| Rgba([c.r, c.g, c.b, c.a]))
                .unwrap_or(Rgba([0, 0, 0, 255]));

            // Get font size from effective style or default
            let font_size = effective_style
                .and_then(|s| s.font_size.as_ref())
                .map(|fs| fs.to_px() as f32)
                .unwrap_or(16.0);

            self.draw_text(
                text,
                dims.content.x as i32,
                dims.content.y as i32,
                font_size,
                text_color,
            );
        }

        // Render children
        for child in &layout_box.children {
            self.render_box_with_inherited_style(child, doc, effective_style);
        }
    }

    /// Fill a rectangle with the given color.
    fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Rgba<u8>) {
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx as i32;
                let py = y + dy as i32;
                if px >= 0 && py >= 0 && (px as u32) < self.width && (py as u32) < self.height {
                    self.buffer.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }

    /// Draw text at the given position.
    fn draw_text(&mut self, text: &str, x: i32, y: i32, font_size: f32, color: Rgba<u8>) {
        // Skip if no font is available
        let font = match &self.font {
            Some(f) => f,
            None => return,
        };

        let mut cursor_x = x as f32;
        let cursor_y = y as f32;

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
            let glyph_y = cursor_y as i32 + (font_size as i32 - metrics.ymin - metrics.height as i32);

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
                            let blended = alpha_blend(color, *bg, alpha);
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
