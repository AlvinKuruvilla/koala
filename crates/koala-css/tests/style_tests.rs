//! Integration tests for CSS style types.

use koala_css::values::{AutoLength, ColorValue, LengthValue, ResolutionContext};

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
    let ctx = ResolutionContext::default();
    assert_eq!(len.to_px(&ctx), 16.0);
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
    assert!(len.is_length());

    // Test as_length() accessor
    let ctx = ResolutionContext::default();
    assert_eq!(len.as_length().map(|l| l.to_px(&ctx)), Some(20.0));
    assert_eq!(auto.as_length(), None);
}

#[test]
fn test_viewport_units() {
    // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
    // "1vw = 1% of viewport width"
    // "1vh = 1% of viewport height"

    // Test vw: 60vw on a 1000px wide viewport = 600px
    let vw = LengthValue::Vw(60.0);
    let ctx = ResolutionContext::with_viewport(1000.0, 800.0);
    assert_eq!(vw.to_px(&ctx), 600.0);

    // Test vh: 15vh on a 800px tall viewport = 120px
    let vh = LengthValue::Vh(15.0);
    assert_eq!(vh.to_px(&ctx), 120.0);

    // Test 100vw = full viewport width
    let ctx2 = ResolutionContext::with_viewport(1280.0, 720.0);
    let full_vw = LengthValue::Vw(100.0);
    assert_eq!(full_vw.to_px(&ctx2), 1280.0);

    // Test 100vh = full viewport height
    let full_vh = LengthValue::Vh(100.0);
    assert_eq!(full_vh.to_px(&ctx2), 720.0);
}

#[test]
fn test_em_units() {
    // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    // "em: Equal to the computed value of font-size on the element"

    // Test em with default font size (16px)
    let em = LengthValue::Em(2.0);
    let ctx = ResolutionContext::default();
    assert_eq!(em.to_px(&ctx), 32.0); // 2em * 16px = 32px

    // Test em with custom font size
    let ctx_custom = ResolutionContext::new(20.0, 16.0, 1000.0, 800.0);
    assert_eq!(em.to_px(&ctx_custom), 40.0); // 2em * 20px = 40px
}

#[test]
fn test_rem_units() {
    // [§ 5.1.1 Font-relative lengths](https://www.w3.org/TR/css-values-4/#font-relative-lengths)
    // "rem: Equal to the computed value of font-size on the root element"

    // Test rem with default root font size (16px)
    let rem = LengthValue::Rem(1.5);
    let ctx = ResolutionContext::default();
    assert_eq!(rem.to_px(&ctx), 24.0); // 1.5rem * 16px = 24px

    // Test rem with custom root font size (element font size should not affect rem)
    let ctx_custom = ResolutionContext::new(20.0, 24.0, 1000.0, 800.0);
    assert_eq!(rem.to_px(&ctx_custom), 36.0); // 1.5rem * 24px = 36px
}
