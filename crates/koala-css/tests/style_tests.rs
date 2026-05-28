//! Integration tests for CSS style types.

#![allow(clippy::float_cmp, clippy::similar_names)]

use koala_css::{AutoLength, ColorValue, LengthValue};

#[test]
fn test_color_from_hex_6() {
    let color = ColorValue::from_hex("#ff0000").unwrap();
    assert_eq!(
        color,
        ColorValue {
            r: 255,
            g: 0,
            b: 0,
            a: 255
        }
    );
}

#[test]
fn test_color_from_hex_3() {
    let color = ColorValue::from_hex("#f00").unwrap();
    assert_eq!(
        color,
        ColorValue {
            r: 255,
            g: 0,
            b: 0,
            a: 255
        }
    );
}

#[test]
fn test_color_from_hex_mixed_case() {
    let color = ColorValue::from_hex("#FfA500").unwrap();
    assert_eq!(
        color,
        ColorValue {
            r: 255,
            g: 165,
            b: 0,
            a: 255
        }
    );
}

#[test]
fn test_color_from_hex_without_hash() {
    let color = ColorValue::from_hex("00ff00").unwrap();
    assert_eq!(
        color,
        ColorValue {
            r: 0,
            g: 255,
            b: 0,
            a: 255
        }
    );
}

#[test]
fn test_color_from_named() {
    assert_eq!(
        ColorValue::from_named("white"),
        Some(ColorValue {
            r: 255,
            g: 255,
            b: 255,
            a: 255
        })
    );
    assert_eq!(
        ColorValue::from_named("BLACK"),
        Some(ColorValue {
            r: 0,
            g: 0,
            b: 0,
            a: 255
        })
    );
    assert_eq!(ColorValue::from_named("unknown"), None);
}

#[test]
fn test_length_px() {
    let len = LengthValue::Px(16.0);
    assert_eq!(len.to_px(), 16.0);
}

#[test]
fn test_auto_length() {
    // [§ 4.4 Automatic values](https://www.w3.org/TR/CSS2/cascade.html#value-def-auto)
    // Test that AutoLength::Auto is properly handled
    let auto = AutoLength::Auto;
    assert!(auto.is_auto());

    // Test that AutoLength::Length properly wraps a length
    let len = AutoLength::Length(LengthValue::Px(20.0));
    assert!(!len.is_auto());

    // Test to_px() for AutoLength
    assert_eq!(len.to_px(), 20.0);
    assert_eq!(auto.to_px(), 0.0); // auto returns 0.0 as fallback
}

#[test]
fn test_viewport_units() {
    // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    // "1vw = 1% of viewport width"
    // "1vh = 1% of viewport height"

    // Test vw: 60vw on a 1000px wide viewport = 600px
    let vw = LengthValue::Vw(60.0);
    assert_eq!(vw.to_px_with_viewport(1000.0, 800.0), 600.0);

    // Test vh: 15vh on a 800px tall viewport = 120px
    let vh = LengthValue::Vh(15.0);
    assert_eq!(vh.to_px_with_viewport(1000.0, 800.0), 120.0);

    // Test 100vw = full viewport width
    let full_vw = LengthValue::Vw(100.0);
    assert_eq!(full_vw.to_px_with_viewport(1280.0, 720.0), 1280.0);

    // Test 100vh = full viewport height
    let full_vh = LengthValue::Vh(100.0);
    assert_eq!(full_vh.to_px_with_viewport(1280.0, 720.0), 720.0);
}

#[test]
fn test_em_units() {
    // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    // "em: Equal to the computed value of font-size on the element"

    // Test em with default font size (16px)
    let em = LengthValue::Em(2.0);
    assert_eq!(em.to_px(), 32.0); // 2em * 16px = 32px
}

// [§ 6.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
//
// "ch unit: Equal to the used advance measure of the '0' (ZERO,
// U+0030) glyph found in the font used to render it. In the cases
// where it is impossible or impractical to determine the measure
// of the '0' glyph, it must be assumed to be 0.5em wide by 1em
// tall."
//
// koala-css takes the spec's fallback path: we resolve every `ch`
// value as `0.5 * font_size`, with `font_size` currently hard-coded
// to `DEFAULT_FONT_SIZE_PX` (16px) — the same approximation the
// existing `em` path uses until font-size plumbing arrives.

#[test]
fn test_ch_parses_from_dimension_token() {
    use koala_css::parse_single_length;
    use koala_css::parser::ComponentValue;
    use koala_css::tokenizer::CSSToken;

    let token = ComponentValue::Token(CSSToken::Dimension {
        value: 54.0,
        int_value: None,
        unit: "ch".to_owned(),
        numeric_type: koala_css::tokenizer::NumericType::Number,
    });
    assert_eq!(parse_single_length(&token), Some(LengthValue::Ch(54.0)));
}

