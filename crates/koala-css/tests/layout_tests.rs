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
    layout_html_with_viewport(html, 800.0, 600.0)
}

/// Helper: parse HTML with a custom viewport width/height.
fn layout_html_with_viewport(html: &str, vw: f32, vh: f32) -> LayoutBox {
    use koala_css::cascade::compute_styles;
    use koala_css::{CSSParser, CSSTokenizer, Stylesheet};
    use std::collections::HashMap;
    let mut tokenizer = koala_html::HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let parser = koala_html::HTMLParser::new(tokenizer.into_tokens());
    let (dom, _) = parser.run_with_issues();

    // Extract <style> content from the HTML and parse as author stylesheet.
    let css_text = koala_css::extract_style_content(&dom);
    let author = if css_text.is_empty() {
        Stylesheet { rules: vec![] }
    } else {
        let mut css_tok = CSSTokenizer::new(css_text);
        css_tok.run();
        let mut css_parser = CSSParser::new(css_tok.into_tokens());
        css_parser.parse_stylesheet()
    };

    let ua = koala_css::ua_stylesheet::ua_stylesheet();
    let styles = compute_styles(&dom, ua, &author);

    let image_dims = HashMap::new();
    let mut layout_tree = LayoutBox::build_layout_tree(&dom, &styles, dom.root(), &image_dims)
        .expect("should produce a layout tree");

    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: vw,
        height: vh,
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
    let expected_h1_y = body_content_top + h1.dimensions.border.top + h1.dimensions.padding.top;
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
    let height_with_margin = (last_mb.y + last_mb.height) - body.dimensions.content.y;
    assert!(
        body.dimensions.content.height < height_with_margin - 0.5,
        "body content height ({:.1}) should be less than full margin-box height ({:.1})",
        body.dimensions.content.height,
        height_with_margin
    );
}

// ---------------------------------------------------------------------------
// Flexbox layout tests
//
// [§ 9 Flex Layout Algorithm](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
// ---------------------------------------------------------------------------

/// [§ 9 Flex Layout](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
///
/// Two children in a `display: flex` container are laid out side-by-side
/// (same y, different x) rather than stacked vertically.
#[test]
fn test_flex_row_basic() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; }\
         .item { width: 100px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div><div class='item'>B</div></div></body></html>",
    );

    // Document > html > body > div.flex
    let body = box_at_depth(&root, 2);
    let flex_container = &body.children[0];
    assert!(
        flex_container.children.len() >= 2,
        "flex container should have at least 2 children, got {}",
        flex_container.children.len()
    );

    let item_a = &flex_container.children[0];
    let item_b = &flex_container.children[1];

    // Items should be side-by-side: same y, different x.
    assert!(
        (item_a.dimensions.content.y - item_b.dimensions.content.y).abs() < 1.0,
        "flex items should have same y: A.y={:.1}, B.y={:.1}",
        item_a.dimensions.content.y,
        item_b.dimensions.content.y
    );
    assert!(
        item_b.dimensions.content.x > item_a.dimensions.content.x,
        "item B should be to the right of item A: A.x={:.1}, B.x={:.1}",
        item_a.dimensions.content.x,
        item_b.dimensions.content.x
    );
}

/// [§ 9.7 Resolving Flexible Lengths](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
///
/// Container width 300px, two items with flex-grow 1 and 2.
/// First item gets ~100px, second gets ~200px.
#[test]
fn test_flex_grow_distribution() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 300px; }\
         .a { flex-grow: 1; }\
         .b { flex-grow: 2; }\
         </style></head>\
         <body><div class='flex'><div class='a'>A</div><div class='b'>B</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    let item_a = &flex.children[0];
    let item_b = &flex.children[1];

    // With no initial base size (text content only), all free space is
    // distributed by flex-grow ratio (1:2), so item_b should be about
    // twice as wide as item_a.
    let ratio = item_b.dimensions.content.width / item_a.dimensions.content.width;
    assert!(
        (ratio - 2.0).abs() < 0.5,
        "item B should be ~2x item A width: A={:.1}, B={:.1}, ratio={:.2}",
        item_a.dimensions.content.width,
        item_b.dimensions.content.width,
        ratio
    );

    // Both should roughly sum to the container width (300px).
    let total = item_a.dimensions.margin_box().width + item_b.dimensions.margin_box().width;
    assert!(
        (total - 300.0).abs() < 5.0,
        "total flex item widths should be ~300px, got {total:.1}"
    );
}

