//! Integration tests for CSS layout types.

use koala_css::layout::default_display_for_element;
use koala_css::{ApproximateFontMetrics, DisplayValue, LayoutBox, Rect};

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

// ---------------------------------------------------------------------------
// Margin collapsing tests
//
// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
//
// "When two or more margins collapse, the resulting margin width is the
// maximum of the collapsing margins' widths."
// ---------------------------------------------------------------------------

/// Helper: parse HTML via koala-browser, build and compute layout, return the
/// root layout box with dimensions filled in.
fn layout_html(html: &str) -> LayoutBox {
    use koala_css::cascade::compute_styles;
    use koala_css::Stylesheet;
    use std::collections::HashMap;
    let mut tokenizer = koala_html::HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let parser = koala_html::HTMLParser::new(tokenizer.into_tokens());
    let (dom, _) = parser.run_with_issues();

    // Use UA stylesheet + empty author stylesheet
    let ua = koala_css::ua_stylesheet::ua_stylesheet();
    let empty = Stylesheet { rules: vec![] };
    let styles = compute_styles(&dom, ua, &empty);

    let image_dims = HashMap::new();
    let mut layout_tree = LayoutBox::build_layout_tree(&dom, &styles, dom.root(), &image_dims)
        .expect("should produce a layout tree");

    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    layout_tree.layout(viewport, viewport, &ApproximateFontMetrics);

    layout_tree
}

/// Helper: find the first box at a given depth in the tree (0 = root).
fn box_at_depth(root: &LayoutBox, depth: usize) -> &LayoutBox {
    if depth == 0 {
        return root;
    }
    box_at_depth(&root.children[0], depth - 1)
}

/// [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
///
/// Two adjacent siblings with positive margins: the gap between their border
/// edges should be max(mb_A, mt_B), not mb_A + mt_B.
#[test]
fn test_sibling_margin_collapsing_both_positive() {
    // h1 has margin 0.67em top/bottom (≈ 21.44px at 32px font-size via UA)
    // p has margin 1em top/bottom (= 16px at 16px font-size via UA)
    let root = layout_html("<body><h1>A</h1><p>B</p></body>");

    // Document > html > body > [h1, p]
    let body = box_at_depth(&root, 2);
    assert!(
        body.children.len() >= 2,
        "body should have at least 2 children, got {}",
        body.children.len()
    );

    let h1 = &body.children[0];
    let p = &body.children[1];

    let h1_border_bottom = h1.dimensions.content.y + h1.dimensions.content.height;
    let p_border_top = p.dimensions.content.y;
    let gap = p_border_top - h1_border_bottom;

    let h1_mb = h1.dimensions.margin.bottom;
    let p_mt = p.dimensions.margin.top;

    // Gap should be max(h1_mb, p_mt), not h1_mb + p_mt
    let expected = h1_mb.max(p_mt);
    assert!(
        (gap - expected).abs() < 1.0,
        "gap between h1 and p should be ~{expected:.1} (collapsed), got {gap:.1} \
         (h1 margin-bottom={h1_mb:.1}, p margin-top={p_mt:.1})"
    );
}

/// Two paragraphs with equal margins: collapsed margin = the margin value itself.
#[test]
fn test_sibling_margin_collapsing_equal_margins() {
    let root = layout_html("<body><p>A</p><p>B</p></body>");

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let p1 = &body.children[0];
    let p2 = &body.children[1];

    let p1_border_bottom = p1.dimensions.content.y + p1.dimensions.content.height;
    let p2_border_top = p2.dimensions.content.y;
    let gap = p2_border_top - p1_border_bottom;

    // Both p elements have 16px top and bottom margin (UA stylesheet: 1em at 16px)
    // Collapsed: max(16, 16) = 16
    let p1_mb = p1.dimensions.margin.bottom;
    let p2_mt = p2.dimensions.margin.top;
    let expected = p1_mb.max(p2_mt);

    assert!(
        (gap - expected).abs() < 1.0,
        "gap between two <p> elements should be ~{expected:.1} (collapsed), got {gap:.1}"
    );

    // Also verify it's NOT the sum (which would be 32)
    let uncollapsed = p1_mb + p2_mt;
    assert!(
        gap < uncollapsed - 0.5,
        "gap ({gap:.1}) should be less than uncollapsed sum ({uncollapsed:.1})"
    );
}

