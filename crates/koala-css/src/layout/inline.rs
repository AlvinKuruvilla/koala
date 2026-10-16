//! CSS Inline Layout and Line Box Model.
//!
//! [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
//!
//! "In an inline formatting context, boxes are laid out horizontally, one
//! after the other, beginning at the top of a containing block. Horizontal
//! margins, borders, and padding are respected between these boxes."
//!
//! [§ 10.8 Line height calculations: the 'line-height' and 'vertical-align' properties](https://www.w3.org/TR/CSS2/visudet.html#line-height)
//!
//! "The height of the line box is determined by the rules given in the
//! section on line height calculations."

use crate::style::ColorValue;
use crate::style::values::{FontStyle, TextAlign, TextDecorationLine};

use super::box_model::{EdgeSizes, Rect};
use super::layout_box::ElementBoxId;

/// A font's ascent as a fraction of its em square: 0.8, close to typical
/// Latin fonts (Helvetica ≈ 0.77, Arial ≈ 0.81).
///
/// Used only where no font says otherwise: by [`ApproximateFontMetrics`],
/// and for the baseline of atomic inlines (see `fragment_ascent_descent`).
const ASCENDER_RATIO: f32 = 0.8;

/// Font metrics interface for text measurement during layout.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
///
/// Implementors provide the actual per-glyph advance widths and line
/// height values needed for inline layout. The layout engine calls
/// these methods to measure text for line breaking and fragment placement.
pub trait FontMetrics {
    /// Measure the total advance width of a text string at the given font size,
    /// including any inter-character `letter_spacing` (in px).
    ///
    /// The returned value sums per-glyph advance widths plus
    /// `(n_chars - 1) * letter_spacing` — i.e. spacing applies
    /// *between* adjacent characters, not after the last. An empty
    /// or single-character string contributes no spacing. Negative
    /// `letter_spacing` is allowed and may produce a total smaller
    /// than the sum of glyph widths (or even negative); callers
    /// must not clamp.
    fn text_width(&self, text: &str, font_size: f32, letter_spacing: f32) -> f32;

    /// Calculate the line height for a given font size.
    ///
    /// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
    ///
    /// "The initial value of 'line-height' is 'normal'. We recommend a used
    /// value for 'normal' between 1.0 and 1.2."
    fn line_height(&self, font_size: f32) -> f32;

    /// The font's own vertical metrics at `font_size`, in pixels.
    ///
    /// [§ 3.2.1 Ascent and Descent Metrics](https://drafts.csswg.org/css-inline-3/#ascent-descent)
    ///
    /// "CSS assumes that every font has font metrics that specify a
    /// characteristic height above the baseline—called the ascent
    /// metric—and a characteristic depth below it—called the descent
    /// metric—which CSS uses for laying out text and boxes in an inline
    /// formatting context."
    ///
    /// Which of a font's tables supply these is the implementor's
    /// business: this trait is where font formats stop and CSS starts.
    fn vertical_metrics(&self, font_size: f32) -> VerticalFontMetrics;
}

/// A font's vertical metrics at one font size, in pixels.
///
/// [§ 3.2 Font and Glyph Metrics](https://drafts.csswg.org/css-inline-3/#font-metrics)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerticalFontMetrics {
    /// "a characteristic height above the baseline—called the ascent
    /// metric".
    pub ascent: f32,
    /// "a characteristic depth below it—called the descent metric",
    /// positive below the baseline.
    pub descent: f32,
    /// [§ 3.2.2 Line Gap Metrics](https://drafts.csswg.org/css-inline-3/#font-line-gap)
    ///
    /// "a font-recommended “line gap” or “external leading” metric".
    /// "UAs must floor the line gap metric at zero", so this is never
    /// negative.
    pub line_gap: f32,
    /// The font's suggested underline, if it gives one.
    ///
    /// [§ 2.8.2 Using Font Metrics for Automatic Positioning](https://drafts.csswg.org/css-text-decor-4/#line-auto-offset)
    ///
    /// "Some font formats (such as OpenType) can offer information about
    /// the appropriate position of a line decoration."
    pub underline: Option<DecorationMetrics>,
    /// The font's suggested line-through, if it gives one.
    pub strikeout: Option<DecorationMetrics>,
}

/// Where a font places one of its decoration lines, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecorationMetrics {
    /// Height of the line's top edge above the baseline; negative below
    /// it, as an underline usually is.
    pub top: f32,
    /// The line's thickness.
    pub thickness: f32,
}

/// Approximate font metrics using fixed ratios.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
///
/// Implementation note: Without access to actual font data, we use fixed
/// ratio approximations. The average advance width of Latin glyphs in a
/// proportional font is approximately 0.6× the font size (typical for
/// Helvetica/Arial body text). Line height uses 1.2×, the upper end of
/// the spec's recommended range for `line-height: normal`.
///
/// This is used as a fallback when no font is available, and in tests.
pub struct ApproximateFontMetrics;

impl FontMetrics for ApproximateFontMetrics {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn text_width(&self, text: &str, font_size: f32, letter_spacing: f32) -> f32 {
        const CHAR_WIDTH_RATIO: f32 = 0.6;
        let n = text.chars().count();
        n as f32 * font_size * CHAR_WIDTH_RATIO
            + n.saturating_sub(1) as f32 * letter_spacing
    }

    fn line_height(&self, font_size: f32) -> f32 {
        const LINE_HEIGHT_RATIO: f32 = 1.2;
        font_size * LINE_HEIGHT_RATIO
    }

    /// The ascent is `ASCENDER_RATIO` of the em and the descent the rest,
    /// with no line gap; the font is not known, so neither is where it
    /// would put its decorations.
    fn vertical_metrics(&self, font_size: f32) -> VerticalFontMetrics {
        VerticalFontMetrics {
            ascent: font_size * ASCENDER_RATIO,
            descent: font_size * (1.0 - ASCENDER_RATIO),
            line_gap: 0.0,
            underline: None,
            strikeout: None,
        }
    }
}

/// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
///
/// "The rectangular area that contains the boxes that form a line is called
/// a line box."
///
/// "The width of a line box is determined by a containing block and the
/// presence of floats."
///
/// "The height of a line box is determined by the rules given in the
/// section on line height calculations."
#[derive(Debug, Clone)]
pub struct LineBox {
    /// The bounding rectangle of this line box.
    pub bounds: Rect,

