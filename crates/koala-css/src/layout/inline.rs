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

use koala_dom::NodeId;

use crate::style::ColorValue;

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
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
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
    /// [§ 9.2.4 Atomic inline-level boxes](https://www.w3.org/TR/css-display-3/#atomic-inline)
    ///
    /// "An inline-level box that is not an inline box (such as replaced
    /// inline-level elements, inline-block elements, and inline-table
    /// elements) is called an atomic inline-level box because it
    /// participates in its inline formatting context as a single opaque box."
    ///
    /// Stores the `NodeId` so the corresponding child `LayoutBox` can be
    /// repositioned after line finalization.
    InlineBlock(NodeId),
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

/// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
///
/// "This property describes how inline-level content of a block container
/// is aligned."
///
/// "Values have the following meanings:
///
/// left
///   Inline-level content is aligned to the left line edge of the line box.
///
/// right
///   Inline-level content is aligned to the right line edge of the line box.
///
/// center
///   Inline-level content is centered within the line box.
///
/// justify
///   Inline-level content is justified. Text should be spaced to line up
///   its left and right edges to the left and right edges of the line box,
///   except for the last line."
///
/// "The initial value is 'left' if 'direction' is 'ltr', and 'right' if
/// 'direction' is 'rtl'."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum TextAlign {
    /// "Inline-level content is aligned to the left line edge."
    #[default]
    Left,
    /// "Inline-level content is aligned to the right line edge."
    Right,
    /// "Inline-level content is centered within the line box."
    Center,
    /// "Inline-level content is justified."
    Justify,
}

/// [§ 3.3 'font-style'](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
///
/// "The 'font-style' property allows italic or oblique faces to be selected."
///
/// "normal — Selects a face that is classified as a normal face."
/// "italic — Selects a font that is labeled as an italic face."
/// "oblique — Selects a font that is labeled as an oblique face."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum FontStyle {
    /// "Selects a face that is classified as a normal face."
    #[default]
    Normal,
    /// "Selects a font that is labeled as an italic face."
    Italic,
    /// "Selects a font that is labeled as an oblique face."
    Oblique,
}

