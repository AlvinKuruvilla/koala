//! Painting a run of text with its decoration lines.
//!
//! [CSS Text Decoration 4](https://drafts.csswg.org/css-text-decor-4/)
//!
//! Decorations are placed here, from the font metrics layout recorded on
//! each [`TextRun`], and reach the renderer as plain rectangles: the
//! renderer draws glyphs on a baseline and fills rectangles, and knows
//! nothing of where CSS puts an underline.
//!
//! Each line is positioned from the run's own font. § 2.5 positions a
//! decorating box's line from all the text it decorates on that line,
//! averaged; with one font per run and no averaging, a decoration over
//! runs of different sizes steps between them instead.

use crate::layout::inline::{DecorationMetrics, TextRun, VerticalFontMetrics};
use crate::style::BorderRadius;
use crate::style::ColorValue;

use super::{DisplayCommand, DisplayList};

/// Pushes the commands that paint `run`, whose fragment's top-left
/// corner is at (`x`, `top`), in `color`.
///
/// [CSS 2.1 Appendix E.2](https://www.w3.org/TR/CSS2/zindex.html#painting-order),
/// for a run of text:
pub(super) fn push_text_run(
    display_list: &mut DisplayList,
    x: f32,
    top: f32,
    run: &TextRun,
    color: &ColorValue,
) {
    let baseline = top + run.baseline;
    let metrics = run.vertical_metrics;
    let decoration = run.text_decoration;
    let line = |(line_top, thickness): (f32, f32)| DisplayCommand::FillRect {
        x,
        y: line_top,
        width: run.width,
        height: thickness,
        color: color.clone(),
        border_radius: BorderRadius::default(),
    };

    // "any underlining affecting the text of the element, in tree order
    // of the elements applying the underlining"
    if decoration.underline {
        display_list.push(line(underline(metrics, run.font_size, baseline)));
    }
    // "any overlining affecting the text of the element, in tree order of
    // the elements applying the overlining"
    if decoration.overline {
        display_list.push(line(overline(metrics, run.font_size, baseline)));
    }

    // "the text."
    display_list.push(DisplayCommand::DrawText {
        x,
        y: baseline,
        text: run.text.clone(),
        font_size: run.font_size,
        color: color.clone(),
        font_weight: run.font_weight,
        font_style: run.font_style,
        letter_spacing: run.letter_spacing,
    });

    // "any line-through affecting the text of the element, in tree order
    // of the elements applying the line-through"
    if decoration.line_through {
        display_list.push(line(line_through(metrics, run.font_size, baseline)));
    }
}

/// [§ 2.4 text-decoration-thickness](https://drafts.csswg.org/css-text-decor-4/#text-decoration-thickness-property)
///
/// "The UA should round the actual value to the nearest integer device
/// pixel, and ensure it is at least one device pixel."
fn round_thickness(thickness: f32) -> f32 {
    thickness.round().max(1.0)
}

/// A line's thickness when the font suggests none: a sixteenth of the
/// font size. Arbitrary; about what fonts suggest (Helvetica's underline
/// is 101/2048 ≈ 0.049 em).
fn fallback_thickness(font_size: f32) -> f32 {
    font_size / 16.0
}

/// The underline's top edge and thickness, for text on `baseline`.
fn underline(metrics: VerticalFontMetrics, font_size: f32, baseline: f32) -> (f32, f32) {
    // Without font metrics, one line thickness below the baseline:
    // arbitrary, but close to where fonts put it (Helvetica's top edge is
    // 155/2048 ≈ 0.076 em down, against 0.0625 em here).
    let Some(DecorationMetrics { top, thickness }) = metrics.underline else {
        let thickness = round_thickness(fallback_thickness(font_size));
        return (baseline + thickness, thickness);
    };

    // [§ 2.8.2 Using Font Metrics for Automatic Positioning](https://drafts.csswg.org/css-text-decor-4/#line-auto-offset)
    //
    // "The UA should use such font-based information in its choice of
    // auto offset wherever appropriate"
    //
    // [§ 2.7 text-underline-position](https://drafts.csswg.org/css-text-decor-4/#text-underline-position-property)
    //
    // "auto: The user agent may use any algorithm to determine the
    // underline's position; however it must be placed at or under the
    // alphabetic baseline."
    //
    // `top` is up from the baseline and pixel rows run down, so a positive
    // `top`, an underline above the baseline, is held at the baseline
    // instead.
    let line_top = (baseline - top).max(baseline);

    // [§ 2.4.1 Automatic Thickness of Text Decoration Lines](https://drafts.csswg.org/css-text-decor-4/#text-decoration-thickness)
    //
    // "The UA should use such font-based information when choosing auto
    // line thicknesses wherever appropriate."
    (line_top, round_thickness(thickness))
}