/// Auto-height parent should reflect collapsed positions, not naive margin-box sums.
///
/// [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
///
/// With parent-child bottom margin collapsing, the last child's bottom
/// margin is excluded from the parent's content height (it becomes part
/// of the parent's own margin instead).
#[test]
fn test_auto_height_with_collapsed_margins() {
    let root = layout_html("<body><p>A</p><p>B</p><p>C</p></body>");

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 3);

    let last = body.children.last().unwrap();
    let last_mb = last.dimensions.margin_box();
    let last_child_effective_mb = last.effective_margin_bottom();

    // Body height goes from content top to the last child's margin-box
    // bottom, minus the last child's bottom margin (which collapsed with
    // body's own bottom margin per § 8.3.1).
    let expected_height =
        (last_mb.y + last_mb.height) - body.dimensions.content.y - last_child_effective_mb;

    assert!(
        (body.dimensions.content.height - expected_height).abs() < 1.0,
        "body auto height should be ~{expected_height:.1}, got {:.1}",
        body.dimensions.content.height
    );

    // Verify body has a collapsed bottom margin
    assert!(
        body.collapsed_margin_bottom.is_some(),
        "body should have collapsed_margin_bottom set"
    );
}

// ---------------------------------------------------------------------------
// Parent-child margin collapsing tests
//
// [§ 8.3.1 Collapsing margins](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
//
// "The top margin of an in-flow block element collapses with its first
// in-flow block-level child's top margin value if the element has no
// top border, no top padding, and the child has no clearance."
// ---------------------------------------------------------------------------

/// [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
///
/// Parent-child top margin collapsing: body (8px margin) with first child
/// h1 (~21.44px margin-top). The collapsed margin should be max(8, 21.44).
/// The h1 should sit flush at body's content top, not offset by its own
/// margin-top.
#[test]
fn test_parent_child_top_margin_collapsing() {
    let root = layout_html("<body><h1>Title</h1></body>");

    // Document > html > body
    let body = box_at_depth(&root, 2);
    assert!(!body.children.is_empty());

    let h1 = &body.children[0];

    // h1 should sit flush at body's content top — its margin-top has
    // collapsed with body's margin-top, so it doesn't add extra space
    // inside body's content area.
    let h1_content_top = h1.dimensions.content.y;
    let body_content_top = body.dimensions.content.y;

    // The h1's content.y should be body_content_top + h1.border.top +
    // h1.padding.top (with margin absorbed into the parent).
    let expected_h1_y =
        body_content_top + h1.dimensions.border.top + h1.dimensions.padding.top;
    assert!(
        (h1_content_top - expected_h1_y).abs() < 1.0,
        "h1 content.y should be ~{expected_h1_y:.1} (flush with body content top), \
         got {h1_content_top:.1}"
    );

    // Body should have a collapsed_margin_top set
    assert!(
        body.collapsed_margin_top.is_some(),
        "body should have collapsed_margin_top set"
    );

    // The collapsed margin should be max(body_mt, h1_mt)
    let body_mt = body.dimensions.margin.top;
    let h1_mt = h1.dimensions.margin.top;
    let expected_collapsed = body_mt.max(h1_mt);
    assert!(
        (body.collapsed_margin_top.unwrap() - expected_collapsed).abs() < 1.0,
        "collapsed_margin_top should be ~{expected_collapsed:.1}, got {:.1}",
        body.collapsed_margin_top.unwrap()
    );
}

/// [§ 8.3.1](https://www.w3.org/TR/CSS2/box.html#collapsing-margins)
///
/// "The bottom margin of an in-flow block-level element collapses with
/// the bottom margin of its last in-flow block-level child if the element
/// has no bottom padding and no bottom border."
#[test]
fn test_parent_child_bottom_margin_collapsing() {
    let root = layout_html("<body><p>text</p></body>");

    let body = box_at_depth(&root, 2);
    assert!(!body.children.is_empty());

    let p = body.children.last().unwrap();

    // Body should have a collapsed bottom margin
    assert!(
        body.collapsed_margin_bottom.is_some(),
        "body should have collapsed_margin_bottom set"
    );

    // The collapsed margin should be max(body_mb, p_mb)
    let body_mb = body.dimensions.margin.bottom;
    let p_mb = p.effective_margin_bottom();
    let expected_collapsed = body_mb.max(p_mb);
    assert!(
        (body.collapsed_margin_bottom.unwrap() - expected_collapsed).abs() < 1.0,
        "collapsed_margin_bottom should be ~{expected_collapsed:.1}, got {:.1}",
        body.collapsed_margin_bottom.unwrap()
    );

    // Body's content height should exclude the last child's collapsed margin
    let last_mb = p.dimensions.margin_box();
    let height_with_margin =
        (last_mb.y + last_mb.height) - body.dimensions.content.y;
    assert!(
        body.dimensions.content.height < height_with_margin - 0.5,
        "body content height ({:.1}) should be less than full margin-box height ({:.1})",
        body.dimensions.content.height,
        height_with_margin
    );
}
