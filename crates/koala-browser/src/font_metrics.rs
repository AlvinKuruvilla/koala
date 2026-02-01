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
    pub fn new(font: &'a Font) -> Self {
        Self { font }
    }
}

impl FontMetrics for FontdueFontMetrics<'_> {
    fn text_width(&self, text: &str, font_size: f32) -> f32 {
        // Sum per-character advance widths, matching the cursor advancement
        // used in Renderer::draw_text (renderer.rs).
        //
        // Uses Font::metrics() instead of Font::rasterize() to avoid
        // generating bitmaps when only measurements are needed.
        text.chars()
            .filter(|ch| !ch.is_control())
            .map(|ch| self.font.metrics(ch, font_size).advance_width)
            .sum()
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