    /// Fragments laid out on this line.
    pub fragments: Vec<LineFragment>,

    /// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
    ///
    /// "The height of the line box is the distance between the uppermost
    /// box top and the lowermost box bottom."
    pub line_height: f32,

    /// The baseline position relative to the line box top.
    ///
    /// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
    ///
    /// "CSS assumes that every font has font metrics that specify a
    /// characteristic height above the baseline and a depth below it."
    pub baseline: f32,
}

/// A fragment of content placed on a line.
///
/// [§ 9.2.1.1 Anonymous inline boxes](https://www.w3.org/TR/CSS2/visuren.html#anonymous-inline)
///
/// "When an inline box contains an in-flow block-level box, the inline box
/// (and its inline ancestors within the same line box) are broken around
/// the block-level box... splitting it into two boxes."
///
/// A fragment represents a piece of an inline box that has been placed on
/// a specific line. One inline box may produce multiple fragments if it
/// wraps across lines.
#[derive(Debug, Clone)]
pub struct LineFragment {
    /// The position and size of this fragment on the line.
    pub bounds: Rect,

    /// The content of this fragment.
    pub content: FragmentContent,

    /// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
    ///
    /// "The 'vertical-align' property affects the vertical positioning
    /// inside a line box of the boxes generated by an inline-level element."
    pub vertical_align: VerticalAlign,
}

/// The content of a line fragment.
#[derive(Debug, Clone)]
pub enum FragmentContent {
    /// A run of text.
    Text(TextRun),
    /// [§ 2 Fragmentation Model and Terminology](https://www.w3.org/TR/css-break-3/#box-fragment)
    ///
    /// "box fragment: The portion of a box that belongs to exactly one
    /// fragmentainer. ... Each fragment has its own share of the box's
    /// border, padding, and margin"
    ///
    /// "Breaking inline content into lines is another form of fragmentation,
    /// and similarly creates box fragments when it breaks inline boxes
    /// across line boxes."
    ///
    /// A box fragment of a non-replaced inline box (`<a>`, `<span>`);
    /// `bounds` is its border box. It precedes its contents in the line's
    /// fragments, so the last one containing a point is the innermost box.
    InlineBox {
        /// The inline box this is a fragment of.
        id: ElementBoxId,
        /// Distance from the top of the border box to the baseline.
        ascent: f32,
    },
    /// A replaced inline element (e.g., `<img>`).
    ReplacedElement,
    /// [§ 9.2.4 Atomic inline-level boxes](https://www.w3.org/TR/css-display-3/#atomic-inline)
    ///
    /// "An inline-level box that is not an inline box (such as replaced
    /// inline-level elements, inline-block elements, and inline-table
    /// elements) is called an atomic inline-level box because it
    /// participates in its inline formatting context as a single opaque box."
    ///
    /// Stores which box it is so the corresponding child `LayoutBox` can be
    /// repositioned after line finalization.
    InlineBlock(ElementBoxId),
}

/// A contiguous run of text within a line fragment.
///
/// [§ 2.5 Text Runs](https://www.w3.org/TR/css-display-3/#text-nodes)
///
/// "A text run is a maximal sequence of consecutive text nodes."
#[derive(Debug, Clone)]
pub struct TextRun {
    /// The text content of this run.
    pub text: String,
    /// The width of this text run in pixels (computed from font metrics).
    pub width: f32,
    /// Font size in pixels.
    pub font_size: f32,
    /// [§ 3.1 'color'](https://www.w3.org/TR/css-color-4/#the-color-property)
    ///
    /// "This property describes the foreground color of an element's text content."
    ///
    /// The inherited text color for this run.
    pub color: ColorValue,
    /// [§ 3.2 'font-weight'](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
    ///
    /// Numeric weight (400 = normal, 700 = bold).
    pub font_weight: u16,
    /// [§ 3.3 'font-style'](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
    pub font_style: FontStyle,
    /// [§ 3 'text-decoration-line'](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
    pub text_decoration: TextDecorationLine,

    /// [§ 9.3 'letter-spacing'](https://www.w3.org/TR/css-text-3/#letter-spacing-property)
    ///
    /// Inter-character spacing in pixels, used by the renderer to
    /// advance the cursor by `glyph_advance + letter_spacing` for
    /// every glyph except the last in the run.
    pub letter_spacing: f32,

    /// The run's font metrics at its font size, kept so painting can
    /// place decorations without measuring the font again.
    pub vertical_metrics: VerticalFontMetrics,

    /// Distance from the top of the fragment to the run's baseline: A' in
    /// [§ 10.8.1](https://www.w3.org/TR/CSS2/visudet.html#leading), the
    /// font's ascent plus half the leading. See [`baseline_offset`].
    pub baseline: f32,
}

/// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
///
/// The distance from the top of a glyph's inline box to its baseline,
/// for a font with `metrics` in a box `line_height` tall.
///
/// "In this section we use A to mean that height (for a given font at a
/// given size) and D the depth. We also define AD = A + D, the distance
/// from the top to the bottom."
fn baseline_offset(metrics: VerticalFontMetrics, line_height: f32) -> f32 {
    // "Still for each glyph, determine the leading L to add, where L =
    // 'line-height' - AD."
    let leading = line_height - (metrics.ascent + metrics.descent);
    // "Half the leading is added above A and the other half below D,
    // giving the glyph and its leading a total height above the baseline
    // of A' = A + L/2 and a total depth of D' = D + L/2."
    //
    // "Note. L may be negative."
    leading.mul_add(0.5, metrics.ascent)
}

/// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
///
/// "The 'vertical-align' property affects the vertical positioning inside
/// a line box of the boxes generated by an inline-level element."
///
/// "Values for this property have different meanings in the context of
/// tables. Please consult the section on vertical alignment in tables
/// for details."
///
/// "Values (for inline elements) have the following meanings:
///
/// baseline
///   Align the baseline of the box with the baseline of the parent box.
///
/// middle
///   Align the vertical midpoint of the box with the baseline of the
///   parent box plus half the x-height of the parent.
///
/// sub
///   Lower the baseline of the box to the proper position for subscripts.
///
/// super
///   Raise the baseline of the box to the proper position for superscripts.
///
/// text-top
///   Align the top of the box with the top of the parent's content area.
///
/// text-bottom
///   Align the bottom of the box with the bottom of the parent's content area.
///
/// top
///   Align the top of the aligned subtree with the top of the line box.
///
/// bottom
///   Align the bottom of the aligned subtree with the bottom of the line box.
///
/// `<percentage>`
///   Raise (positive) or lower (negative) the box by this distance
///   (a percentage of the 'line-height' value).
///
/// `<length>`
///   Raise (positive) or lower (negative) the box by this distance."
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum VerticalAlign {
    /// "Align the baseline of the box with the baseline of the parent box."
    #[default]
    Baseline,
    /// "Align the vertical midpoint of the box with the baseline plus half x-height."
    Middle,
    /// "Lower the baseline of the box for subscripts."
    Sub,
    /// "Raise the baseline of the box for superscripts."
    Super,
    /// "Align the top with the top of the parent's content area."
    TextTop,
    /// "Align the bottom with the bottom of the parent's content area."
    TextBottom,
    /// "Align the top of the aligned subtree with the top of the line box."
    Top,
    /// "Align the bottom of the aligned subtree with the bottom of the line box."
    Bottom,
    /// Offset from baseline by a specific pixel amount.
    Length(f32),
}

/// Inline formatting context that manages line box construction.
///
/// [§ 9.4.2 Inline formatting contexts](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
///
/// "In an inline formatting context, boxes are laid out horizontally, one
/// after the other, beginning at the top of a containing block."
pub struct InlineLayout {
    /// Completed line boxes.
    pub line_boxes: Vec<LineBox>,
    /// Fragments accumulating on the current line.
    pub current_line_fragments: Vec<LineFragment>,
    /// Current X position on the current line.
    pub current_x: f32,
    /// Current line's Y position (top of line box).
    pub current_y: f32,
    /// Maximum width available for line boxes.
    pub available_width: f32,
    /// Maximum height seen on the current line (for line box height).
    pub current_line_max_height: f32,
    /// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
    ///
    /// Text alignment for this inline formatting context, inherited from
    /// the block container that established it.
    pub text_align: TextAlign,

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// Absolute X coordinate of the containing block's content edge.
    /// All fragment X positions are offset by this amount so they are
    /// in absolute coordinates (matching `current_y` which is already
    /// absolute).
    pub start_x: f32,

    /// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
    ///
    /// "The current and subsequent line boxes created next to the float
    /// are shortened as necessary to make room for the margin box of the
    /// float."
    ///
    /// Horizontal offset from the content box left edge caused by left
    /// floats. Fragment x positions are shifted by this amount.
    pub left_offset: f32,

    /// [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
    ///
    /// When true, suppresses soft line breaks (text wrapping).
    /// Set when `white-space` is `nowrap` or `pre`.
    pub no_wrap: bool,

    /// [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
    ///
    /// "This value prevents user agents from collapsing sequences of white
    /// space. Lines are only broken at preserved newline characters."
    ///
    /// When true, newline characters (`\n`) in text force line breaks instead
    /// of being collapsed. Set when `white-space` is `pre`, `pre-wrap`, or
    /// `pre-line`.
    pub preserve_newlines: bool,

    /// Inline boxes begun and not yet ended, outermost first.
    open_inline_boxes: Vec<OpenInlineBox>,
}

/// An inline box whose fragment on the current line is still growing.
struct OpenInlineBox {
    id: ElementBoxId,
    /// Index of its fragment in `current_line_fragments`.
    fragment: usize,
    border_padding_right: f32,
    margin_right: f32,
    height: f32,
    ascent: f32,
}

