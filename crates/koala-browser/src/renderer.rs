//! Software renderer for headless screenshot generation.
//!
//! Executes a `DisplayList` to a pixel buffer using fontdue for text rasterization.
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
use koala_css::layout::inline::TextDecorationLine;
use koala_css::{BorderRadius, ColorValue, DisplayCommand, DisplayList, FontStyle};
use std::collections::HashMap;
use std::path::Path;

use koala_common::image::LoadedImage;

/// Common system font paths to search for a default (regular) font.
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

/// System font paths for bold variants.
const FONT_BOLD_SEARCH_PATHS: &[&str] = &[
    // macOS
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/Library/Fonts/Arial Bold.ttf",
    // Linux
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
    // Windows
    "C:\\Windows\\Fonts\\arialbd.ttf",
];

/// System font paths for italic variants.
const FONT_ITALIC_SEARCH_PATHS: &[&str] = &[
    // macOS
    "/System/Library/Fonts/Supplemental/Arial Italic.ttf",
    "/Library/Fonts/Arial Italic.ttf",
    // Linux
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Oblique.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Italic.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSansOblique.ttf",
    // Windows
    "C:\\Windows\\Fonts\\ariali.ttf",
];

/// System font paths for bold-italic variants.
const FONT_BOLD_ITALIC_SEARCH_PATHS: &[&str] = &[
    // macOS
    "/System/Library/Fonts/Supplemental/Arial Bold Italic.ttf",
    "/Library/Fonts/Arial Bold Italic.ttf",
    // Linux
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-BoldOblique.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-BoldOblique.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-BoldItalic.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSansBoldOblique.ttf",
    // Windows
    "C:\\Windows\\Fonts\\arialbi.ttf",
];

/// Software renderer that executes a display list to a pixel buffer.
///
/// The renderer is stateless with respect to CSS - it only knows how to
/// execute drawing commands (fill rectangles, draw text, draw images).
pub struct Renderer {
    /// RGBA pixel buffer
    buffer: RgbaImage,
    /// Width in pixels
    width: u32,
    /// Height in pixels
    height: u32,
    /// Regular font for text rendering (None if no font found)
    font: Option<Font>,
    /// Bold font variant (None falls back to regular)
    font_bold: Option<Font>,
    /// Italic font variant (None falls back to regular)
    font_italic: Option<Font>,
    /// Bold-italic font variant (None falls back to bold or italic or regular)
    font_bold_italic: Option<Font>,
    /// Loaded images keyed by src attribute. Used for `DrawImage` commands.
    images: HashMap<String, LoadedImage>,
    /// Stack of active clip rectangles for overflow: hidden.
    ///
    /// [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    ///
    /// Each entry is (x, y, width, height) in pixel coordinates.
    clip_stack: Vec<(f32, f32, f32, f32)>,
}