/// [§ 9.7 Resolving Flexible Lengths](https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths)
///
/// Container 200px, two items each with flex-basis 150px and default
/// flex-shrink 1. Both should shrink equally to fit.
#[test]
fn test_flex_shrink() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 200px; }\
         .item { flex-basis: 150px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div><div class='item'>B</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    let item_a = &flex.children[0];
    let item_b = &flex.children[1];

    // Both items have equal flex-shrink (1) and equal base size (150px).
    // Total 300px > 200px available, so each should shrink to 100px.
    assert!(
        (item_a.dimensions.content.width - 100.0).abs() < 5.0,
        "item A should shrink to ~100px, got {:.1}",
        item_a.dimensions.content.width
    );
    assert!(
        (item_b.dimensions.content.width - 100.0).abs() < 5.0,
        "item B should shrink to ~100px, got {:.1}",
        item_b.dimensions.content.width
    );
}

/// [§ 4 Flex Items](https://www.w3.org/TR/css-flexbox-1/#flex-items)
///
/// "Margins of adjacent flex items do not collapse."
/// Adjacent flex items with margin: 10px each should have a 20px gap (10+10),
/// not collapsed to 10px.
#[test]
fn test_flex_no_margin_collapse() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; }\
         .item { width: 100px; margin: 10px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div><div class='item'>B</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    let item_a = &flex.children[0];
    let item_b = &flex.children[1];

    // Gap between A's right margin edge and B's left margin edge should be
    // A.margin_right + B.margin_left = 10 + 10 = 20, NOT collapsed to 10.
    let a_right_edge = item_a.dimensions.margin_box().x + item_a.dimensions.margin_box().width;
    let b_left_edge = item_b.dimensions.margin_box().x;
    let gap = b_left_edge - a_right_edge;

    assert!(
        gap.abs() < 1.0,
        "flex items should abut (margin boxes should be adjacent), gap={gap:.1}"
    );

    // The actual space between content boxes should be sum of margins.
    let content_gap = item_b.dimensions.content.x
        - (item_a.dimensions.content.x + item_a.dimensions.content.width);
    let expected_gap = item_a.dimensions.margin.right
        + item_a.dimensions.border.right
        + item_a.dimensions.padding.right
        + item_b.dimensions.margin.left
        + item_b.dimensions.border.left
        + item_b.dimensions.padding.left;
    assert!(
        (content_gap - expected_gap).abs() < 1.0,
        "content gap should be {expected_gap:.1} (no collapsing), got {content_gap:.1}"
    );
}

/// [§ 8.2 justify-content: center](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
///
/// Items narrower than container are offset from the left when centered.
#[test]
fn test_flex_justify_center() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; justify-content: center; }\
         .item { width: 50px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div><div class='item'>B</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    let item_a = &flex.children[0];
    let item_b = &flex.children[1];

    // Free space = 400 - 100 = 300. Center offset = 150.
    // item_a's margin-box x should be container.content.x + 150.
    let expected_offset = (400.0 - 100.0) / 2.0; // 150
    let actual_offset = item_a.dimensions.margin_box().x - flex.dimensions.content.x;
    assert!(
        (actual_offset - expected_offset).abs() < 5.0,
        "center offset should be ~{expected_offset:.1}, got {actual_offset:.1}"
    );

    // B should be right after A.
    let a_right = item_a.dimensions.margin_box().x + item_a.dimensions.margin_box().width;
    assert!(
        (item_b.dimensions.margin_box().x - a_right).abs() < 1.0,
        "item B should be immediately after item A"
    );
}