impl InlineLayout {
    /// Create a new inline layout context.
    #[must_use]
    pub const fn new(
        available_width: f32,
        start_x: f32,
        start_y: f32,
        text_align: TextAlign,
    ) -> Self {
        Self {
            line_boxes: Vec::new(),
            current_line_fragments: Vec::new(),
            current_x: 0.0,
            current_y: start_y,
            available_width,
            current_line_max_height: 0.0,
            text_align,
            start_x,
            left_offset: 0.0,
            no_wrap: false,
            preserve_newlines: false,
            open_inline_boxes: Vec::new(),
        }
    }

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// Add a text run to the inline formatting context.
    ///
    /// "In an inline formatting context, boxes are laid out horizontally, one
    /// after the other, beginning at the top of a containing block."
    ///
    /// "When an inline box exceeds the width of a line box, it is split into
    /// several boxes and these boxes are distributed across several line boxes."
    #[allow(clippy::too_many_arguments)]
    pub fn add_text(
        &mut self,
        text: &str,
        font_size: f32,
        color: &ColorValue,
        font_weight: u16,
        font_style: FontStyle,
        text_decoration: TextDecorationLine,
        letter_spacing: f32,
        font_metrics: &dyn FontMetrics,
    ) {
        // STEP 0: Handle preserved newlines.
        // [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
        //
        // "If 'white-space' is set to 'pre', 'pre-wrap', or 'pre-line',
        // line breaks are forced at preserved newline characters."
        //
        // [§ 4.1.1 Phase I: Collapsing and Transformation](https://www.w3.org/TR/css-text-3/#white-space-phase-1)
        //
        // "A preserved newline forces a line break."
        if self.preserve_newlines
            && let Some(nl_pos) = text.find('\n')
        {
                // Place text before the newline on the current line.
                let before = &text[..nl_pos];
                if !before.is_empty() {
                    let lh = font_metrics.line_height(font_size);
                    self.place_text_fragment(
                        before,
                        font_size,
                        lh,
                        color,
                        font_weight,
                        font_style,
                        text_decoration,
                        letter_spacing,
                        font_metrics,
                    );
                }
                // Force line break at the newline character.
                self.force_line_break(font_size, font_metrics);
                // Continue with text after the newline.
                let after = &text[nl_pos + 1..];
                if !after.is_empty() {
                    self.add_text(
                        after,
                        font_size,
                        color,
                        font_weight,
                        font_style,
                        text_decoration,
                        letter_spacing,
                        font_metrics,
                    );
                }
                return;
        }

        // STEP 1: Measure the text width.
        // [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
        //
        // "CSS assumes that every font has font metrics that specify a
        // characteristic height above the baseline and a depth below it."
        //
        // The width comes from summing per-glyph advance widths via FontMetrics.
        // The height contribution is the line-height from FontMetrics.
        let text_width = font_metrics.text_width(text, font_size, letter_spacing);
        let line_height = font_metrics.line_height(font_size);

        // STEP 2: Check if text fits on the current line.
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "When the total width of the inline-level boxes on a line is less
        // than the width of the line box containing them, their horizontal
        // distribution within the line box is determined by the 'text-align'
        // property of the inline box."
        //
        // "When an inline box exceeds the width of a line box, it is split
        // into several boxes and these boxes are distributed across several
        // line boxes."
        // [§ 16.6 'white-space'](https://www.w3.org/TR/CSS2/text.html#white-space-prop)
        //
        // "This value collapses white space as for 'normal', but suppresses
        // line breaks (text wrapping) within text."
        //
        // When no_wrap is true, text always fits on the current line
        // (no soft wrapping occurs).
        let fits_on_current_line = self.no_wrap
            || self.current_x + text_width <= self.available_width
            || self.current_x == 0.0;

        if !fits_on_current_line {
            // STEP 3: Handle line breaking.
            // [§ 5.5.1 Line Breaking Details](https://www.w3.org/TR/css-text-3/#line-breaking)
            //
            // "A line break is forced at a preserved newline."
            //
            // [§ 5.5.2 Word Breaking Rules](https://www.w3.org/TR/css-text-3/#word-breaking)
            //
            // Try to find a soft wrap opportunity that fits on the current line.
            let remaining_width = self.available_width - self.current_x;

            if let Some(break_idx) = Self::find_break_opportunity(
                text,
                remaining_width,
                font_size,
                letter_spacing,
                font_metrics,
            ) {
                // Split at the break point: place the first part on the
                // current line, then recurse for the remainder.
                let (first, rest) = text.split_at(break_idx);

                // Trim trailing whitespace from the first part per spec:
                // [§ 4.1.3](https://www.w3.org/TR/css-text-3/#white-space-phase-2)
                // "A sequence of collapsible spaces at the end of a line is removed."
                let first_trimmed = first.trim_end();
                if !first_trimmed.is_empty() {
                    self.place_text_fragment(
                        first_trimmed,
                        font_size,
                        line_height,
                        color,
                        font_weight,
                        font_style,
                        text_decoration,
                        letter_spacing,
                        font_metrics,
                    );
                }

                // Finalize this line and start a new one.
                self.finish_line();

                // Trim leading whitespace from the remainder per spec:
                // "A sequence of collapsible spaces at the beginning of a line is removed."
                let rest_trimmed = rest.trim_start();
                if !rest_trimmed.is_empty() {
                    self.add_text(
                        rest_trimmed,
                        font_size,
                        color,
                        font_weight,
                        font_style,
                        text_decoration,
                        letter_spacing,
                        font_metrics,
                    );
                }
                return;
            }

            // No break opportunity found that fits — wrap the entire text
            // to a new line. If the line is not empty, finish it first.
            // The `current_x == 0.0` guard in `fits_on_current_line` above
            // prevents infinite recursion: on a fresh line we always place
            // the text even if it overflows.
            self.finish_line();
            self.add_text(
                text,
                font_size,
                color,
                font_weight,
                font_style,
                text_decoration,
                letter_spacing,
                font_metrics,
            );
            return;
        }

        // STEP 4: Place fragment on the current line.
        self.place_text_fragment(
            text,
            font_size,
            line_height,
            color,
            font_weight,
            font_style,
            text_decoration,
            letter_spacing,
            font_metrics,
        );
    }

    /// Place a text fragment at the current position on the current line.
    ///
    /// This is the shared placement logic used by `add_text` after measurement
    /// and line-breaking decisions have been made.
    #[allow(clippy::too_many_arguments)]
    fn place_text_fragment(
        &mut self,
        text: &str,
        font_size: f32,
        line_height: f32,
        color: &ColorValue,
        font_weight: u16,
        font_style: FontStyle,
        text_decoration: TextDecorationLine,
        letter_spacing: f32,
        font_metrics: &dyn FontMetrics,
    ) {
        let text_width = font_metrics.text_width(text, font_size, letter_spacing);
        let vertical_metrics = font_metrics.vertical_metrics(font_size);

        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "In an inline formatting context, boxes are laid out horizontally,
        // one after the other, beginning at the top of a containing block."
        // [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
        //
        // Fragment x is offset by left_offset to account for left floats
        // intruding into the line box.
        let fragment = LineFragment {
            bounds: Rect {
                x: self.start_x + self.left_offset + self.current_x,
                y: self.current_y,
                width: text_width,
                height: line_height,
            },
            content: FragmentContent::Text(TextRun {
                text: text.to_string(),
                width: text_width,
                font_size,
                color: color.clone(),
                font_weight,
                font_style,
                text_decoration,
                letter_spacing,
                vertical_metrics,
                baseline: baseline_offset(vertical_metrics, line_height),
            }),
            vertical_align: VerticalAlign::Baseline,
        };
        self.current_line_fragments.push(fragment);

        // STEP 5: Update current position.
        self.current_x += text_width;
        if line_height > self.current_line_max_height {
            self.current_line_max_height = line_height;
        }
    }