/// [§ 3 Text Decoration Lines](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
///
/// "Specifies what line decorations, if any, are added to the element."
///
/// "Values: none | [ underline || overline || line-through ]"
///
/// Multiple values can be combined (e.g., `underline line-through`).
/// `Default` gives all `false` = `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct TextDecorationLine {
    /// "Each line of text has an underline."
    pub underline: bool,
    /// "Each line of text has a line over it (i.e., on the opposite side
    /// from an underline)."
    pub overline: bool,
    /// "Each line of text has a line through the middle."
    pub line_through: bool,
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
        font_metrics: &dyn FontMetrics,
    ) {
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
                    self.place_text_fragment(
                        first_trimmed,
                        font_size,
                        line_height,
                        color,
                        font_weight,
                        font_style,
                        text_decoration,
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
        font_metrics: &dyn FontMetrics,
    ) {
        let text_width = font_metrics.text_width(text, font_size);

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
                x: self.start_x + self.left_offset + self.current_x,
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
    pub fn add_inline_block(&mut self, node_id: NodeId, width: f32, height: f32) {
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
            content: FragmentContent::InlineBlock(node_id),
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
    /// The `left_mbp` parameter is the sum of the left margin, border,
    /// and padding of the inline box. This horizontal offset is applied
    /// at the start edge of the inline box's content.
    ///
    /// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
    ///
    /// "When an inline box exceeds the width of a line box, it is split
    /// into several boxes and these boxes are distributed across several
    /// line boxes."
    ///
    /// If the left edge does not fit on the current line and the line
    /// is not empty, the current line is finished first.
    pub fn begin_inline_box(&mut self, left_mbp: f32) {
        // STEP 1: Check if the left margin+border+padding fits on the
        // current line.
        //
        // If the line already has content and adding just the opening
        // edge would overflow, wrap to a new line. The `current_x == 0.0`
        // guard prevents infinite wrapping on an empty line.
        if self.current_x + left_mbp > self.available_width && self.current_x > 0.0 {
            self.finish_line();
        }

        // STEP 2: Advance current_x by the left margin+border+padding.
        //
        // Content placed after this call will start after the left edge
        // decoration of the inline box.
        self.current_x += left_mbp;
    }

    /// [§ 9.2.2 Inline-level elements and inline boxes](https://www.w3.org/TR/CSS2/visuren.html#inline-boxes)
    ///
    /// End a non-replaced inline box.
    ///
    /// The `right_mbp` parameter is the sum of the right margin, border,
    /// and padding of the inline box. This horizontal offset is applied
    /// at the end edge of the inline box's content.
    ///
    /// [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    ///
    /// "Horizontal margins, borders, and padding are respected between
    /// these boxes."
    pub fn end_inline_box(&mut self, right_mbp: f32) {
        // Advance current_x by the right margin+border+padding.
        //
        // Content placed after this call will start after the right edge
        // decoration of the inline box.
        self.current_x += right_mbp;
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
        // For each baseline-aligned fragment, we compute ascent and descent:
        //   - half_leading = (line_height - font_size) / 2
        //   - ascent = half_leading + font_size * ASCENDER_RATIO
        //   - descent = line_height - ascent
        //
        // The line box height is max_ascent + max_descent.
        // The baseline is at max_ascent from the top of the line box.
        //
        // Implementation note: Without actual font tables, we approximate
        // the ascender as 80% of the em square. This matches typical Latin
        // fonts (e.g., Helvetica ascender ≈ 0.77, Arial ≈ 0.81).
        const ASCENDER_RATIO: f32 = 0.8;

        // Don't create empty line boxes.
        if self.current_line_fragments.is_empty() {
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
            return;
        }

        let mut max_ascent: f32 = 0.0;
        let mut max_descent: f32 = 0.0;

        for frag in &self.current_line_fragments {
            match frag.vertical_align {
                VerticalAlign::Top | VerticalAlign::Bottom => {
                    // Top/bottom-aligned fragments don't participate in
                    // baseline calculation — they are positioned after
                    // the baseline is established.
                }
                _ => {
                    let (ascent, descent) = Self::fragment_ascent_descent(frag, ASCENDER_RATIO);
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
            let (frag_ascent, _) = Self::fragment_ascent_descent(frag, ASCENDER_RATIO);

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
    }

    /// Calculate the ascent and descent of a fragment for vertical alignment.
    ///
    /// [§ 10.8.1 Leading and half-leading](https://www.w3.org/TR/CSS2/visudet.html#leading)
    ///
    /// "The height of the inline box encloses all glyphs and their
    /// half-leading on each side and is thus exactly 'line-height'."
    ///
    /// For a text fragment:
    ///   `half_leading = (line_height - font_size) / 2`
    ///   `ascent = half_leading + font_size × ascender_ratio`
    ///   `descent = line_height - ascent`
    ///
    /// For non-text fragments (`InlineBox`, `ReplacedElement`), we approximate
    /// using the fragment height directly.
    fn fragment_ascent_descent(frag: &LineFragment, ascender_ratio: f32) -> (f32, f32) {
        let frag_height = frag.bounds.height;

        let font_size = match &frag.content {
            FragmentContent::Text(run) => run.font_size,
            // For non-text fragments, treat the full height as the "font size"
            // so ascent = height × ascender_ratio.
            FragmentContent::InlineBox
            | FragmentContent::ReplacedElement
            | FragmentContent::InlineBlock(_) => frag_height,
        };

        let half_leading = (frag_height - font_size) / 2.0;
        let ascent = font_size.mul_add(ascender_ratio, half_leading);
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
    #[must_use]
    pub fn total_height(&self) -> f32 {
        self.line_boxes.iter().map(|lb| lb.line_height).sum()
    }
}