#[test]
fn test_ch_parser_is_case_insensitive() {
    use koala_css::parse_single_length;
    use koala_css::parser::ComponentValue;
    use koala_css::tokenizer::CSSToken;

    let token = ComponentValue::Token(CSSToken::Dimension {
        value: 10.0,
        int_value: None,
        unit: "CH".to_owned(),
        numeric_type: koala_css::tokenizer::NumericType::Number,
    });
    assert_eq!(parse_single_length(&token), Some(LengthValue::Ch(10.0)));
}

#[test]
fn test_ch_resolves_via_half_em_fallback() {
    // 14ch → 14 * 16 * 0.5 = 112px with the default 16px font.
    let ch = LengthValue::Ch(14.0);
    assert_eq!(ch.to_px(), 112.0);
}

#[test]
fn test_ch_resolves_fractional() {
    let ch = LengthValue::Ch(0.5);
    assert_eq!(ch.to_px(), 4.0); // 0.5 * 16 * 0.5
}

#[test]
fn test_ch_resolves_negative() {
    // Negative ch values are legal in CSS length contexts that
    // allow negatives (e.g. `letter-spacing: -1ch`). Round-trip
    // through the resolver with the same fallback.
    let ch = LengthValue::Ch(-2.0);
    assert_eq!(ch.to_px(), -16.0); // -2 * 16 * 0.5
}

#[test]
fn test_ch_resolves_the_same_via_viewport_and_containing_block() {
    // `ch` is font-relative, not viewport- or containing-block-
    // relative, so the three resolution paths must all return the
    // same value.
    let ch = LengthValue::Ch(10.0);
    assert_eq!(ch.to_px(), 80.0);
    assert_eq!(ch.to_px_with_viewport(1280.0, 720.0), 80.0);
    assert_eq!(ch.to_px_with_containing_block(500.0, 1280.0, 720.0), 80.0);
}

// letter-spacing parser
//
// [§ 9.3 letter-spacing](https://www.w3.org/TR/css-text-3/#letter-spacing-property)
//
// The helper accepts `normal` and `<length>` in px. Other length
// units (em, %, …) are deliberately a TODO at this layer because
// resolving them needs a font-size context the parser doesn't
// have; the tests pin those down as `None` so a future regression
// (accepting em without resolving it) shows up immediately.

#[test]
fn test_letter_spacing_normal_is_zero() {
    use koala_css::parser::ComponentValue;
    use koala_css::parse_letter_spacing;
    use koala_css::tokenizer::CSSToken;

    let values = [ComponentValue::Token(CSSToken::Ident("normal".to_owned()))];
    assert_eq!(parse_letter_spacing(&values), Some(0.0));
}

#[test]
fn test_letter_spacing_px_dimension() {
    use koala_css::parser::ComponentValue;
    use koala_css::parse_letter_spacing;
    use koala_css::tokenizer::CSSToken;

    let values = [ComponentValue::Token(CSSToken::Dimension {
        value: 2.0,
        int_value: None,
        unit: "px".to_owned(),
        numeric_type: koala_css::tokenizer::NumericType::Number,
    })];
    assert_eq!(parse_letter_spacing(&values), Some(2.0));
}

#[test]
fn test_letter_spacing_normal_is_case_insensitive() {
    use koala_css::parser::ComponentValue;
    use koala_css::parse_letter_spacing;
    use koala_css::tokenizer::CSSToken;

    let values = [ComponentValue::Token(CSSToken::Ident("NoRmAl".to_owned()))];
    assert_eq!(parse_letter_spacing(&values), Some(0.0));
}

#[test]
fn test_letter_spacing_em_is_unsupported_for_now() {
    // `em` would need a font-size context the parser doesn't have
    // at this layer. Until that's wired in, the helper deliberately
    // returns `None` so the cascade falls back to inheritance or
    // the initial value rather than silently dropping the unit.
    use koala_css::parser::ComponentValue;
    use koala_css::parse_letter_spacing;
    use koala_css::tokenizer::CSSToken;

    let values = [ComponentValue::Token(CSSToken::Dimension {
        value: 2.0,
        int_value: None,
        unit: "em".to_owned(),
        numeric_type: koala_css::tokenizer::NumericType::Number,
    })];
    assert_eq!(parse_letter_spacing(&values), None);
}

#[test]
fn test_letter_spacing_rejects_unknown_keyword() {
    // Anything other than `normal` (or a valid `<length>`) must
    // return `None` so the cascade can fall back; otherwise we'd
    // silently accept typos as zero.
    use koala_css::parser::ComponentValue;
    use koala_css::parse_letter_spacing;
    use koala_css::tokenizer::CSSToken;

    let values = [ComponentValue::Token(CSSToken::Ident("wide".to_owned()))];
    assert_eq!(parse_letter_spacing(&values), None);
}
