//! Integration tests for CSS cascade and style computation.

use koala_css::Stylesheet;
use koala_css::cascade::compute_styles;
use koala_css::parser::CSSParser;
use koala_css::tokenizer::CSSTokenizer;
use koala_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};

fn parse_css(css: &str) -> Stylesheet {
    let mut tokenizer = CSSTokenizer::new(css.to_string());
    tokenizer.run();
    let mut parser = CSSParser::new(tokenizer.into_tokens());
    parser.parse_stylesheet()
}

/// Empty stylesheet used as the UA stylesheet in tests.
/// Tests exercise author CSS behavior, so no UA defaults are needed.
fn empty_stylesheet() -> Stylesheet {
    Stylesheet { rules: vec![] }
}

/// Helper to create element node types
fn make_element(tag: &str, id: Option<&str>, classes: &[&str]) -> NodeType {
    let mut attrs = AttributesMap::new();
    if let Some(id_val) = id {
        let _ = attrs.insert("id".to_string(), id_val.to_string());
    }
    if !classes.is_empty() {
        let _ = attrs.insert("class".to_string(), classes.join(" "));
    }
    NodeType::Element(ElementData {
        tag_name: tag.to_string(),
        attrs,
    })
}

#[test]
fn test_compute_styles_simple() {
    let css = "body { color: #333; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let body_id = tree.alloc(make_element("body", None, &[]));
    tree.append_child(NodeId::ROOT, body_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    // Document has no style
    assert!(!styles.contains_key(&NodeId::ROOT));

    // Body should have the color applied
    let body_style = styles.get(&body_id).unwrap();
    assert!(body_style.color.is_some());
    let color = body_style.color.as_ref().unwrap();
    assert_eq!(color.r, 0x33);
    assert_eq!(color.g, 0x33);
    assert_eq!(color.b, 0x33);
}

#[test]
fn test_compute_styles_inheritance() {
    let css = "body { color: #ff0000; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let body_id = tree.alloc(make_element("body", None, &[]));
    let p_id = tree.alloc(make_element("p", None, &[]));
    tree.append_child(NodeId::ROOT, body_id);
    tree.append_child(body_id, p_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    // P should inherit color from body
    let p_style = styles.get(&p_id).unwrap();
    assert!(p_style.color.is_some());
    let color = p_style.color.as_ref().unwrap();
    assert_eq!(color.r, 0xff);
}

#[test]
fn test_compute_styles_specificity() {
    // Class selector should override type selector
    let css = "p { color: #ff0000; } .highlight { color: #00ff00; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let p_id = tree.alloc(make_element("p", None, &["highlight"]));
    tree.append_child(NodeId::ROOT, p_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    let p_style = styles.get(&p_id).unwrap();
    let color = p_style.color.as_ref().unwrap();
    // Class selector wins (green)
    assert_eq!(color.g, 0xff);
    assert_eq!(color.r, 0x00);
}

#[test]
fn test_compute_styles_id_selector() {
    let css = "#main-content { background-color: white; padding: 16px; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", Some("main-content"), &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    let div_style = styles.get(&div_id).unwrap();
    assert!(div_style.background_color.is_some());
    assert!(div_style.padding_top.is_some());
}

#[test]
fn test_background_color_not_inherited() {
    // [§ 3.2 background-color](https://www.w3.org/TR/css-backgrounds-3/#background-color)
    // "Inherited: no"
    let css = "body { background-color: #f5f5f5; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let body_id = tree.alloc(make_element("body", None, &[]));
    let p_id = tree.alloc(make_element("p", None, &[]));
    tree.append_child(NodeId::ROOT, body_id);
    tree.append_child(body_id, p_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    // Body should have background-color
    let body_style = styles.get(&body_id).unwrap();
    assert!(body_style.background_color.is_some());
    let bg = body_style.background_color.as_ref().unwrap();
    assert_eq!(bg.r, 0xf5);

    // P should NOT inherit background-color
    let p_style = styles.get(&p_id).unwrap();
    assert!(p_style.background_color.is_none());
}

#[test]
fn test_line_height_inherited() {
    // [§ 4.2 line-height](https://www.w3.org/TR/css-inline-3/#line-height-property)
    // "Inherited: yes"
    let css = "body { line-height: 1.6; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let body_id = tree.alloc(make_element("body", None, &[]));
    let p_id = tree.alloc(make_element("p", None, &[]));
    tree.append_child(NodeId::ROOT, body_id);
    tree.append_child(body_id, p_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    // P should inherit line-height from body
    let p_style = styles.get(&p_id).unwrap();
    assert!(p_style.line_height.is_some());
    assert!((p_style.line_height.unwrap() - 1.6).abs() < 0.01);
}

#[test]
fn test_margin_and_padding_shorthand() {
    let css = "div { margin: 20px; padding: 16px; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", None, &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    let div_style = styles.get(&div_id).unwrap();

    // All four sides should be set
    assert!(div_style.margin_top.is_some());
    assert!(div_style.margin_right.is_some());
    assert!(div_style.margin_bottom.is_some());
    assert!(div_style.margin_left.is_some());

    assert!(div_style.padding_top.is_some());
    assert!(div_style.padding_right.is_some());
    assert!(div_style.padding_bottom.is_some());
    assert!(div_style.padding_left.is_some());

    // Verify values
    // [§ 8.3 Margin properties](https://www.w3.org/TR/CSS2/box.html#margin-properties)
    // Margins can be 'auto' or a length. Here we expect a length value.
    if let Some(koala_css::AutoLength::Length(koala_css::LengthValue::Px(v))) =
        &div_style.margin_top
    {
        assert!((v - 20.0).abs() < 0.01);
    }
    if let Some(koala_css::LengthValue::Px(v)) = &div_style.padding_top {
        assert!((v - 16.0).abs() < 0.01);
    }
}

#[test]
fn test_font_size_inherited() {
    // [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
    // "Inherited: yes"
    let css = "body { font-size: 16px; } h1 { font-size: 32px; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let body_id = tree.alloc(make_element("body", None, &[]));
    let h1_id = tree.alloc(make_element("h1", None, &[]));
    let span_id = tree.alloc(make_element("span", None, &[]));
    tree.append_child(NodeId::ROOT, body_id);
    tree.append_child(body_id, h1_id);
    tree.append_child(h1_id, span_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    // Span inside h1 should inherit h1's font-size (32px)
    let span_style = styles.get(&span_id).unwrap();
    assert!(span_style.font_size.is_some());
    if let Some(koala_css::LengthValue::Px(v)) = &span_style.font_size {
        assert!((v - 32.0).abs() < 0.01, "Expected 32px but got {}px", v);
    }
}

#[test]
fn test_border_parsing() {
    let css = "#box { border: 1px solid #ddd; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", Some("box"), &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    let div_style = styles.get(&div_id).unwrap();

    // All four borders should be set
    assert!(div_style.border_top.is_some());
    assert!(div_style.border_right.is_some());
    assert!(div_style.border_bottom.is_some());
    assert!(div_style.border_left.is_some());

    // Verify border properties
    let border = div_style.border_top.as_ref().unwrap();
    match border.width {
        koala_css::LengthValue::Px(w) => {
            assert!(
                (w - 1.0).abs() < 0.01,
                "Expected border width ~1.0px, got {w}px"
            )
        }
        koala_css::LengthValue::Em(_) => {
            panic!("Expected border width in Px, got Em (should have been resolved)")
        }
        koala_css::LengthValue::Vw(_) => {
            panic!("Expected border width in Px, got Vw (should have been resolved)")
        }
        koala_css::LengthValue::Vh(_) => {
            panic!("Expected border width in Px, got Vh (should have been resolved)")
        }
    }
    assert_eq!(border.style, "solid");
    assert_eq!(border.color.r, 0xdd);
    assert_eq!(border.color.g, 0xdd);
    assert_eq!(border.color.b, 0xdd);
}

#[test]
fn test_simple_html_full_pipeline() {
    // Integration test matching res/simple.html CSS
    use koala_css::extract_style_content;
    use koala_html::{HTMLParser, HTMLTokenizer};

    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <title>Test</title>
    <style>
        body {
            font-family: sans-serif;
            background-color: #f5f5f5;
            color: #333;
            margin: 20px;
        }
        h1 {
            color: #2563eb;
            font-size: 32px;
        }
        .highlight {
            background-color: #fef08a;
            padding: 4px 8px;
        }
        #main-content {
            border: 1px solid #ddd;
            padding: 16px;
            background-color: white;
        }
        p {
            line-height: 1.6;
            margin-bottom: 16px;
        }
    </style>
</head>
<body>
    <h1>Welcome</h1>
    <div id="main-content">
        <p>Test <span class="highlight">styled</span> text.</p>
    </div>
</body>
</html>"#;

    // Parse HTML
    let mut tokenizer = HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let parser = HTMLParser::new(tokenizer.into_tokens());
    let tree = parser.run();

    // Extract and parse CSS
    let css_text = extract_style_content(&tree);
    assert!(!css_text.is_empty(), "CSS should be extracted from <style>");

    let mut css_tokenizer = CSSTokenizer::new(css_text);
    css_tokenizer.run();
    let mut css_parser = CSSParser::new(css_tokenizer.into_tokens());
    let stylesheet = css_parser.parse_stylesheet();

    // Compute styles
    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);

    // Should have styles for multiple elements
    assert!(!styles.is_empty(), "Should have computed styles");

    // Find body and verify styles
    fn find_element(tree: &DomTree, from: NodeId, tag: &str) -> Option<NodeId> {
        if let Some(data) = tree.as_element(from) {
            if data.tag_name.eq_ignore_ascii_case(tag) {
                return Some(from);
            }
        }
        for &child_id in tree.children(from) {
            if let Some(found) = find_element(tree, child_id, tag) {
                return Some(found);
            }
        }
        None
    }

    // Verify body has styles
    if let Some(body_id) = find_element(&tree, tree.root(), "body") {
        let body_style = styles.get(&body_id).unwrap();
        assert!(body_style.color.is_some(), "body should have color");
        assert!(
            body_style.background_color.is_some(),
            "body should have background-color"
        );
        assert!(body_style.margin_top.is_some(), "body should have margin");
    }

    // Verify h1 has specific color
    if let Some(h1_id) = find_element(&tree, tree.root(), "h1") {
        let h1_style = styles.get(&h1_id).unwrap();
        let color = h1_style.color.as_ref().unwrap();
        // #2563eb = rgb(37, 99, 235)
        assert_eq!(color.r, 0x25);
        assert_eq!(color.g, 0x63);
        assert_eq!(color.b, 0xeb);
    }
}

/// [§ 4 Logical Property Groups](https://drafts.csswg.org/css-logical-1/#logical-property-groups)
///
/// Test that logical and physical margin properties compete in the cascade.
/// The property declared later (higher source order) should win.
#[test]
fn test_logical_property_cascade_order() {
    // margin-block-start (10px) comes before margin-top (20px)
    // margin-top should win because it has higher source order
    let css = "div { margin-block-start: 10px; margin-top: 20px; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", None, &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);
    let div_style = styles.get(&div_id).unwrap();

    // margin-top should be 20px (the later declaration wins)
    if let Some(koala_css::AutoLength::Length(koala_css::LengthValue::Px(v))) =
        &div_style.margin_top
    {
        assert!(
            (v - 20.0).abs() < 0.01,
            "Expected margin-top 20px but got {}px (margin-top should win)",
            v
        );
    } else {
        panic!("Expected margin_top to be set");
    }
}

/// [§ 4 Logical Property Groups](https://drafts.csswg.org/css-logical-1/#logical-property-groups)
///
/// Test that when physical property comes first, logical property wins.
#[test]
fn test_logical_property_cascade_order_reversed() {
    // margin-top (20px) comes before margin-block-start (10px)
    // margin-block-start should win because it has higher source order
    let css = "div { margin-top: 20px; margin-block-start: 10px; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", None, &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);
    let div_style = styles.get(&div_id).unwrap();

    // margin-top should be 10px (margin-block-start declared later wins)
    if let Some(koala_css::AutoLength::Length(koala_css::LengthValue::Px(v))) =
        &div_style.margin_top
    {
        assert!(
            (v - 10.0).abs() < 0.01,
            "Expected margin-top 10px but got {}px (margin-block-start should win)",
            v
        );
    } else {
        panic!("Expected margin_top to be set");
    }
}

/// Test margin-block-end cascade resolution.
#[test]
fn test_logical_property_block_end_cascade() {
    // margin-bottom comes before margin-block-end
    // margin-block-end should win
    let css = "div { margin-bottom: 30px; margin-block-end: 15px; }";
    let stylesheet = parse_css(css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", None, &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);
    let div_style = styles.get(&div_id).unwrap();

    // margin-bottom should be 15px (margin-block-end declared later wins)
    if let Some(koala_css::AutoLength::Length(koala_css::LengthValue::Px(v))) =
        &div_style.margin_bottom
    {
        assert!(
            (v - 15.0).abs() < 0.01,
            "Expected margin-bottom 15px but got {}px",
            v
        );
    } else {
        panic!("Expected margin_bottom to be set");
    }
}

// ===== Color function tests =====

/// Helper: parse CSS, apply to a div, return the computed color.
fn color_from_css(property: &str, value: &str) -> Option<koala_css::ColorValue> {
    let css = format!("div {{ {}: {}; }}", property, value);
    let stylesheet = parse_css(&css);

    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element("div", None, &[]));
    tree.append_child(NodeId::ROOT, div_id);

    let styles = compute_styles(&tree, &empty_stylesheet(), &stylesheet);
    let style = styles.get(&div_id)?;
    match property {
        "color" => style.color.clone(),
        "background-color" => style.background_color.clone(),
        _ => None,
    }
}

/// [§ 4.1 The RGB Functions](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// Legacy comma-separated syntax: rgb(R, G, B)
#[test]
fn test_rgb_legacy_syntax() {
    let c = color_from_css("color", "rgb(255, 0, 128)").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 128);
    assert_eq!(c.a, 255);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// Modern space-separated syntax: rgb(R G B)
#[test]
fn test_rgb_modern_syntax() {
    let c = color_from_css("color", "rgb(100 200 50)").unwrap();
    assert_eq!(c.r, 100);
    assert_eq!(c.g, 200);
    assert_eq!(c.b, 50);
    assert_eq!(c.a, 255);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// rgba() is an alias for rgb() with alpha.
#[test]
fn test_rgba_legacy_syntax() {
    let c = color_from_css("color", "rgba(255, 0, 0, 0.5)").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 128);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// Modern syntax with slash-separated alpha: rgb(R G B / A)
#[test]
fn test_rgb_modern_with_alpha() {
    let c = color_from_css("color", "rgb(0 128 255 / 0.75)").unwrap();
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 128);
    assert_eq!(c.b, 255);
    assert_eq!(c.a, 191);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// Percentage values: "100% = 255"
#[test]
fn test_rgb_percentage() {
    let c = color_from_css("color", "rgb(100%, 0%, 50%)").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 128);
    assert_eq!(c.a, 255);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// "Values outside these ranges are not invalid, but are clamped."
#[test]
fn test_rgb_clamping() {
    let c = color_from_css("color", "rgb(300, -10, 128)").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 128);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#rgb-functions)
///
/// rgb() works for background-color too.
#[test]
fn test_rgb_background_color() {
    let c = color_from_css("background-color", "rgb(64, 128, 192)").unwrap();
    assert_eq!(c.r, 64);
    assert_eq!(c.g, 128);
    assert_eq!(c.b, 192);
}

/// [§ 4.1 The HSL Functions](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// Pure red: hsl(0, 100%, 50%)
#[test]
fn test_hsl_red() {
    let c = color_from_css("color", "hsl(0, 100%, 50%)").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// Pure green: hsl(120, 100%, 50%)
#[test]
fn test_hsl_green() {
    let c = color_from_css("color", "hsl(120, 100%, 50%)").unwrap();
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 255);
    assert_eq!(c.b, 0);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// Pure blue: hsl(240, 100%, 50%)
#[test]
fn test_hsl_blue() {
    let c = color_from_css("color", "hsl(240, 100%, 50%)").unwrap();
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 255);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// hsla() with alpha.
#[test]
fn test_hsla_with_alpha() {
    let c = color_from_css("color", "hsla(0, 100%, 50%, 0.5)").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 128);
}

/// [§ 4.1](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
///
/// Black and white from HSL.
/// hsl(0, 0%, 0%) = black, hsl(0, 0%, 100%) = white
#[test]
fn test_hsl_black_white() {
    let black = color_from_css("color", "hsl(0, 0%, 0%)").unwrap();
    assert_eq!(black.r, 0);
    assert_eq!(black.g, 0);
    assert_eq!(black.b, 0);

    let white = color_from_css("background-color", "hsl(0, 0%, 100%)").unwrap();
    assert_eq!(white.r, 255);
    assert_eq!(white.g, 255);
    assert_eq!(white.b, 255);
}