    /// [§ 9.2.4 Atomic inline-level boxes](https://www.w3.org/TR/css-display-3/#atomic-inline)
    ///
    /// Add an atomic inline-level block container (display: inline-block)
    /// to the current line.
    ///
    /// [§ 10.3.9 'Inline-block', non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#inlineblock-width)
    ///
    /// "Inline-block elements participate in their parent's inline formatting
    /// context as a single opaque box."
    ///
    /// Unlike regular inline boxes, inline-blocks are atomic — they cannot
    /// be split across lines.
    pub fn add_inline_block(&mut self, box_id: ElementBoxId, width: f32, height: f32) {
        // STEP 1: Check if the inline-block fits on the current line.
        //
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "When an inline box exceeds the width of a line box, it is split
        // into several boxes..."
        //
        // Inline-blocks are atomic and cannot split; if they don't fit
        // and we're not at the start of a line, wrap to a new line.
        let fits_on_current_line =
            self.current_x + width <= self.available_width || self.current_x == 0.0;

        if !fits_on_current_line {
            self.finish_line();
        }

        // STEP 2: Place the inline-block fragment on the current line.
        let fragment = LineFragment {
            bounds: Rect {
                x: self.start_x + self.left_offset + self.current_x,
                y: self.current_y,
                width,
                height,
            },
            content: FragmentContent::InlineBlock(box_id),
            vertical_align: VerticalAlign::Baseline,
        };
        self.current_line_fragments.push(fragment);

        // STEP 3: Advance current position and update line height.
        self.current_x += width;
        if height > self.current_line_max_height {
            self.current_line_max_height = height;
        }
    }

    /// [§ 9.2.2 Inline-level elements and inline boxes](https://www.w3.org/TR/CSS2/visuren.html#inline-boxes)
    ///
    /// Begin a non-replaced inline box (e.g., `<span>`, `<a>`, `<em>`).
    ///
    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    ///
    /// "Horizontal margins, borders, and padding are respected between
    /// these boxes."
    ///
    /// `border_padding` is the box's border plus padding on each side.
    /// Everything laid out until the matching `end_inline_box` is the
    /// box's content.
    ///
    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// "When an inline box exceeds the width of a line box, it is split
    /// into several boxes and these boxes are distributed across several
    /// line boxes."
    ///
    /// If the left edge does not fit on the current line and the line
    /// is not empty, the current line is finished first.
    pub fn begin_inline_box(
        &mut self,
        id: ElementBoxId,
        font_size: f32,
        margin_left: f32,
        margin_right: f32,
        border_padding: EdgeSizes,
        font_metrics: &dyn FontMetrics,
    ) {
        let left_mbp = margin_left + border_padding.left;

        // STEP 1: Check if the left margin+border+padding fits on the
        // current line.
        //
        // If the line already has content and adding just the opening
        // edge would overflow, wrap to a new line. The `current_x == 0.0`
        // guard prevents infinite wrapping on an empty line.
        if self.current_x + left_mbp > self.available_width && self.current_x > 0.0 {
            self.finish_line();
        }

        // STEP 2: Start the box's fragment on this line at its left border
        // edge.
        //
        // [§ 10.6.1 Inline, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-non-replaced)
        //
        // "The height of the content area should be based on the font, but
        // this specification does not specify how. A UA may, e.g., use the
        // em-box or the maximum ascender and descender of the font."
        //
        // The maximum ascender and descender: the content area is the
        // font's A + D tall, with the baseline A below its top, so the
        // baseline sits where the box's text puts it. The em-box would
        // leave the baseline's place inside the box undefined.
        //
        // "The vertical padding, border and margin of an inline, non-replaced
        // box start at the top and bottom of the content area, and has
        // nothing to do with the 'line-height'."
        //
        // The vertical margin is outside the border box, so it is not part
        // of the fragment.
        let metrics = font_metrics.vertical_metrics(font_size);
        let height = border_padding.top + metrics.ascent + metrics.descent + border_padding.bottom;
        let ascent = border_padding.top + metrics.ascent;
        self.open_inline_boxes.push(OpenInlineBox {
            id,
            fragment: self.current_line_fragments.len(),
            border_padding_right: border_padding.right,
            margin_right,
            height,
            ascent,
        });
        self.current_line_fragments.push(LineFragment {
            bounds: Rect {
                x: self.start_x + self.left_offset + self.current_x + margin_left,
                y: self.current_y,
                width: 0.0,
                height,
            },
            content: FragmentContent::InlineBox { id, ascent },
            vertical_align: VerticalAlign::Baseline,
        });

        // STEP 3: Advance current_x by the left margin+border+padding.
        //
        // Content placed after this call will start after the left edge
        // decoration of the inline box.
        self.current_x += left_mbp;
    }

    /// [§ 9.2.2 Inline-level elements and inline boxes](https://www.w3.org/TR/CSS2/visuren.html#inline-boxes)
    ///
    /// End the innermost open non-replaced inline box.
    ///
    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    ///
    /// "Horizontal margins, borders, and padding are respected between
    /// these boxes."
    ///
    /// # Panics
    ///
    /// If no inline box is open.
    pub fn end_inline_box(&mut self) {
        let open = self
            .open_inline_boxes
            .pop()
            .expect("end_inline_box pairs with a begin_inline_box");

        // STEP 1: Close the box's fragment on this line at its right border
        // edge.
        let right_border_edge =
            self.start_x + self.left_offset + self.current_x + open.border_padding_right;
        let fragment = &mut self.current_line_fragments[open.fragment];
        fragment.bounds.width = right_border_edge - fragment.bounds.x;

        // STEP 2: Advance current_x by the right border+padding+margin.
        //
        // Content placed after this call will start after the right edge
        // decoration of the inline box.
        self.current_x += open.border_padding_right + open.margin_right;
    }

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// "When an inline box is split, margins, borders, and padding have no
    /// visual effect where the split occurs (or at any split, when there
    /// are several)."
    ///
    /// Close every open box's fragment at the end of the current line,
    /// with no right edge. Called before the line is finished.
    fn split_open_inline_boxes(&mut self) {
        let line_end = self.start_x + self.left_offset + self.current_x;
        for open in &self.open_inline_boxes {
            let fragment = &mut self.current_line_fragments[open.fragment];
            fragment.bounds.width = line_end - fragment.bounds.x;
        }
    }

    /// Continue every open box on the new current line, with no left edge
    /// (see `split_open_inline_boxes`). Called once the line is empty.
    fn continue_open_inline_boxes(&mut self) {
        debug_assert!(self.current_line_fragments.is_empty());
        let line_start = self.start_x + self.left_offset + self.current_x;
        for open in &mut self.open_inline_boxes {
            open.fragment = self.current_line_fragments.len();
            self.current_line_fragments.push(LineFragment {
                bounds: Rect {
                    x: line_start,
                    y: self.current_y,
                    width: 0.0,
                    height: open.height,
                },
                content: FragmentContent::InlineBox {
                    id: open.id,
                    ascent: open.ascent,
                },
                vertical_align: VerticalAlign::Baseline,
            });
        }
    }

