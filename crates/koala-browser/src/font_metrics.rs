//! Font metrics backed by fontdue for accurate text measurement during layout.
//!
//! [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
//!
//! "CSS assumes that every font has font metrics that specify a
//! characteristic height above the baseline and a depth below it."

use fontdue::Font;
use koala_css::FontMetrics;

/// Font metrics implementation backed by fontdue's per-glyph metrics.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
///
/// This implementation queries fontdue for exact per-character advance
/// widths, providing accurate text measurement for layout. It uses
/// `Font::metrics()` (not `Font::rasterize()`) to avoid the cost of
/// bitmap generation when only measurements are needed.
pub struct FontdueFontMetrics<'a> {
    font: &'a Font,
}

impl<'a> FontdueFontMetrics<'a> {
    /// Create a new font metrics provider from a fontdue Font.
    #[must_use]
    pub const fn new(font: &'a Font) -> Self {
        Self { font }
    }
}

impl FontMetrics for FontdueFontMetrics<'_> {
    #[allow(clippy::cast_precision_loss)]
    fn text_width(&self, text: &str, font_size: f32, letter_spacing: f32) -> f32 {
        // Sum per-character advance widths, matching the cursor advancement
        // used in Renderer::draw_text (renderer.rs). Adds
        // `(n_chars - 1) * letter_spacing` between adjacent glyphs;
        // the count and the sum iterate the same control-filter chain
        // so the returned width matches what `draw_text` will actually
        // advance through.
        //
        // Uses Font::metrics() instead of Font::rasterize() to avoid
        // generating bitmaps when only measurements are needed.
        let mut sum: f32 = 0.0;
        let mut n: usize = 0;
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            sum += self.font.metrics(ch, font_size).advance_width;
            n += 1;
        }
        sum + n.saturating_sub(1) as f32 * letter_spacing
    }

    fn line_height(&self, font_size: f32) -> f32 {
        // [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
        //
        // "The initial value of 'line-height' is 'normal'. We recommend a used
        // value for 'normal' between 1.0 and 1.2."
        //
        // Use 1.2× as the line height ratio, matching common browser defaults.
        font_size * 1.2
    }
}
