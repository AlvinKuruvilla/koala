//! CSS Cascading and Style Computation
//!
//! This module implements style computation per
//! [CSS Cascading and Inheritance Level 4](https://www.w3.org/TR/css-cascade-4/).

use std::collections::HashMap;

use crate::parser::{Rule, StyleRule, Stylesheet};
use crate::selector::{ParsedSelector, Specificity, parse_selector};
use crate::style::ComputedStyle;
use koala_common::warning::warn_once;
use koala_dom::{DomTree, NodeId, NodeType};

/// [§ 6 Cascading](https://www.w3.org/TR/css-cascade-4/#cascading)
///
/// A matched rule with its specificity for cascade ordering.
struct MatchedRule<'a> {
    specificity: Specificity,
    rule: &'a StyleRule,
}

/// [§ 6 Cascading](https://www.w3.org/TR/css-cascade-4/#cascading)
/// "The cascade takes an unordered list of declared values for a given property
/// on a given element, sorts them by their declaration's precedence..."
///
/// Compute styles for the entire DOM tree given a stylesheet.
/// Returns a map from NodeId to computed style.
pub fn compute_styles(tree: &DomTree, stylesheet: &Stylesheet) -> HashMap<NodeId, ComputedStyle> {
    let mut styles = HashMap::new();

    // Parse all selectors upfront
    let parsed_rules: Vec<(ParsedSelector, &StyleRule)> = stylesheet
        .rules
        .iter()
        .filter_map(|rule| match rule {
            Rule::Style(style_rule) => {
                // Try each selector in the rule (comma-separated selectors)
                // For MVP, we just use the first valid one
                let result = style_rule
                    .selectors
                    .iter()
                    .find_map(|sel| parse_selector(&sel.text).map(|parsed| (parsed, style_rule)));
                // Warn if all selectors in this rule failed to parse
                if result.is_none() && !style_rule.selectors.is_empty() {
                    let selector_text = style_rule
                        .selectors
                        .iter()
                        .map(|s| s.text.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    warn_once(
                        "CSS",
                        &format!("failed to parse selector '{selector_text}'"),
                    );
                }
                result
            }
            Rule::At(_) => None, // Skip at-rules for MVP
        })
        .collect();

    // Start with default inherited style (none)
    let initial_style = ComputedStyle::default();
    compute_node_styles(
        tree,
        tree.root(),
        &parsed_rules,
        &initial_style,
        &mut styles,
    );

    styles
}

/// [§ 6 Cascading](https://www.w3.org/TR/css-cascade-4/#cascading)
///
/// Recursively compute styles for a node and its children.
/// Applies cascade sorting and specificity rules per the spec.
fn compute_node_styles(
    tree: &DomTree,
    id: NodeId,
    rules: &[(ParsedSelector, &StyleRule)],
    inherited: &ComputedStyle,
    styles: &mut HashMap<NodeId, ComputedStyle>,
) {
    let Some(node) = tree.get(id) else { return };

    match &node.node_type {
        NodeType::Element(_element_data) => {
            // [§ 7 Inheritance](https://www.w3.org/TR/css-cascade-4/#inheriting)
            // Start with inherited styles
            let mut computed = inherit_styles(inherited);

            // [§ 6.4 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
            // Find all matching rules using tree-aware matching for combinator support
            let mut matched: Vec<MatchedRule> = rules
                .iter()
                .filter(|(selector, _)| selector.matches_in_tree(tree, id))
                .map(|(selector, rule)| MatchedRule {
                    specificity: selector.specificity,
                    rule,
                })
                .collect();

            // [§ 6.4.3 Specificity](https://www.w3.org/TR/css-cascade-4/#cascade-specificity)
            // Sort by specificity (lower first, so later ones override)
            matched.sort_by(|a, b| a.specificity.cmp(&b.specificity));

            // Apply declarations in order
            for m in matched {
                for decl in &m.rule.declarations {
                    computed.apply_declaration(decl);
                }
            }

            // Store the computed style
            let _ = styles.insert(id, computed.clone());

            // Recurse to children with this element's computed style as inherited
            for &child_id in tree.children(id) {
                compute_node_styles(tree, child_id, rules, &computed, styles);
            }
        }
        NodeType::Document => {
            // Document doesn't have styles itself, but pass through to children
            for &child_id in tree.children(id) {
                compute_node_styles(tree, child_id, rules, inherited, styles);
            }
        }
        NodeType::Text(_) | NodeType::Comment(_) => {
            // Text and comment nodes don't have styles applied directly.
            // They inherit from their parent element when rendered.
        }
    }
}

/// [§ 7.1 Inherited Properties](https://www.w3.org/TR/css-cascade-4/#inherited-property)
/// "Some properties are inherited from an ancestor element to its descendants."
///
/// Create a new style inheriting appropriate properties from the parent.
fn inherit_styles(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        // Inherited properties
        // [§ 3.1 color](https://www.w3.org/TR/css-color-4/#the-color-property)
        // "Inherited: yes"
        color: parent.color.clone(),

        // [§ 3.1 font-family](https://www.w3.org/TR/css-fonts-4/#font-family-prop)
        // "Inherited: yes"
        font_family: parent.font_family.clone(),

        // [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
        // "Inherited: yes"
        font_size: parent.font_size.clone(),

        // [§ 4.2 line-height](https://www.w3.org/TR/css-inline-3/#line-height-property)
        // "Inherited: yes"
        line_height: parent.line_height,

        // Non-inherited properties start as None
        //
        // [§ 2 display](https://www.w3.org/TR/css-display-3/#the-display-properties)
        // "Inherited: no"
        display: None,
        display_none: false,

        // [§ 3.2 background-color](https://www.w3.org/TR/css-backgrounds-3/#background-color)
        // "Inherited: no"
        background_color: None,

        // [§ 6 Box Model](https://www.w3.org/TR/css-box-4/)
        // "Inherited: no"
        margin_top: None,
        margin_right: None,
        margin_bottom: None,
        margin_left: None,
        padding_top: None,
        padding_right: None,
        padding_bottom: None,
        padding_left: None,

        // Borders are not inherited
        border_top: None,
        border_right: None,
        border_bottom: None,
        border_left: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CSSParser;
    use crate::tokenizer::CSSTokenizer;
    use koala_dom::{AttributesMap, ElementData};

    fn parse_css(css: &str) -> Stylesheet {
        let mut tokenizer = CSSTokenizer::new(css.to_string());
        tokenizer.run();
        let mut parser = CSSParser::new(tokenizer.into_tokens());
        parser.parse_stylesheet()
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

        let styles = compute_styles(&tree, &stylesheet);

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

        let styles = compute_styles(&tree, &stylesheet);

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

        let styles = compute_styles(&tree, &stylesheet);

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

        let styles = compute_styles(&tree, &stylesheet);

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

        let styles = compute_styles(&tree, &stylesheet);

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

        let styles = compute_styles(&tree, &stylesheet);

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

        let styles = compute_styles(&tree, &stylesheet);

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
        if let Some(crate::style::AutoLength::Length(crate::style::LengthValue::Px(v))) =
            &div_style.margin_top
        {
            assert!((v - 20.0).abs() < 0.01);
        }
        if let Some(crate::style::LengthValue::Px(v)) = &div_style.padding_top {
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

        let styles = compute_styles(&tree, &stylesheet);

        // Span inside h1 should inherit h1's font-size (32px)
        let span_style = styles.get(&span_id).unwrap();
        assert!(span_style.font_size.is_some());
        if let Some(crate::style::LengthValue::Px(v)) = &span_style.font_size {
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

        let styles = compute_styles(&tree, &stylesheet);

        let div_style = styles.get(&div_id).unwrap();

        // All four borders should be set
        assert!(div_style.border_top.is_some());
        assert!(div_style.border_right.is_some());
        assert!(div_style.border_bottom.is_some());
        assert!(div_style.border_left.is_some());

        // Verify border properties
        let border = div_style.border_top.as_ref().unwrap();
        match border.width {
            crate::style::LengthValue::Px(w) => {
                assert!(
                    (w - 1.0).abs() < 0.01,
                    "Expected border width ~1.0px, got {w}px"
                )
            }
            crate::style::LengthValue::Em(_) => {
                panic!("Expected border width in Px, got Em (should have been resolved)")
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
        use crate::extract_style_content;
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
        let styles = compute_styles(&tree, &stylesheet);

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
}
