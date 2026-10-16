//! Font metrics backed by fontdue for accurate text measurement during layout.
//!
//! [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
//!
//! "CSS assumes that every font has font metrics that specify a
//! characteristic height above the baseline and a depth below it."
//!
//! fontdue measures glyph advances. The font-wide metrics come from
//! `koala-font`, which reads them from the font's tables; this module is
//! where those tables are mapped onto what CSS asks for.

use fontdue::Font;
use koala_css::{ApproximateFontMetrics, DecorationMetrics, FontMetrics, VerticalFontMetrics};

/// A font's vertical metrics in ems, read once from its tables when it
/// loads, so measuring at a font size is a multiplication.
#[derive(Debug, Clone, Copy)]
pub struct EmMetrics {
    ascent: f32,
    descent: f32,
    line_gap: f32,
    underline: Option<DecorationMetrics>,
    strikeout: Option<DecorationMetrics>,
}

impl EmMetrics {
    /// Reads the metrics of face 0 of the font in `bytes`, the face
    /// fontdue opens with its default settings.
    ///
    /// # Errors
    ///
    /// Whatever stops `koala-font` opening the face.
    pub fn read(bytes: &[u8]) -> Result<Self, koala_font::FontError> {
        let font = koala_font::Font::from_source(bytes, koala_font::FontSettings::default())?;
        let em = font.scale_factor(1.0);

        // [§ 3.2.1 Ascent and Descent Metrics](https://drafts.csswg.org/css-inline-3/#ascent-descent)
        //
        // "Note: It is recommended that implementations that use OpenType
        // or TrueType fonts use the metrics sTypoAscender and
        // sTypoDescender from the font's OS/2 table (after scaling to the
        // current element's font size) to find the ascent metric and
        // descent metric for CSS layout. In the absence of these metrics,
        // the "Ascent" and "Descent" metrics from the HHEA table should be
        // used."
        let metrics = font.typo_metrics().unwrap_or_else(|| font.hhea_metrics());

        // [§ 3.2.2 Line Gap Metrics](https://drafts.csswg.org/css-inline-3/#font-line-gap)
        //
        // "Note: In OpenType, the line gap metric can be found as
        // sTypoLineGap or hhea.lineGap." It is taken from the same table
        // as the ascent and descent, so the three describe one font
        // designer's idea of a line.
        //
        // "UAs must floor the line gap metric at zero."
        let line_gap = metrics.line_gap.scaled(em).max(0.0);

        let in_ems = |line: koala_font::DecorationLine| DecorationMetrics {
            top: line.position.scaled(em),
            thickness: line.thickness.scaled(em),
        };
        Ok(Self {
            ascent: metrics.ascender.scaled(em),
            // OpenType descenders are negative below the baseline; CSS's
            // descent metric is a depth, positive below it.
            descent: -metrics.descender.scaled(em),
            line_gap,
            // `post.underlinePosition`: "Suggested y-coordinate of the top
            // of the underline."
            underline: font.underline().map(in_ems),
            // `OS/2.yStrikeoutPosition`: "The position of the top of the
            // strikeout stroke relative to the baseline".
            strikeout: font.strikeout().map(in_ems),
        })
    }

    /// These metrics at `font_size` pixels per em.
    fn at(self, font_size: f32) -> VerticalFontMetrics {
        let scaled = |line: DecorationMetrics| DecorationMetrics {
            top: line.top * font_size,
            thickness: line.thickness * font_size,
        };
        VerticalFontMetrics {
            ascent: self.ascent * font_size,
            descent: self.descent * font_size,
            line_gap: self.line_gap * font_size,
            underline: self.underline.map(scaled),
            strikeout: self.strikeout.map(scaled),
        }
    }
}

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
    /// `None` when `koala-font` could not read the font's tables; the
    /// vertical metrics then fall back to [`ApproximateFontMetrics`].
    em: Option<EmMetrics>,
}

impl<'a> FontdueFontMetrics<'a> {
    /// Create a new font metrics provider from a fontdue Font and the
    /// metrics `koala-font` read from the same font.
    #[must_use]
    pub const fn new(font: &'a Font, em: Option<EmMetrics>) -> Self {
        Self { font, em }
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

    fn vertical_metrics(&self, font_size: f32) -> VerticalFontMetrics {
        self.em.map_or_else(
            || ApproximateFontMetrics.vertical_metrics(font_size),
            |em| em.at(font_size),
        )
    }
}