/// The overline's top edge and thickness, for text on `baseline`.
///
/// [§ 2.5 Determining the Position and Thickness of Line Decorations](https://drafts.csswg.org/css-text-decor-4/#line-position)
///
/// "over lines: Align the line decoration with respect to the highest
/// over EM-box edge of the considered text."
///
/// The over edge is taken as the font's ascent above the baseline, and
/// the line hangs below it, inside the text. Fonts give no overline
/// thickness, so it matches the underline's, as decorations from one box
/// usually do.
fn overline(metrics: VerticalFontMetrics, font_size: f32, baseline: f32) -> (f32, f32) {
    let thickness = round_thickness(
        metrics
            .underline
            .map_or_else(|| fallback_thickness(font_size), |line| line.thickness),
    );
    (baseline - metrics.ascent, thickness)
}

/// The line-through's top edge and thickness, for text on `baseline`.
///
/// [§ 2.5 Determining the Position and Thickness of Line Decorations](https://drafts.csswg.org/css-text-decor-4/#line-position)
///
/// "For each run of considered text with the same font-size, compute an
/// ideal position averaged from its font metrics."
fn line_through(metrics: VerticalFontMetrics, font_size: f32, baseline: f32) -> (f32, f32) {
    // Without font metrics, centred on half the x-height.
    // [Appendix A.2](https://drafts.csswg.org/css-inline-3/#baseline-synthesis-fonts)
    // of CSS Inline 3 suggests "x-height: .5em" when a font gives none.
    let Some(DecorationMetrics { top, thickness }) = metrics.strikeout else {
        let thickness = round_thickness(fallback_thickness(font_size));
        let middle = font_size.mul_add(-0.25, baseline);
        return (middle - thickness / 2.0, thickness);
    };

    // `OS/2.yStrikeoutPosition`, "The position of the top of the strikeout
    // stroke relative to the baseline".
    (baseline - top, round_thickness(thickness))
}

#[cfg(test)]
mod tests {
    use super::{line_through, overline, underline};
    use crate::layout::inline::{DecorationMetrics, VerticalFontMetrics};

    /// Helvetica.ttc face 0 at 16px, as `koala-font` reads it: typo
    /// metrics, `post`'s underline, `OS/2`'s strikeout.
    const HELVETICA_16PX: VerticalFontMetrics = VerticalFontMetrics {
        ascent: 12.0,
        descent: 2.718_75,
        line_gap: 0.476_562_5,
        underline: Some(DecorationMetrics {
            top: -1.210_937_5,
            thickness: 0.789_062_5,
        }),
        strikeout: Some(DecorationMetrics {
            top: 4.0,
            thickness: 0.796_875,
        }),
    };

    /// The font's underline: its top edge 1.21px below the baseline, and
    /// its 0.79px thickness rounded up to one device pixel.
    #[test]
    fn underline_follows_the_font() {
        assert_eq!(underline(HELVETICA_16PX, 16.0, 100.0), (100.0 + 1.210_937_5, 1.0));
    }

    /// "it must be placed at or under the alphabetic baseline": a font
    /// whose underline sits above the baseline is overruled.
    #[test]
    fn underline_never_rises_above_the_baseline() {
        let metrics = VerticalFontMetrics {
            underline: Some(DecorationMetrics {
                top: 3.0,
                thickness: 1.0,
            }),
            ..HELVETICA_16PX
        };
        assert_eq!(underline(metrics, 16.0, 100.0), (100.0, 1.0));
    }

    #[test]
    fn underline_without_font_metrics_sits_one_thickness_below_the_baseline() {
        let metrics = VerticalFontMetrics {
            underline: None,
            ..HELVETICA_16PX
        };
        assert_eq!(underline(metrics, 16.0, 100.0), (101.0, 1.0));
    }

    #[test]
    fn line_through_follows_the_font() {
        assert_eq!(line_through(HELVETICA_16PX, 16.0, 100.0), (96.0, 1.0));
    }

    /// Centred on half of the fallback x-height: 0.25em above the
    /// baseline, 4px at 16px.
    #[test]
    fn line_through_without_font_metrics_crosses_half_the_x_height() {
        let metrics = VerticalFontMetrics {
            strikeout: None,
            ..HELVETICA_16PX
        };
        assert_eq!(line_through(metrics, 16.0, 100.0), (95.5, 1.0));
    }

    #[test]
    fn overline_hangs_from_the_ascent() {
        assert_eq!(overline(HELVETICA_16PX, 16.0, 100.0), (88.0, 1.0));
    }
}