    /// Whether the current line holds anything besides inline-box
    /// fragments.
    ///
    /// Inline-box fragments alone do not make a line box. A box that wraps
    /// gets a fragment on the next line before any of its content does;
    /// if what wrapped was only collapsible white space, that line must
    /// not exist.
    ///
    /// TODO: § 9.4.2 does keep a line holding an inline box with non-zero
    /// margins, padding or borders; see SESSION.md.
    fn line_has_content(&self) -> bool {
        self.current_line_fragments
            .iter()
            .any(|fragment| !matches!(fragment.content, FragmentContent::InlineBox { .. }))
    }

    /// [§ 15.3.8 The br element](https://html.spec.whatwg.org/multipage/rendering.html#the-br-element)
    ///
    /// "The br element represents a line break."
    ///
    /// [§ 4.5.27 The br element](https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-br-element)
    ///
    /// "The br element represents a line break. ...must cause a line break
    /// to occur in the inline formatting context."
    ///
    /// Forces a line break at the current position. If the current line is
    /// empty, a strut-height line box is created so the break produces
    /// visible vertical space.
    pub fn add_line_break(&mut self, font_size: f32, font_metrics: &dyn FontMetrics) {
        self.force_line_break(font_size, font_metrics);
    }

    /// Force a line break, ensuring visible vertical space even on an empty
    /// line.
    ///
    /// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
    ///
    /// "On a block container element whose content is composed of inline-level
    /// elements, 'line-height' specifies the minimal height of line boxes
    /// within the element."
    ///
    /// Even if no text has been placed on the current line, the line break
    /// must advance by at least one line-height (the "strut"). This ensures
    /// `<br>` and preserved newlines produce visible vertical space.
    fn force_line_break(&mut self, font_size: f32, font_metrics: &dyn FontMetrics) {
        let line_height = font_metrics.line_height(font_size);
        if line_height > self.current_line_max_height {
            self.current_line_max_height = line_height;
        }

        if !self.line_has_content() {
            // [§ 10.8.1](https://www.w3.org/TR/CSS2/visudet.html#leading)
            //
            // "The minimum height consists of a minimum height above the
            // baseline and a minimum depth below it, exactly as if each line
            // box starts with a zero-width inline box with the element's font
            // and line height properties."
            //
            // Create a zero-width strut fragment so finish_line() generates
            // a line box with the proper height.
            //
            // "If the inline box contains no glyphs at all, it is considered
            // to contain a strut (an invisible glyph of zero width) with the
            // A and D of the element's first available font."
            let vertical_metrics = font_metrics.vertical_metrics(font_size);
            let fragment = LineFragment {
                bounds: Rect {
                    x: self.start_x + self.left_offset + self.current_x,
                    y: self.current_y,
                    width: 0.0,
                    height: line_height,
                },
                content: FragmentContent::Text(TextRun {
                    text: String::new(),
                    width: 0.0,
                    font_size,
                    color: ColorValue::BLACK,
                    font_weight: 400,
                    font_style: FontStyle::Normal,
                    text_decoration: TextDecorationLine::default(),
                    // A strut carries no visible glyphs, so there's
                    // no inter-character spacing to apply.
                    letter_spacing: 0.0,
                    vertical_metrics,
                    baseline: baseline_offset(vertical_metrics, line_height),
                }),
                vertical_align: VerticalAlign::Baseline,
            };
            self.current_line_fragments.push(fragment);
        }

        self.finish_line();
    }