/// [§ 8.2 justify-content: space-between](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
///
/// 3 items: first at left edge, last at right edge, middle centered.
#[test]
fn test_flex_justify_space_between() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 300px; justify-content: space-between; }\
         .item { width: 50px; }\
         </style></head>\
         <body><div class='flex'>\
         <div class='item'>A</div><div class='item'>B</div><div class='item'>C</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    assert_eq!(flex.children.len(), 3);

    let item_a = &flex.children[0];
    let item_c = &flex.children[2];

    // First item at left edge.
    let left_offset = item_a.dimensions.margin_box().x - flex.dimensions.content.x;
    assert!(
        left_offset.abs() < 1.0,
        "first item should be at left edge, offset={left_offset:.1}"
    );

    // Last item at right edge.
    let right_edge = item_c.dimensions.margin_box().x + item_c.dimensions.margin_box().width;
    let container_right = flex.dimensions.content.x + flex.dimensions.content.width;
    assert!(
        (right_edge - container_right).abs() < 2.0,
        "last item should be at right edge: item_right={right_edge:.1}, container_right={container_right:.1}"
    );
}

/// [§ 9.9 Cross Size Determination](https://www.w3.org/TR/css-flexbox-1/#algo-cross-container)
///
/// Container with no explicit height gets the height of the tallest child.
#[test]
fn test_flex_container_auto_height() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; }\
         .short { width: 100px; height: 50px; }\
         .tall { width: 100px; height: 120px; }\
         </style></head>\
         <body><div class='flex'>\
         <div class='short'>A</div><div class='tall'>B</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];

    // Container height should be the tallest child's margin-box height.
    let tallest = flex
        .children
        .iter()
        .map(|c| c.dimensions.margin_box().height)
        .fold(0.0_f32, f32::max);

    assert!(
        (flex.dimensions.content.height - tallest).abs() < 1.0,
        "flex container auto height should be ~{tallest:.1}, got {:.1}",
        flex.dimensions.content.height
    );
}

/// [§ 7.1 'flex-basis'](https://www.w3.org/TR/css-flexbox-1/#flex-basis-property)
///
/// "If the specified flex-basis is not auto, the flex base size is the
/// computed value of the flex-basis property."
///
/// flex-basis: 100px should override width: 50px.
#[test]
fn test_flex_explicit_basis() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; }\
         .item { width: 50px; flex-basis: 100px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    let item = &flex.children[0];

    // flex-basis overrides width for the flex base size.
    // With no flex-grow, the item keeps its base size of 100px.
    assert!(
        (item.dimensions.content.width - 100.0).abs() < 5.0,
        "item width should be ~100px (from flex-basis), got {:.1}",
        item.dimensions.content.width
    );
}

/// [§ 9.2 step 3](https://www.w3.org/TR/css-flexbox-1/#algo-main-item)
///
/// Items with text content and no explicit width get width from
/// content-based measurement (measure_content_size).
#[test]
fn test_flex_content_based_sizing() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 800px; }\
         </style></head>\
         <body><div class='flex'><div>Short</div><div>A much longer piece of text</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex = &body.children[0];
    assert!(flex.children.len() >= 2);

    let short_item = &flex.children[0];
    let long_item = &flex.children[1];

    // The longer text item should be wider than the shorter one.
    assert!(
        long_item.dimensions.content.width > short_item.dimensions.content.width,
        "longer text item should be wider: short={:.1}, long={:.1}",
        short_item.dimensions.content.width,
        long_item.dimensions.content.width
    );

    // Both should have non-zero width.
    assert!(
        short_item.dimensions.content.width > 0.0,
        "short item should have non-zero width"
    );
    assert!(
        long_item.dimensions.content.width > 0.0,
        "long item should have non-zero width"
    );
}

// ---------------------------------------------------------------------------
// Relative positioning tests
//
// [§ 9.4.3 Relative positioning](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
//
// "Once a box has been laid out according to the normal flow, it may be
// shifted relative to its normal position."
// ---------------------------------------------------------------------------

