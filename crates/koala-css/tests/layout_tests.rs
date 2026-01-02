//! Integration tests for CSS layout types.

use koala_css::layout::default_display_for_element;
use koala_css::DisplayValue;

#[test]
fn test_default_display_block() {
    assert_eq!(
        default_display_for_element("div"),
        Some(DisplayValue::block())
    );
    assert_eq!(
        default_display_for_element("p"),
        Some(DisplayValue::block())
    );
}

#[test]
fn test_default_display_inline() {
    assert_eq!(
        default_display_for_element("span"),
        Some(DisplayValue::inline())
    );
    assert_eq!(
        default_display_for_element("a"),
        Some(DisplayValue::inline())
    );
}

#[test]
fn test_default_display_none() {
    assert_eq!(default_display_for_element("script"), None);
    assert_eq!(default_display_for_element("style"), None);
    assert_eq!(default_display_for_element("head"), None);
}