    /// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
    ///
    /// Finalize the current line box and start a new one.
    ///
    /// "The height of the line box is the distance between the uppermost box
    /// top and the lowermost box bottom."
    ///
    /// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
    ///
    /// "This property describes how inline-level content of a block container
    ///  is aligned."
    pub fn finish_line(&mut self) {
        // STEP 1: Calculate line box height and baseline.
        // [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
        //
        // "The height of the line box is the distance between the uppermost
        //  box top and the lowermost box bottom."
        //
        // "CSS assumes that every font has font metrics that specify a
        // characteristic height above the baseline and a depth below it."
        //
        // For each baseline-aligned fragment, we take its height above and
        // depth below the baseline: A' and D' for text (see
        // `baseline_offset`), and what `fragment_ascent_descent` says for
        // the rest.
        //
        // The line box height is max_ascent + max_descent.
        // The baseline is at max_ascent from the top of the line box.

        // Don't create empty line boxes.
        if !self.line_has_content() {
            // Even though we skip creating a LineBox, reset horizontal
            // position so the next content starts at the line's leading edge.
            //
            // Without this, begin_inline_box() advances from
            // prior inline-box edges persist across the "line break" and
            // cause infinite recursion in add_text()'s wrap-to-new-line
            // path: current_x stays non-zero → text doesn't fit →
            // finish_line() is a no-op → recurse with same state.
            self.current_x = 0.0;
            self.current_line_max_height = 0.0;
            self.current_line_fragments.clear();
            self.continue_open_inline_boxes();
            return;
        }

        self.split_open_inline_boxes();

        let mut max_ascent: f32 = 0.0;
        let mut max_descent: f32 = 0.0;

        for frag in &self.current_line_fragments {
            match (&frag.content, frag.vertical_align) {
                // Top/bottom-aligned fragments don't participate in
                // baseline calculation — they are positioned after
                // the baseline is established.
                (_, VerticalAlign::Top | VerticalAlign::Bottom)
                // [§ 10.6.1](https://www.w3.org/TR/CSS2/visudet.html#inline-non-replaced)
                //
                // "But only the 'line-height' is used when calculating the
                // height of the line box."
                //
                // An inline box's border box can stick out of the line box;
                // its text, placed by its own line-height, is what counts.
                | (FragmentContent::InlineBox { .. }, _) => {}
                _ => {
                    let (ascent, descent) = Self::fragment_ascent_descent(frag);
                    if ascent > max_ascent {
                        max_ascent = ascent;
                    }
                    if descent > max_descent {
                        max_descent = descent;
                    }
                }
            }
        }

        let line_height = (max_ascent + max_descent).max(self.current_line_max_height);
        let baseline = max_ascent;

        // STEP 2: Apply vertical alignment.
        // [§ 10.8.1](https://www.w3.org/TR/CSS2/visudet.html#leading)
        //
        // "The 'vertical-align' property affects the vertical positioning
        // inside a line box of the boxes generated by an inline-level element."
        for frag in &mut self.current_line_fragments {
            let (frag_ascent, _) = Self::fragment_ascent_descent(frag);

            frag.bounds.y = match frag.vertical_align {
                // "Align the baseline of the box with the baseline of the
                // parent box."
                VerticalAlign::Baseline => self.current_y + baseline - frag_ascent,
                // "Align the vertical midpoint of the box with the baseline
                // of the parent box plus half the x-height of the parent."
                //
                // Approximation: x-height ≈ 0.5 × font_size of the strut.
                VerticalAlign::Middle => self.current_y + baseline - frag.bounds.height / 2.0,
                // "Lower the baseline of the box to the proper position
                // for subscripts of the parent's box."
                //
                // Approximation: shift down by 0.1 × parent font size.
                VerticalAlign::Sub => {
                    let shift = max_ascent * 0.125;
                    self.current_y + baseline - frag_ascent + shift
                }
                // "Raise the baseline of the box to the proper position
                // for superscripts of the parent's box."
                //
                // Approximation: shift up by 0.33 × parent font size.
                VerticalAlign::Super => {
                    let shift = max_ascent * 0.4;
                    self.current_y + baseline - frag_ascent - shift
                }
                // "Align the top of the box with the top of the parent's
                // content area."
                // "Align the top of the aligned subtree with the top of
                // the line box."
                VerticalAlign::TextTop | VerticalAlign::Top => self.current_y,
                // "Align the bottom of the box with the bottom of the
                // parent's content area."
                // "Align the bottom of the aligned subtree with the bottom
                // of the line box."
                VerticalAlign::TextBottom | VerticalAlign::Bottom => {
                    self.current_y + line_height - frag.bounds.height
                }
                // "Raise (positive value) or lower (negative value) the box
                // by this distance."
                VerticalAlign::Length(offset) => self.current_y + baseline - frag_ascent - offset,
            };
        }

        // STEP 3: Apply text-align.
        // [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
        //
        // "This property describes how inline-level content of a block
        // container is aligned."
        let line_width = self.current_x;
        let x_offset = match self.text_align {
            // "Inline-level content is aligned to the left line edge."
            //
            // "Inline-level content is justified."
            // TODO: Distribute extra space between words. For now, treat
            // justify as left-aligned (per spec, the last line of a
            // justified block is left-aligned anyway).
            TextAlign::Left | TextAlign::Justify => 0.0,
            // "Inline-level content is aligned to the right line edge."
            TextAlign::Right => (self.available_width - line_width).max(0.0),
            // "Inline-level content is centered within the line box."
            TextAlign::Center => ((self.available_width - line_width) / 2.0).max(0.0),
        };

        if x_offset > 0.0 {
            for frag in &mut self.current_line_fragments {
                frag.bounds.x += x_offset;
            }
        }

        // STEP 4: Create line box and advance Y.
        let fragments = std::mem::take(&mut self.current_line_fragments);
        let line_box = LineBox {
            bounds: Rect {
                x: self.start_x,
                y: self.current_y,
                width: self.available_width,
                height: line_height,
            },
            fragments,
            line_height,
            baseline,
        };
        self.line_boxes.push(line_box);

        // Advance to the next line.
        self.current_y += line_height;
        self.current_x = 0.0;
        self.current_line_max_height = 0.0;
        self.continue_open_inline_boxes();
    }

    /// Calculate the ascent and descent of a fragment for vertical alignment.
    ///
    /// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
    ///
    /// "The height of the inline box encloses all glyphs and their
    /// half-leading on each side and is thus exactly 'line-height'."
    ///
    /// A text fragment is its run's A' and D' (`TextRun::baseline`, and
    /// the fragment's height, `line-height`, less that). An inline box's
    /// fragment carries its own ascent, set by `begin_inline_box`.
    ///
    /// An atomic fragment (`ReplacedElement`, `InlineBlock`) puts its
    /// baseline `ASCENDER_RATIO` of the way down. That is an
    /// approximation. § 10.8.1: "The baseline of an 'inline-block' is the
    /// baseline of its last line box in the normal flow"; and for
    /// 'vertical-align: baseline', "If the box does not have a baseline,
    /// align the bottom margin edge with the parent's baseline", as for a
    /// replaced element.
    fn fragment_ascent_descent(frag: &LineFragment) -> (f32, f32) {
        let frag_height = frag.bounds.height;

        let ascent = match &frag.content {
            FragmentContent::Text(run) => run.baseline,
            &FragmentContent::InlineBox { ascent, .. } => {
                return (ascent, frag_height - ascent);
            }
            FragmentContent::ReplacedElement | FragmentContent::InlineBlock(_) => {
                frag_height * ASCENDER_RATIO
            }
        };
        let descent = frag_height - ascent;

        (ascent.max(0.0), descent.max(0.0))
    }