/// position: relative with left offset shifts the box to the right.
#[test]
fn test_relative_position_left_offset() {
    let root = layout_html(
        "<html><head><style>\
         .rel { position: relative; left: 20px; }\
         </style></head>\
         <body><div>Static</div><div class='rel'>Relative</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let static_div = &body.children[0];
    let relative_div = &body.children[1];

    // Both should have the same normal-flow x-position base, but the
    // relative div is shifted right by 20px.
    let expected_offset = 20.0;
    let actual_offset = relative_div.dimensions.content.x - static_div.dimensions.content.x;
    assert!(
        (actual_offset - expected_offset).abs() < 0.1,
        "relative div should be 20px right of static div, got offset {actual_offset:.1}"
    );
}

/// position: relative with top offset shifts the box downward.
#[test]
fn test_relative_position_top_offset() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .rel { position: relative; top: 15px; }\
         </style></head>\
         <body><div class='rel'>Moved</div><div>After</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let moved_div = &body.children[0];
    let after_div = &body.children[1];

    // "Moved" div should be shifted down by 15px from its normal position.
    // "After" div should NOT be affected by the offset — it should be
    // positioned as if "Moved" were in its normal-flow position.
    //
    // The body's content box is the containing block. The normal-flow y
    // for the first child is body.content.y + margin_top.
    let body_content_y = body.dimensions.content.y;
    assert!(
        (moved_div.dimensions.content.y - body_content_y - 15.0).abs() < 0.1,
        "moved div should be 15px below body content top, got y={:.1} (body.y={body_content_y:.1})",
        moved_div.dimensions.content.y
    );

    // "After" div should be positioned as if the first div were NOT offset.
    // Its y should be body.content.y + first_div_height (normal flow).
    assert!(
        after_div.dimensions.content.y < moved_div.dimensions.content.y + moved_div.dimensions.content.height,
        "after div should overlap with moved div since relative positioning \
         does not affect subsequent siblings"
    );
}

/// position: relative with right offset shifts the box to the left.
#[test]
fn test_relative_position_right_offset() {
    let root = layout_html(
        "<html><head><style>\
         .rel { position: relative; right: 10px; }\
         </style></head>\
         <body><div>Static</div><div class='rel'>Relative</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let static_div = &body.children[0];
    let relative_div = &body.children[1];

    // right: 10px means "shift left by 10px"
    let actual_offset = relative_div.dimensions.content.x - static_div.dimensions.content.x;
    assert!(
        (actual_offset - (-10.0)).abs() < 0.1,
        "relative div should be 10px left of static div, got offset {actual_offset:.1}"
    );
}

/// position: relative with bottom offset shifts the box upward.
#[test]
fn test_relative_position_bottom_offset() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .rel { position: relative; bottom: 5px; }\
         </style></head>\
         <body><div>Before</div><div class='rel'>Moved</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let before_div = &body.children[0];
    let moved_div = &body.children[1];

    // bottom: 5px means "shift up by 5px". The normal-flow y of the second
    // div is after the first div. The offset should reduce y by 5.
    let normal_flow_y =
        before_div.dimensions.content.y + before_div.dimensions.content.height;
    let actual_y = moved_div.dimensions.content.y;
    assert!(
        (actual_y - (normal_flow_y - 5.0)).abs() < 0.1,
        "moved div should be 5px above normal flow position, got y={actual_y:.1} \
         (normal={normal_flow_y:.1})"
    );
}

/// position: relative with no offsets should not move the box.
#[test]
fn test_relative_position_no_offsets() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .rel { position: relative; }\
         </style></head>\
         <body><div>Static</div><div class='rel'>Relative</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let static_div = &body.children[0];
    let relative_div = &body.children[1];

    // No offsets means no movement — same x position.
    assert!(
        (relative_div.dimensions.content.x - static_div.dimensions.content.x).abs() < 0.1,
        "relative div with no offsets should have same x as static div"
    );
}

/// Over-constrained: when both left and right are set, left wins (LTR).
#[test]
fn test_relative_position_overconstrained_horizontal() {
    let root = layout_html(
        "<html><head><style>\
         .rel { position: relative; left: 30px; right: 10px; }\
         </style></head>\
         <body><div>Static</div><div class='rel'>Relative</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(body.children.len() >= 2);

    let static_div = &body.children[0];
    let relative_div = &body.children[1];

    // [§ 9.4.3]: "If neither 'left' nor 'right' is 'auto', the position is
    // over-constrained... If 'direction' is 'ltr', the value of 'left' wins."
    let actual_offset = relative_div.dimensions.content.x - static_div.dimensions.content.x;
    assert!(
        (actual_offset - 30.0).abs() < 0.1,
        "left should win over right in LTR, got offset {actual_offset:.1}"
    );
}

/// Over-constrained: when both top and bottom are set, top wins.
#[test]
fn test_relative_position_overconstrained_vertical() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .rel { position: relative; top: 25px; bottom: 10px; }\
         </style></head>\
         <body><div class='rel'>Moved</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    assert!(!body.children.is_empty());

    let moved_div = &body.children[0];

    // [§ 9.4.3]: "If neither is 'auto', 'bottom' is ignored."
    let body_content_y = body.dimensions.content.y;
    assert!(
        (moved_div.dimensions.content.y - body_content_y - 25.0).abs() < 0.1,
        "top should win over bottom, got y={:.1} (body.y={body_content_y:.1})",
        moved_div.dimensions.content.y
    );
}

