//! Render-layer verification for `letter-spacing`.
//!
//! [CSS Text Module Level 3 § 9.3](https://www.w3.org/TR/css-text-3/#letter-spacing-property)
//!
//! The layout pipeline produces a `DisplayCommand::DrawText` whose
//! `letter_spacing` field tells the renderer to advance the glyph
//! cursor by `glyph_advance + letter_spacing` between adjacent
//! characters (and only between — not after the last). The painted
//! `total_advance` is what `Renderer::draw_text` then uses to size
//! `text-decoration: underline`, so the underline's painted length
//! is a direct, pixel-readable signal of the cursor's final
//! position.
//!
//! This test paints two underlined runs of identical text with two
//! different `letter_spacing` values, scans the buffer for the
//! rightmost underline pixel in each, and asserts the difference
//! matches `(n - 1) * delta_spacing`. That's the same `(n - 1) *
//! letter_spacing` term the layout-side `FontMetrics::text_width`
//! adds, so a passing test confirms the layout and render layers
//! agree on text width.

use koala_std::collections::HashMap;
use std::sync::Arc;

use fontdue::{Font, FontSettings};
use koala_browser::{Renderer, RendererFonts};
use koala_css::{ColorValue, DisplayCommand, DisplayList, FontStyle, TextDecorationLine};

/// Inter-Regular baked at compile time so the test is independent of
/// whatever fonts happen to be installed on the host. The four Inter
/// variants live in `res/fonts/` (OFL-licensed, v4.1).
const INTER_REGULAR_TTF: &[u8] = include_bytes!("../../../res/fonts/Inter-Regular.ttf");

/// Build a `Renderer` whose only loaded font is `Inter-Regular`. The
/// other three variants stay `None`; the test only paints regular
/// text so the fallback chain never fires.
fn make_renderer(width: u32, height: u32) -> Renderer {
    let font = Font::from_bytes(INTER_REGULAR_TTF, FontSettings::default())
        .expect("Inter-Regular.ttf is a valid font file");
    let fonts = RendererFonts {
        regular: Some(Arc::new(font)),
        bold: None,
        italic: None,
        bold_italic: None,
    };
    Renderer::new_with_fonts(width, height, HashMap::new(), fonts)
}

/// Construct a `DisplayList` with exactly one underlined `DrawText`
/// command at the given `letter_spacing`. All other text properties
/// are held constant so the only signal in the painted buffer is the
/// spacing.
fn underlined_drawtext(
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    letter_spacing: f32,
) -> DisplayList {
    let mut list = DisplayList::new();
    list.push(DisplayCommand::DrawText {
        x,
        y,
        text: text.to_string(),
        font_size,
        color: ColorValue {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        font_weight: 400,
        font_style: FontStyle::Normal,
        text_decoration: TextDecorationLine {
            underline: true,
            overline: false,
            line_through: false,
        },
        letter_spacing,
    });
    list
}

/// Scan a horizontal pixel row for the rightmost non-white pixel.
/// The buffer is initialized to opaque white in `allocate_buffer`,
/// so any column that holds a black or near-black pixel is part of
/// the underline (or, near the bottom of the glyph, part of an
/// antialiased descender — the underline is the right-edge signal
/// we care about and the descender pixels never extend past it).
fn rightmost_dark_pixel_x(rgba: &[u8], width: u32, row: u32) -> Option<u32> {
    let row_start = (row * width * 4) as usize;
    for x in (0..width).rev() {
        let i = row_start + (x * 4) as usize;
        // Treat anything noticeably darker than the background as
        // "painted" — the underline uses pure black (0, 0, 0) so a
        // generous threshold gives the test some headroom against
        // alpha-blending noise we don't actually care about.
        if rgba[i] < 200 && rgba[i + 1] < 200 && rgba[i + 2] < 200 {
            return Some(x);
        }
    }
    None
}

/// Pick a pixel row that lies inside the underline band. The
/// renderer paints the underline at `y + font_size * 0.9` with a
/// thickness of `max(1, font_size / 16)`. For our font_size = 32 at
/// y = 10 that's row 38–39; we sample row 39 to be safely inside
/// the band.
const UNDERLINE_ROW: u32 = 39;

/// Identical text painted at two different `letter_spacing` values
/// must produce underlines whose right edges differ by exactly
/// `(n - 1) * delta_spacing` pixels. With `n = 4` characters and a
/// 5px increase, the spaced underline should reach `3 × 5 = 15` px
/// further to the right than the unspaced one.
#[test]
fn test_letter_spacing_extends_underline_width() {
    const TEXT: &str = "ABCD";
    const X: f32 = 10.0;
    const Y: f32 = 10.0;
    const FONT_SIZE: f32 = 32.0;
    const SPACING_DELTA: f32 = 5.0;
    let n = TEXT.chars().count();

    let mut r0 = make_renderer(400, 80);
    let mut rs = make_renderer(400, 80);

    r0.render(&underlined_drawtext(TEXT, X, Y, FONT_SIZE, 0.0));
    rs.render(&underlined_drawtext(TEXT, X, Y, FONT_SIZE, SPACING_DELTA));

    let x0 = rightmost_dark_pixel_x(r0.rgba_bytes(), 400, UNDERLINE_ROW)
        .expect("unspaced underline must paint at least one pixel");
    let xs = rightmost_dark_pixel_x(rs.rgba_bytes(), 400, UNDERLINE_ROW)
        .expect("spaced underline must paint at least one pixel");

    let expected_delta = (n - 1) as f32 * SPACING_DELTA;
    let actual_delta = xs as f32 - x0 as f32;
    // The underline is a solid `fill_rect` with hard pixel edges, so
    // the delta should be exact in principle. We allow ±1 to absorb
    // any fontdue-vs-renderer rounding seam at the right boundary.
    assert!(
        (actual_delta - expected_delta).abs() <= 1.0,
        "expected underline to extend by (n-1) × {SPACING_DELTA} = {expected_delta} px, got {actual_delta} px (unspaced x={x0}, spaced x={xs})",
    );
}

/// A single-character run must produce identical underlines at
/// every `letter_spacing` because `(n - 1) × spacing` is zero when
/// `n = 1`. This is the underflow guard in `text_width`'s
/// `saturating_sub(1)` made visible on the painted buffer.
#[test]
fn test_letter_spacing_single_char_underline_unaffected() {
    const X: f32 = 10.0;
    const Y: f32 = 10.0;
    const FONT_SIZE: f32 = 32.0;

    let mut r0 = make_renderer(200, 80);
    let mut rs = make_renderer(200, 80);

    r0.render(&underlined_drawtext("X", X, Y, FONT_SIZE, 0.0));
    rs.render(&underlined_drawtext("X", X, Y, FONT_SIZE, 10.0));

    let x0 = rightmost_dark_pixel_x(r0.rgba_bytes(), 200, UNDERLINE_ROW)
        .expect("unspaced underline must paint");
    let xs = rightmost_dark_pixel_x(rs.rgba_bytes(), 200, UNDERLINE_ROW)
        .expect("spaced underline must paint");

    assert!(
        x0.abs_diff(xs) <= 1,
        "single-char underline must not shift with letter_spacing; \
         unspaced x={x0}, spaced x={xs}",
    );
}