impl Renderer {
    /// Create a new renderer with the given dimensions and optional image data.
    #[must_use]
    pub fn new(width: u32, height: u32, images: HashMap<String, LoadedImage>) -> Self {
        // Create white background
        let buffer = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));

        // Try to load system fonts (regular + variants)
        let font = Self::load_font_from_paths(FONT_SEARCH_PATHS, "regular");
        let font_bold = Self::load_font_from_paths(FONT_BOLD_SEARCH_PATHS, "bold");
        let font_italic = Self::load_font_from_paths(FONT_ITALIC_SEARCH_PATHS, "italic");
        let font_bold_italic =
            Self::load_font_from_paths(FONT_BOLD_ITALIC_SEARCH_PATHS, "bold-italic");

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
            font_bold,
            font_italic,
            font_bold_italic,
            images,
            clip_stack: Vec::new(),
        }
    }

    /// Try to load a font from a list of filesystem paths.
    fn load_font_from_paths(paths: &[&str], label: &str) -> Option<Font> {
        for path in paths {
            if let Ok(data) = std::fs::read(path)
                && let Ok(font) = Font::from_bytes(data, FontSettings::default())
            {
                eprintln!("Loaded {label} font: {path}");
                return Some(font);
            }
        }
        None
    }

    /// Try to load the regular system font (public API, kept for compatibility).
    #[must_use]
    pub fn load_system_font() -> Option<Font> {
        Self::load_font_from_paths(FONT_SEARCH_PATHS, "regular")
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
            DisplayCommand::DrawBoxShadow {
                border_box_x,
                border_box_y,
                border_box_width,
                border_box_height,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                color,
                inset,
            } => {
                self.draw_box_shadow(
                    *border_box_x,
                    *border_box_y,
                    *border_box_width,
                    *border_box_height,
                    *offset_x,
                    *offset_y,
                    *blur_radius,
                    *spread_radius,
                    color,
                    *inset,
                );
            }
            DisplayCommand::FillRect {
                x,
                y,
                width,
                height,
                color,
                border_radius,
            } => {
                self.fill_rect(*x, *y, *width, *height, color, border_radius);
            }
            DisplayCommand::DrawImage {
                x,
                y,
                width,
                height,
                src,
                opacity,
            } => {
                self.draw_image(src, *x, *y, *width, *height, *opacity);
            }
            DisplayCommand::DrawText {
                x,
                y,
                text,
                font_size,
                color,
                font_weight,
                font_style,
                text_decoration,
            } => {
                self.draw_text(
                    text,
                    *x,
                    *y,
                    *font_size,
                    color,
                    *font_weight,
                    *font_style,
                    *text_decoration,
                );
            }
            DisplayCommand::PushClip {
                x,
                y,
                width,
                height,
            } => {
                self.clip_stack.push((*x, *y, *width, *height));
            }
            DisplayCommand::PopClip => {
                let _ = self.clip_stack.pop();
            }
        }
    }

    /// Check if a pixel is within all active clip rectangles.
    ///
    /// [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    #[allow(clippy::cast_precision_loss)]
    fn is_visible(&self, px: i32, py: i32) -> bool {
        let fx = px as f32;
        let fy = py as f32;
        self.clip_stack
            .iter()
            .all(|&(cx, cy, cw, ch)| fx >= cx && fx < cx + cw && fy >= cy && fy < cy + ch)
    }

    /// Fill a rectangle with the given color, optionally with rounded corners.
    ///
    /// [§ 5 'border-radius'](https://www.w3.org/TR/css-backgrounds-3/#border-radius)
    ///
    /// For each pixel near a corner with a non-zero radius, we check if the
    /// pixel falls inside the quarter-circle arc. Pixels outside the arc are
    /// skipped, producing rounded corners.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::many_single_char_names,
    )]
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: &ColorValue,
        border_radius: &BorderRadius,
    ) {
        let rgba = Rgba([color.r, color.g, color.b, color.a]);
        let xi = x as i32;
        let yi = y as i32;
        let w = width as u32;
        let h = height as u32;

        let has_radius = border_radius.top_left > 0.0
            || border_radius.top_right > 0.0
            || border_radius.bottom_left > 0.0
            || border_radius.bottom_right > 0.0;

        for dy in 0..h {
            for dx in 0..w {
                let px = xi + dx as i32;
                let py = yi + dy as i32;
                if px < 0
                    || py < 0
                    || (px as u32) >= self.width
                    || (py as u32) >= self.height
                    || !self.is_visible(px, py)
                {
                    continue;
                }

                // Check if pixel is inside rounded corners
                if has_radius {
                    let fx = dx as f32;
                    let fy = dy as f32;
                    let fw = width;
                    let fh = height;

                    // Top-left corner
                    let r = border_radius.top_left;
                    if r > 0.0 && fx < r && fy < r {
                        let cx = r;
                        let cy = r;
                        let dist_sq = (fx - cx).mul_add(fx - cx, (fy - cy) * (fy - cy));
                        if dist_sq > r * r {
                            continue;
                        }
                    }

                    // Top-right corner
                    let r = border_radius.top_right;
                    if r > 0.0 && fx >= fw - r && fy < r {
                        let cx = fw - r;
                        let cy = r;
                        let dist_sq = (fx - cx).mul_add(fx - cx, (fy - cy) * (fy - cy));
                        if dist_sq > r * r {
                            continue;
                        }
                    }

                    // Bottom-left corner
                    let r = border_radius.bottom_left;
                    if r > 0.0 && fx < r && fy >= fh - r {
                        let cx = r;
                        let cy = fh - r;
                        let dist_sq = (fx - cx).mul_add(fx - cx, (fy - cy) * (fy - cy));
                        if dist_sq > r * r {
                            continue;
                        }
                    }

                    // Bottom-right corner
                    let r = border_radius.bottom_right;
                    if r > 0.0 && fx >= fw - r && fy >= fh - r {
                        let cx = fw - r;
                        let cy = fh - r;
                        let dist_sq = (fx - cx).mul_add(fx - cx, (fy - cy) * (fy - cy));
                        if dist_sq > r * r {
                            continue;
                        }
                    }
                }

                self.buffer.put_pixel(px as u32, py as u32, rgba);
            }
        }
    }

    /// Draw an image scaled to the destination rectangle.
    ///
    /// Uses nearest-neighbor sampling to scale the source RGBA data to the
    /// destination size, then alpha-blends onto the buffer.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    fn draw_image(&mut self, src: &str, x: f32, y: f32, width: f32, height: f32, opacity: f32) {
        let Some(img) = self.images.get(src) else {
            return;
        };

        let dest_x = x as i32;
        let dest_y = y as i32;
        let dest_w = width as u32;
        let dest_h = height as u32;
        let src_w = img.width();
        let src_h = img.height();

        if src_w == 0 || src_h == 0 || dest_w == 0 || dest_h == 0 {
            return;
        }

        for dy in 0..dest_h {
            for dx in 0..dest_w {
                let px = dest_x + dx as i32;
                let py = dest_y + dy as i32;

                if px < 0
                    || py < 0
                    || (px as u32) >= self.width
                    || (py as u32) >= self.height
                    || !self.is_visible(px, py)
                {
                    continue;
                }

                // Nearest-neighbor sampling
                let sx = ((u64::from(dx) * u64::from(src_w)) / u64::from(dest_w))
                    .min(u64::from(src_w) - 1) as u32;
                let sy = ((u64::from(dy) * u64::from(src_h)) / u64::from(dest_h))
                    .min(u64::from(src_h) - 1) as u32;
                let src_idx = ((sy * src_w + sx) * 4) as usize;

                let sr = img.rgba_data()[src_idx];
                let sg = img.rgba_data()[src_idx + 1];
                let sb = img.rgba_data()[src_idx + 2];
                let sa = img.rgba_data()[src_idx + 3];

                // [§ 3.2 'opacity'](https://www.w3.org/TR/css-color-4/#transparency)
                //
                // Multiply the source pixel alpha by the element's opacity.
                let effective_alpha = (f32::from(sa) * opacity) as u8;

                if effective_alpha == 0 {
                    continue;
                }

                let fg = Rgba([sr, sg, sb, effective_alpha]);
                if effective_alpha == 255 {
                    self.buffer.put_pixel(px as u32, py as u32, fg);
                } else {
                    let bg = *self.buffer.get_pixel(px as u32, py as u32);
                    let blended = alpha_blend(fg, bg, effective_alpha);
                    self.buffer.put_pixel(px as u32, py as u32, blended);
                }
            }
        }
    }

    /// Draw text at the given position.
    #[allow(
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
    )]
    fn draw_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: &ColorValue,
        font_weight: u16,
        font_style: FontStyle,
        text_decoration: TextDecorationLine,
    ) {
        // Select the best available font for the given weight and style,
        // falling back through: exact match → partial match → regular.
        let is_bold = font_weight >= 700;
        let is_italic = font_style != FontStyle::Normal;

        let font = match (is_bold, is_italic) {
            (true, true) => self
                .font_bold_italic
                .as_ref()
                .or(self.font_bold.as_ref())
                .or(self.font.as_ref()),
            (true, false) => self.font_bold.as_ref().or(self.font.as_ref()),
            (false, true) => self.font_italic.as_ref().or(self.font.as_ref()),
            (false, false) => self.font.as_ref(),
        };

        let Some(font) = font else {
            return;
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
                            && self.is_visible(px, py)
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

        // [§ 3 Text Decoration Lines](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
        //
        // Draw text decoration lines after the glyphs.
        let total_advance = cursor_x - x;
        if total_advance > 0.0
            && (text_decoration.underline || text_decoration.overline || text_decoration.line_through)
        {
            let line_thickness = (font_size / 16.0).max(1.0);

            if text_decoration.underline {
                // Underline: just below the baseline.
                let line_y = font_size.mul_add(0.9, y);
                self.fill_rect(x, line_y, total_advance, line_thickness, color, &BorderRadius::default());
            }
            if text_decoration.line_through {
                // Line-through: through the middle of the text.
                let line_y = font_size.mul_add(0.55, y);
                self.fill_rect(x, line_y, total_advance, line_thickness, color, &BorderRadius::default());
            }
            if text_decoration.overline {
                // Overline: at the top of the text.
                let line_y = font_size.mul_add(0.1, y);
                self.fill_rect(x, line_y, total_advance, line_thickness, color, &BorderRadius::default());
            }
        }
    }

    /// Draw a box shadow (outer or inset).
    ///
    /// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
    ///
    /// Uses layered concentric rectangles with linearly decreasing alpha to
    /// approximate blur. This is a visual approximation, not true Gaussian blur.
    #[allow(
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    fn draw_box_shadow(
        &mut self,
        border_box_x: f32,
        border_box_y: f32,
        border_box_width: f32,
        border_box_height: f32,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        color: &ColorValue,
        inset: bool,
    ) {
        if inset {
            self.draw_inset_shadow(
                border_box_x,
                border_box_y,
                border_box_width,
                border_box_height,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                color,
            );
        } else {
            self.draw_outer_shadow(
                border_box_x,
                border_box_y,
                border_box_width,
                border_box_height,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                color,
            );
        }
    }

    /// Draw an outer box shadow.
    ///
    /// The shadow rect = border-box expanded by spread, offset, and blur.
    /// Pixels inside the border box are skipped (shadow is outside only).
    #[allow(
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
    )]
    fn draw_outer_shadow(
        &mut self,
        border_box_x: f32,
        border_box_y: f32,
        border_box_width: f32,
        border_box_height: f32,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        color: &ColorValue,
    ) {
        // Shadow rect = border-box + spread + offset
        let shadow_x = border_box_x + offset_x - spread_radius;
        let shadow_y = border_box_y + offset_y - spread_radius;
        let shadow_w = spread_radius.mul_add(2.0, border_box_width);
        let shadow_h = spread_radius.mul_add(2.0, border_box_height);

        let layers = if blur_radius > 0.0 {
            blur_radius.ceil() as u32
        } else {
            1
        };

        let base_alpha = f32::from(color.a) / 255.0;

        for layer in 0..layers {
            let expand = if layers > 1 {
                layer as f32
            } else {
                0.0
            };
            // Alpha decreases linearly with each expanding layer
            let layer_alpha = if layers > 1 {
                base_alpha * (1.0 - (layer as f32 / layers as f32))
            } else {
                base_alpha
            };

            let alpha_u8 = (layer_alpha * 255.0) as u8;
            if alpha_u8 == 0 {
                continue;
            }

            let lx = (shadow_x - expand) as i32;
            let ly = (shadow_y - expand) as i32;
            let lw = expand.mul_add(2.0, shadow_w) as u32;
            let lh = expand.mul_add(2.0, shadow_h) as u32;

            let fg = Rgba([color.r, color.g, color.b, alpha_u8]);

            // Border box bounds (pixels inside are skipped for outer shadow)
            let bb_left = border_box_x as i32;
            let bb_top = border_box_y as i32;
            let bb_right = (border_box_x + border_box_width) as i32;
            let bb_bottom = (border_box_y + border_box_height) as i32;

            for dy in 0..lh {
                for dx in 0..lw {
                    let px = lx + dx as i32;
                    let py = ly + dy as i32;

                    // Skip pixels inside the border box (shadow is outside only)
                    if px >= bb_left && px < bb_right && py >= bb_top && py < bb_bottom {
                        continue;
                    }

                    if px >= 0
                        && py >= 0
                        && (px as u32) < self.width
                        && (py as u32) < self.height
                        && self.is_visible(px, py)
                    {
                        let bg = *self.buffer.get_pixel(px as u32, py as u32);
                        let blended = alpha_blend(fg, bg, alpha_u8);
                        self.buffer.put_pixel(px as u32, py as u32, blended);
                    }
                }
            }
        }
    }

    /// Draw an inset box shadow.
    ///
    /// The shadow region = inside the border box but outside the inner rect
    /// (contracted by spread + offset).
    #[allow(
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
    )]
    fn draw_inset_shadow(
        &mut self,
        border_box_x: f32,
        border_box_y: f32,
        border_box_width: f32,
        border_box_height: f32,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        color: &ColorValue,
    ) {
        // Inner rect = border-box shrunk by spread, shifted by offset
        let inner_x = border_box_x + offset_x + spread_radius;
        let inner_y = border_box_y + offset_y + spread_radius;
        let inner_w = spread_radius.mul_add(-2.0, border_box_width);
        let inner_h = spread_radius.mul_add(-2.0, border_box_height);

        let layers = if blur_radius > 0.0 {
            blur_radius.ceil() as u32
        } else {
            1
        };

        let base_alpha = f32::from(color.a) / 255.0;

        let bb_left = border_box_x as i32;
        let bb_top = border_box_y as i32;
        let bb_right = (border_box_x + border_box_width) as i32;
        let bb_bottom = (border_box_y + border_box_height) as i32;

        for layer in 0..layers {
            let shrink = if layers > 1 {
                layer as f32
            } else {
                0.0
            };
            let layer_alpha = if layers > 1 {
                base_alpha * (1.0 - (layer as f32 / layers as f32))
            } else {
                base_alpha
            };

            let alpha_u8 = (layer_alpha * 255.0) as u8;
            if alpha_u8 == 0 {
                continue;
            }

            let il = (inner_x + shrink) as i32;
            let it = (inner_y + shrink) as i32;
            let ir = ((inner_x + inner_w) - shrink) as i32;
            let ib = ((inner_y + inner_h) - shrink) as i32;

            let fg = Rgba([color.r, color.g, color.b, alpha_u8]);

            // Paint pixels inside border box but outside the inner rect
            for py in bb_top..bb_bottom {
                for px in bb_left..bb_right {
                    // Skip pixels inside the inner rect (no shadow there)
                    if px >= il && px < ir && py >= it && py < ib {
                        continue;
                    }

                    if px >= 0
                        && py >= 0
                        && (px as u32) < self.width
                        && (py as u32) < self.height
                        && self.is_visible(px, py)
                    {
                        let bg = *self.buffer.get_pixel(px as u32, py as u32);
                        let blended = alpha_blend(fg, bg, alpha_u8);
                        self.buffer.put_pixel(px as u32, py as u32, blended);
                    }
                }
            }
        }
    }

    /// Save the rendered image to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the image cannot be saved to the given path.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.buffer.save(path).map_err(|e| {
            anyhow::anyhow!("failed to save screenshot to '{}': {e}", path.display())
        })?;
        Ok(())
    }
}

/// Alpha blend a foreground color onto a background color.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn alpha_blend(fg: Rgba<u8>, bg: Rgba<u8>, alpha: u8) -> Rgba<u8> {
    let a = f32::from(alpha) / 255.0;
    let inv_a = 1.0 - a;

    Rgba([
        f32::from(fg[0]).mul_add(a, f32::from(bg[0]) * inv_a) as u8,
        f32::from(fg[1]).mul_add(a, f32::from(bg[1]) * inv_a) as u8,
        f32::from(fg[2]).mul_add(a, f32::from(bg[2]) * inv_a) as u8,
        255,
    ])
}