// ---------------------------------------------------------------------------
// Absolute positioning tests
//
// [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
//
// "In the absolute positioning model, a box is removed from the normal
// flow entirely and assigned a position with respect to a containing block."
// ---------------------------------------------------------------------------

/// [§ 10.3.7 / § 10.6.4](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
///
/// An absolutely positioned element with explicit top, left, width, height
/// should be placed at the specified position relative to the positioned
/// parent's padding box.
#[test]
fn test_absolute_explicit_position() {
    let root = layout_html(
        "<html><head><style>\
         .container { position: relative; width: 400px; height: 300px; margin: 0; padding: 0; }\
         .abs { position: absolute; top: 10px; left: 20px; width: 100px; height: 50px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='container'><div class='abs'>Abs</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];
    let abs_child = &container.children[0];

    // The abs child should be positioned relative to container's padding box.
    let container_padding_x = container.dimensions.content.x - container.dimensions.padding.left;
    let container_padding_y = container.dimensions.content.y - container.dimensions.padding.top;

    // content.x = container_padding_x + left(20) + margin(0) + border(0) + padding(0)
    assert!(
        (abs_child.dimensions.content.x - (container_padding_x + 20.0)).abs() < 1.0,
        "abs child x should be container_padding.x + 20, got x={:.1} (expected {:.1})",
        abs_child.dimensions.content.x,
        container_padding_x + 20.0
    );

    // content.y = container_padding_y + top(10) + margin(0) + border(0) + padding(0)
    assert!(
        (abs_child.dimensions.content.y - (container_padding_y + 10.0)).abs() < 1.0,
        "abs child y should be container_padding.y + 10, got y={:.1} (expected {:.1})",
        abs_child.dimensions.content.y,
        container_padding_y + 10.0
    );

    assert!(
        (abs_child.dimensions.content.width - 100.0).abs() < 1.0,
        "abs child width should be 100, got {:.1}",
        abs_child.dimensions.content.width
    );

    assert!(
        (abs_child.dimensions.content.height - 50.0).abs() < 1.0,
        "abs child height should be 50, got {:.1}",
        abs_child.dimensions.content.height
    );
}

/// [§ 9.3](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
///
/// "In the absolute positioning model, a box is removed from the normal
/// flow entirely." — siblings should lay out as if the absolute child
/// doesn't exist.
#[test]
fn test_absolute_removed_from_flow() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .container { position: relative; width: 400px; }\
         .abs { position: absolute; top: 0; left: 0; width: 100px; height: 100px; }\
         .normal { width: 400px; height: 50px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='container'>\
         <div class='abs'>Absolute</div>\
         <div class='normal'>Normal</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    // Find the normal-flow child (not absolute).
    let normal_child = container
        .children
        .iter()
        .find(|c| c.position_type == koala_css::PositionType::Static)
        .expect("should have a static child");

    // The normal child should be at the top of the container's content area,
    // as if the absolute child doesn't exist.
    assert!(
        (normal_child.dimensions.content.y - container.dimensions.content.y).abs() < 1.0,
        "normal child should be at container top: child.y={:.1}, container.y={:.1}",
        normal_child.dimensions.content.y,
        container.dimensions.content.y
    );
}

/// [§ 10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
///
/// With left and right specified and width auto: width is computed from
/// the constraint equation:
///   width = cb_width - left - right - margin - border - padding
#[test]
fn test_absolute_left_right_computed_width() {
    let root = layout_html(
        "<html><head><style>\
         .container { position: relative; width: 400px; height: 200px; margin: 0; padding: 0; }\
         .abs { position: absolute; left: 50px; right: 50px; top: 0; height: 40px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='container'><div class='abs'>Stretched</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];
    let abs_child = &container.children[0];

    // width = 400 - 50 - 50 - 0 (margins) - 0 (borders) - 0 (padding) = 300
    assert!(
        (abs_child.dimensions.content.width - 300.0).abs() < 1.0,
        "abs child width should be 300 (400 - 50 - 50), got {:.1}",
        abs_child.dimensions.content.width
    );
}

/// [§ 10.6.3](https://www.w3.org/TR/CSS2/visudet.html#normal-block)
///
/// Auto height of the parent should only include in-flow children,
/// not absolute children.
#[test]
fn test_auto_height_ignores_absolute_children() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .container { position: relative; width: 400px; }\
         .abs { position: absolute; top: 0; left: 0; width: 100px; height: 500px; }\
         .normal { width: 400px; height: 50px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='container'>\
         <div class='abs'>Tall absolute</div>\
         <div class='normal'>Normal</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    // Container's auto height should come from the normal child (50px),
    // NOT the absolute child (500px).
    assert!(
        container.dimensions.content.height < 100.0,
        "container auto height should be ~50 (from normal child), not 500; got {:.1}",
        container.dimensions.content.height
    );

    assert!(
        (container.dimensions.content.height - 50.0).abs() < 1.0,
        "container auto height should be 50, got {:.1}",
        container.dimensions.content.height
    );
}

/// [§ 10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
///
/// Over-constrained horizontal: when left, width, and right are all
/// specified and no margins are auto, 'right' is ignored (LTR).
#[test]
fn test_absolute_overconstrained_horizontal() {
    let root = layout_html(
        "<html><head><style>\
         .container { position: relative; width: 400px; height: 200px; margin: 0; padding: 0; }\
         .abs { position: absolute; left: 10px; right: 10px; width: 200px; top: 0; height: 40px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='container'><div class='abs'>Over</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];
    let abs_child = &container.children[0];

    // Width should be the specified 200px.
    assert!(
        (abs_child.dimensions.content.width - 200.0).abs() < 1.0,
        "abs child width should be 200, got {:.1}",
        abs_child.dimensions.content.width
    );

    // Left should be honored (10px).
    let container_padding_x = container.dimensions.content.x - container.dimensions.padding.left;
    assert!(
        (abs_child.dimensions.content.x - (container_padding_x + 10.0)).abs() < 1.0,
        "abs child should respect left: 10px, got x={:.1}",
        abs_child.dimensions.content.x
    );
}

/// [§ 10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
///
/// "If both 'margin-left' and 'margin-right' are 'auto', solve the equation
/// under the extra constraint that the two margins get equal values..."
/// This centers the element horizontally.
#[test]
fn test_absolute_auto_margins_centering() {
    let root = layout_html(
        "<html><head><style>\
         .container { position: relative; width: 400px; height: 200px; margin: 0; padding: 0; }\
         .abs { position: absolute; left: 0; right: 0; width: 200px; top: 0; height: 40px; \
                margin-left: auto; margin-right: auto; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='container'><div class='abs'>Centered</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];
    let abs_child = &container.children[0];

    // With left:0, right:0, width:200, margin:auto on both sides:
    // remaining = 400 - 0 - 200 - 0 = 200
    // each margin = 100
    assert!(
        (abs_child.dimensions.margin.left - 100.0).abs() < 1.0,
        "margin-left should be 100 for centering, got {:.1}",
        abs_child.dimensions.margin.left
    );
    assert!(
        (abs_child.dimensions.margin.right - 100.0).abs() < 1.0,
        "margin-right should be 100 for centering, got {:.1}",
        abs_child.dimensions.margin.right
    );

    // The content box should be centered.
    let container_padding_x = container.dimensions.content.x - container.dimensions.padding.left;
    let expected_x = container_padding_x + 0.0 + 100.0; // left + margin-left
    assert!(
        (abs_child.dimensions.content.x - expected_x).abs() < 1.0,
        "abs child should be centered at x={expected_x:.1}, got {:.1}",
        abs_child.dimensions.content.x
    );
}
