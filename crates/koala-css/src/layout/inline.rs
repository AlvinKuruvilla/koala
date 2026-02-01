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

use super::box_model::Rect;

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
    /// Measure the total advance width of a text string at the given font size.
    ///
    /// This should sum the advance width of each glyph in the string,
    /// matching the cursor advancement used during text rendering.
    fn text_width(&self, text: &str, font_size: f32) -> f32;

    /// Calculate the line height for a given font size.
    ///
    /// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
    ///
    /// "The initial value of 'line-height' is 'normal'. We recommend a used
    /// value for 'normal' between 1.0 and 1.2."
    fn line_height(&self, font_size: f32) -> f32;
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
    fn text_width(&self, text: &str, font_size: f32) -> f32 {
        const CHAR_WIDTH_RATIO: f32 = 0.6;
        text.chars().count() as f32 * font_size * CHAR_WIDTH_RATIO
    }

    fn line_height(&self, font_size: f32) -> f32 {
        const LINE_HEIGHT_RATIO: f32 = 1.2;
        font_size * LINE_HEIGHT_RATIO
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
    /// An inline-level box (e.g., `<span>`, `<a>`).
    InlineBox,
    /// A replaced inline element (e.g., `<img>`).
    ReplacedElement,
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
/// <percentage>
///   Raise (positive) or lower (negative) the box by this distance
///   (a percentage of the 'line-height' value).
///
/// <length>
///   Raise (positive) or lower (negative) the box by this distance."
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlign {
    /// "Align the baseline of the box with the baseline of the parent box."
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

impl Default for VerticalAlign {
    fn default() -> Self {
        VerticalAlign::Baseline
    }
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
}

impl InlineLayout {
    /// Create a new inline layout context.
    pub fn new(available_width: f32, start_y: f32) -> Self {
        InlineLayout {
            line_boxes: Vec::new(),
            current_line_fragments: Vec::new(),
            current_x: 0.0,
            current_y: start_y,
            available_width,
            current_line_max_height: 0.0,
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
    pub fn add_text(&mut self, text: &str, font_size: f32, font_metrics: &dyn FontMetrics) {
        // STEP 1: Measure the text width.
        // [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
        //
        // "CSS assumes that every font has font metrics that specify a
        // characteristic height above the baseline and a depth below it."
        //
        // The width comes from summing per-glyph advance widths via FontMetrics.
        // The height contribution is the line-height from FontMetrics.
        let text_width = font_metrics.text_width(text, font_size);
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
        let fits_on_current_line =
            self.current_x + text_width <= self.available_width || self.current_x == 0.0;

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

            if let Some(break_idx) =
                Self::find_break_opportunity(text, remaining_width, font_size, font_metrics)
            {
                // Split at the break point: place the first part on the
                // current line, then recurse for the remainder.
                let (first, rest) = text.split_at(break_idx);

                // Trim trailing whitespace from the first part per spec:
                // [§ 4.1.3](https://www.w3.org/TR/css-text-3/#white-space-phase-2)
                // "A sequence of collapsible spaces at the end of a line is removed."
                let first_trimmed = first.trim_end();
                if !first_trimmed.is_empty() {
                    self.place_text_fragment(first_trimmed, font_size, line_height, font_metrics);
                }

                // Finalize this line and start a new one.
                self.finish_line();

                // Trim leading whitespace from the remainder per spec:
                // "A sequence of collapsible spaces at the beginning of a line is removed."
                let rest_trimmed = rest.trim_start();
                if !rest_trimmed.is_empty() {
                    self.add_text(rest_trimmed, font_size, font_metrics);
                }
                return;
            }

            // No break opportunity found that fits — wrap the entire text
            // to a new line. If the line is not empty, finish it first.
            // The `current_x == 0.0` guard in `fits_on_current_line` above
            // prevents infinite recursion: on a fresh line we always place
            // the text even if it overflows.
            self.finish_line();
            self.add_text(text, font_size, font_metrics);
            return;
        }

        // STEP 4: Place fragment on the current line.
        self.place_text_fragment(text, font_size, line_height, font_metrics);
    }

    /// Place a text fragment at the current position on the current line.
    ///
    /// This is the shared placement logic used by `add_text` after measurement
    /// and line-breaking decisions have been made.
    fn place_text_fragment(
        &mut self,
        text: &str,
        font_size: f32,
        line_height: f32,
        font_metrics: &dyn FontMetrics,
    ) {
        let text_width = font_metrics.text_width(text, font_size);

        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "In an inline formatting context, boxes are laid out horizontally,
        // one after the other, beginning at the top of a containing block."
        let fragment = LineFragment {
            bounds: Rect {
                x: self.current_x,
                y: self.current_y,
                width: text_width,
                height: line_height,
            },
            content: FragmentContent::Text(TextRun {
                text: text.to_string(),
                width: text_width,
                font_size,
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

    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// Add an inline-level box (e.g., `<span>`) to the current line.
    ///
    /// "Horizontal margins, borders, and padding are respected between
    /// inline boxes."
    ///
    /// "The boxes may be aligned vertically in different ways: their bottoms
    /// or tops may be aligned, or the baselines of text within them may be
    /// aligned."
    pub fn add_inline_box(&mut self, width: f32, height: f32) {
        // STEP 1: Check if box fits on current line.
        // [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
        //
        // "When an inline box exceeds the width of a line box, it is split
        // into several boxes and these boxes are distributed across several
        // line boxes."
        //
        // If the box doesn't fit and we're not at the start of a line,
        // wrap to a new line. The `current_x == 0.0` guard prevents
        // infinite wrapping when a single box is wider than the available
        // width.
        let fits_on_current_line =
            self.current_x + width <= self.available_width || self.current_x == 0.0;

        if !fits_on_current_line {
            self.finish_line();
        }

        // STEP 2: Create fragment and position it.
        //
        // "Horizontal margins, borders, and padding are respected between
        // inline boxes."
        //
        // The width and height parameters represent the margin box
        // dimensions of the inline box, as computed by the caller.
        let fragment = LineFragment {
            bounds: Rect {
                x: self.current_x,
                y: self.current_y,
                width,
                height,
            },
            content: FragmentContent::InlineBox,
            vertical_align: VerticalAlign::Baseline,
        };
        self.current_line_fragments.push(fragment);

        // STEP 3: Advance current position and update line height.
        self.current_x += width;
        if height > self.current_line_max_height {
            self.current_line_max_height = height;
        }
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
        // Don't create empty line boxes.
        if self.current_line_fragments.is_empty() {
            return;
        }

        // STEP 1: Calculate line box height.
        // [§ 10.8.1](https://www.w3.org/TR/CSS2/visudet.html#leading)
        //
        // "The height of the line box is the distance between the uppermost
        //  box top and the lowermost box bottom."
        let line_height = self.current_line_max_height;

        // STEP 2: Calculate baseline position.
        //
        // Simplified: place baseline at 80% of line height (approximates
        // typical font metrics where ascender ≈ 80% of em square).
        //
        // TODO: Derive from actual font metrics (ascent / (ascent + descent)).
        let baseline = line_height * 0.8;

        // STEP 3: Apply vertical alignment.
        // TODO: Adjust fragment y positions based on vertical-align values.
        // For now, all fragments use Baseline alignment and share the same y.

        // STEP 4: Apply text-align.
        // TODO: Adjust fragment x positions for center/right/justify.
        // For now, left-aligned (the default for LTR).

        // STEP 5: Create line box and advance Y.
        let fragments = std::mem::take(&mut self.current_line_fragments);
        let line_box = LineBox {
            bounds: Rect {
                x: 0.0,
                y: self.current_y,
                width: self.current_x,
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
    pub fn find_break_opportunity(
        text: &str,
        max_width: f32,
        font_size: f32,
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
                let prefix_width = font_metrics.text_width(&text[..byte_idx], font_size);
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
            let prefix_width = font_metrics.text_width(text, font_size);
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
    pub fn total_height(&self) -> f32 {
        self.line_boxes.iter().map(|lb| lb.line_height).sum()
    }
}
