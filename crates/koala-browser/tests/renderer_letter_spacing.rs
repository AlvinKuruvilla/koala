//! Render-layer verification for `letter-spacing`.
//!
//! [CSS Text Module Level 3 § 9.3](https://www.w3.org/TR/css-text-3/#letter-spacing-property)
//!
//! The layout pipeline produces a `DisplayCommand::DrawText` whose
//! `letter_spacing` field tells the renderer to advance the glyph
//! cursor by `glyph_advance + letter_spacing` between adjacent
//! characters (and only between — not after the last). Where the last
//! glyph's ink ends is a direct, pixel-readable signal of the cursor's
//! final position.
//!
//! This test paints two runs of identical text with two different
//! `letter_spacing` values, finds the rightmost inked pixel in each, and
//! asserts the difference matches `(n - 1) * delta_spacing`. That's the
//! same `(n - 1) * letter_spacing` term the layout-side
//! `FontMetrics::text_width` adds, and the display list sizes a run's
//! decoration lines by that width, so a passing test confirms the
//! glyphs end where their underline does.

use koala_std::collections::HashMap;
use std::sync::Arc;

use fontdue::{Font, FontSettings};
use koala_browser::{Renderer, RendererFonts};
use koala_css::{ColorValue, DisplayCommand, DisplayList, FontStyle};

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

/// Construct a `DisplayList` with exactly one `DrawText` command on
/// the baseline at `baseline`, at the given `letter_spacing`. All other
/// text properties are held constant so the only signal in the painted
/// buffer is the spacing.
fn drawtext(
    text: &str,
    x: f32,
    baseline: f32,
    font_size: f32,
    letter_spacing: f32,
) -> DisplayList {
    let mut list = DisplayList::new();
    list.push(DisplayCommand::DrawText {
        x,
        y: baseline,
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
        letter_spacing,
    });
    list
}

/// The rightmost column holding any inked pixel. The buffer is
/// initialized to opaque white in `allocate_buffer`, and the text is
/// pure black, so anything noticeably darker than the background is
/// glyph ink; the generous threshold gives headroom against
/// antialiasing we don't care about.
fn rightmost_ink_x(rgba: &[u8], width: u32) -> Option<u32> {
    rgba.chunks_exact(4)
        .enumerate()
        .filter(|(_, pixel)| pixel[0] < 200 && pixel[1] < 200 && pixel[2] < 200)
        .map(|(index, _)| index as u32 % width)
        .max()
}

/// The baseline every test draws on: low enough in the 80px canvas
/// that a 32px font's ascenders stay inside it.
const BASELINE: f32 = 50.0;

/// Identical text painted at two different `letter_spacing` values
/// must end `(n - 1) * delta_spacing` pixels apart. With `n = 4`
/// characters and a 5px increase, the spaced text's last glyph should
/// end `3 × 5 = 15` px further to the right than the unspaced one's.
#[test]
fn test_letter_spacing_moves_the_last_glyph() {
    const TEXT: &str = "ABCD";
    const X: f32 = 10.0;
    const FONT_SIZE: f32 = 32.0;
    const SPACING_DELTA: f32 = 5.0;
    let n = TEXT.chars().count();

    let mut r0 = make_renderer(400, 80);
    let mut rs = make_renderer(400, 80);

    r0.render(&drawtext(TEXT, X, BASELINE, FONT_SIZE, 0.0));
    rs.render(&drawtext(TEXT, X, BASELINE, FONT_SIZE, SPACING_DELTA));

    let x0 = rightmost_ink_x(r0.rgba_bytes(), 400).expect("unspaced text must paint");
    let xs = rightmost_ink_x(rs.rgba_bytes(), 400).expect("spaced text must paint");

    let expected_delta = (n - 1) as f32 * SPACING_DELTA;
    let actual_delta = xs as f32 - x0 as f32;
    // The spacing is a whole number of pixels, so the last glyph moves
    // by exactly that much in principle. We allow ±1 to absorb any
    // fontdue-vs-renderer rounding seam.
    assert!(
        (actual_delta - expected_delta).abs() <= 1.0,
        "expected the text to extend by (n-1) × {SPACING_DELTA} = {expected_delta} px, got {actual_delta} px (unspaced x={x0}, spaced x={xs})",
    );
}

/// A single-character run must paint identically at every
/// `letter_spacing` because `(n - 1) × spacing` is zero when `n = 1`.
/// This is the underflow guard in `text_width`'s `saturating_sub(1)`
/// made visible on the painted buffer.
#[test]
fn test_letter_spacing_single_char_unaffected() {
    const X: f32 = 10.0;
    const FONT_SIZE: f32 = 32.0;

    let mut r0 = make_renderer(200, 80);
    let mut rs = make_renderer(200, 80);

    r0.render(&drawtext("X", X, BASELINE, FONT_SIZE, 0.0));
    rs.render(&drawtext("X", X, BASELINE, FONT_SIZE, 10.0));

    let x0 = rightmost_ink_x(r0.rgba_bytes(), 200).expect("unspaced text must paint");
    let xs = rightmost_ink_x(rs.rgba_bytes(), 200).expect("spaced text must paint");

    assert!(
        x0.abs_diff(xs) <= 1,
        "a single character must not move with letter_spacing; \
         unspaced x={x0}, spaced x={xs}",
    );
}