    /// [§ 5.5 Line Breaking and Word Boundaries](https://www.w3.org/TR/css-text-3/#line-breaking)
    ///
    /// Find the last soft wrap opportunity in a text string that fits
    /// within the given width.
    ///
    /// "A soft wrap opportunity is a position in the text where the
    /// UA may choose to break."
    ///
    /// [§ 5.5.2 Breaking Rules](https://www.w3.org/TR/css-text-3/#word-breaking)
    ///
    /// "When determining line breaks:
    ///
    /// - A sequence of collapsible spaces at the end of a line is removed.
    /// - A soft wrap opportunity exists at the boundary of whitespace.
    /// - A soft wrap opportunity exists before and after CJK characters."
    ///
    ///  [§ 3.3 overflow-wrap](https://www.w3.org/TR/css-text-3/#overflow-wrap-property)
    ///  "If the word is too long to fit on a line by itself, break at
    ///   an arbitrary point."
    #[must_use]
    pub fn find_break_opportunity(
        text: &str,
        max_width: f32,
        font_size: f32,
        letter_spacing: f32,
        font_metrics: &dyn FontMetrics,
    ) -> Option<usize> {
        // STEP 1: Find all soft wrap opportunities.
        // [§ 5.5.2 Breaking Rules](https://www.w3.org/TR/css-text-3/#word-breaking)
        //
        // "A soft wrap opportunity exists at the boundary of whitespace."
        //
        // Scan for whitespace boundaries. A break opportunity exists after
        // each whitespace character (the start of the next word).
        //
        // TODO: Also handle hyphens, CJK characters.
        let mut last_fitting_break: Option<usize> = None;

        // STEP 2: Find the last opportunity that fits.
        //
        // Walk through the string character by character, tracking byte
        // offsets. At each whitespace boundary, check if the text up to
        // that point fits within max_width.
        let mut prev_was_whitespace = false;
        for (byte_idx, ch) in text.char_indices() {
            let is_whitespace = ch == ' ' || ch == '\t';

            // A break opportunity exists at the transition from whitespace
            // to non-whitespace (i.e., the start of a new word).
            if !is_whitespace && prev_was_whitespace {
                let prefix_width =
                    font_metrics.text_width(&text[..byte_idx], font_size, letter_spacing);
                if prefix_width <= max_width {
                    last_fitting_break = Some(byte_idx);
                } else {
                    // Past the limit — return the last opportunity that fit.
                    break;
                }
            }

            prev_was_whitespace = is_whitespace;
        }

        // Also consider breaking at the end of trailing whitespace.
        if prev_was_whitespace {
            let prefix_width = font_metrics.text_width(text, font_size, letter_spacing);
            if prefix_width <= max_width {
                last_fitting_break = Some(text.len());
            }
        }

        last_fitting_break
    }

    /// Return the total height consumed by all completed line boxes.
    ///
    /// [§ 10.6.1 Inline, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#inline-non-replaced)
    ///
    /// "The 'height' property does not apply. The height of the content area
    /// should be based on the font, but this specification does not specify how."
    #[must_use]
    pub fn total_height(&self) -> f32 {
        self.line_boxes.iter().map(|lb| lb.line_height).sum()
    }
}

#[cfg(test)]
mod tests {
    use koala_dom::NodeId;

    use super::{
        DecorationMetrics, FontMetrics, FragmentContent, InlineLayout, VerticalFontMetrics,
    };
    use crate::layout::box_model::EdgeSizes;
    use crate::layout::layout_box::ElementBoxId;
    use crate::style::ColorValue;
    use crate::style::values::{FontStyle, TextAlign, TextDecorationLine};

    /// A font whose ascent and descent do not add up to its em, as real
    /// fonts' seldom do. Close to Helvetica's typo metrics (0.75 em up,
    /// 0.17 em down), with a descent of 0.125 em so every value below is
    /// exact in `f32`.
    struct ShortFont;

    impl FontMetrics for ShortFont {
        fn text_width(&self, text: &str, font_size: f32, _letter_spacing: f32) -> f32 {
            #[allow(clippy::cast_precision_loss)]
            let chars = text.chars().count() as f32;
            chars * font_size * 0.5
        }

        fn line_height(&self, font_size: f32) -> f32 {
            font_size * 1.25
        }

        fn vertical_metrics(&self, font_size: f32) -> VerticalFontMetrics {
            VerticalFontMetrics {
                ascent: font_size * 0.75,
                descent: font_size * 0.125,
                line_gap: 0.0,
                underline: Some(DecorationMetrics {
                    top: font_size * -0.0625,
                    thickness: font_size * 0.0625,
                }),
                strikeout: None,
            }
        }
    }

    fn add_text(layout: &mut InlineLayout, text: &str, font_size: f32) {
        layout.add_text(
            text,
            font_size,
            &ColorValue::BLACK,
            400,
            FontStyle::Normal,
            TextDecorationLine::default(),
            0.0,
            &ShortFont,
        );
    }

    /// [§ 10.8.1](https://www.w3.org/TR/CSS2/visudet.html#leading): at
    /// 16px, A = 12 and D = 2, so AD = 14 in a 20px line: L = 6, and the
    /// baseline is A + L/2 = 15 below the top. An em-based formula would
    /// put it at 0.8 × 16 + (20 - 16) / 2 = 14.8.
    #[test]
    fn text_baseline_is_ascent_plus_half_the_leading() {
        let mut layout = InlineLayout::new(500.0, 0.0, 0.0, TextAlign::Left);
        add_text(&mut layout, "text", 16.0);
        layout.finish_line();

        let line = &layout.line_boxes[0];
        assert_eq!(line.line_height, 20.0);
        assert_eq!(line.baseline, 15.0);
        let FragmentContent::Text(run) = &line.fragments[0].content else {
            panic!("the line holds the text");
        };
        assert_eq!(run.baseline, 15.0);
        assert_eq!(run.vertical_metrics, ShortFont.vertical_metrics(16.0));
    }

    /// An inline box's content area is the font's A + D, with its
    /// baseline A below the top, so it lines up with its own text: 12px
    /// of ascent under 3px of top border and padding puts the box's top
    /// 15 - 3 - 12 = 0 below the line's top.
    #[test]
    fn inline_box_content_area_is_ascent_plus_descent() {
        let mut layout = InlineLayout::new(500.0, 0.0, 0.0, TextAlign::Left);
        let id = ElementBoxId {
            node: NodeId(1),
            pseudo_element: None,
        };
        let border_padding = EdgeSizes {
            top: 3.0,
            right: 0.0,
            bottom: 1.0,
            left: 0.0,
        };
        layout.begin_inline_box(id, 16.0, 0.0, 0.0, border_padding, &ShortFont);
        add_text(&mut layout, "link", 16.0);
        layout.end_inline_box();
        layout.finish_line();

        let line = &layout.line_boxes[0];
        let inline_box = &line.fragments[0];
        let FragmentContent::InlineBox { ascent, .. } = inline_box.content else {
            panic!("the inline box's fragment comes first");
        };
        assert_eq!(ascent, 15.0);
        assert_eq!(inline_box.bounds.height, 3.0 + 12.0 + 2.0 + 1.0);
        assert_eq!(inline_box.bounds.y, line.baseline - 15.0);
    }
}
