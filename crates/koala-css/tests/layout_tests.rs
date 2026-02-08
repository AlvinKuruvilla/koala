//! Integration tests for CSS layout types.

use koala_css::layout::default_display_for_element;
use koala_css::{
    ApproximateFontMetrics, DisplayValue, FragmentContent, InnerDisplayType, LayoutBox,
    OuterDisplayType, Rect, TextRun,
};

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
    layout_tree.layout(viewport, viewport, &ApproximateFontMetrics, viewport);

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
        after_div.dimensions.content.y
            < moved_div.dimensions.content.y + moved_div.dimensions.content.height,
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
    let normal_flow_y = before_div.dimensions.content.y + before_div.dimensions.content.height;
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

// ---------------------------------------------------------------------------
// min-width / max-width / min-height / max-height tests
//
// [§ 10.4 Minimum and maximum widths](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
// [§ 10.7 Minimum and maximum heights](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
// ---------------------------------------------------------------------------

/// [§ 10.4](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
///
/// max-width should clamp the computed width of a block element.
#[test]
fn test_max_width_clamps_block() {
    let root = layout_html(
        "<html><head><style>\
         .box { max-width: 200px; }\
         </style></head>\
         <body><div class='box'>Content</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // Without max-width, the div would be body's content width (~784px in
    // an 800px viewport with 8px body margin). With max-width: 200px, it
    // should be clamped to 200px.
    assert!(
        (div.dimensions.content.width - 200.0).abs() < 1.0,
        "div width should be clamped to 200px by max-width, got {:.1}",
        div.dimensions.content.width
    );
}

/// [§ 10.4](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
///
/// min-width should expand a narrow explicit width.
#[test]
fn test_min_width_expands_narrow() {
    let root = layout_html(
        "<html><head><style>\
         .box { width: 50px; min-width: 100px; }\
         </style></head>\
         <body><div class='box'>Content</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // width: 50px is less than min-width: 100px, so min-width wins.
    assert!(
        (div.dimensions.content.width - 100.0).abs() < 1.0,
        "div width should be expanded to 100px by min-width, got {:.1}",
        div.dimensions.content.width
    );
}

/// [§ 10.4](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
///
/// min-width has no effect when the element is already wider.
#[test]
fn test_min_width_no_effect_when_wider() {
    let root = layout_html(
        "<html><head><style>\
         .box { width: 200px; min-width: 100px; }\
         </style></head>\
         <body><div class='box'>Content</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // width: 200px > min-width: 100px, so width stays at 200px.
    assert!(
        (div.dimensions.content.width - 200.0).abs() < 1.0,
        "div width should remain 200px (min-width has no effect), got {:.1}",
        div.dimensions.content.width
    );
}

/// [§ 10.4](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
///
/// "If the tentative used width is greater than 'max-width', the rules above
/// are applied again, but this time using the computed value of 'max-width'
/// as the computed value for 'width'."
///
/// "If the resulting width is smaller than 'min-width', the rules above are
/// applied again, but this time using the value of 'min-width' as the
/// computed value for 'width'."
///
/// min-width wins over max-width when min > max.
#[test]
fn test_min_wins_over_max() {
    let root = layout_html(
        "<html><head><style>\
         .box { min-width: 200px; max-width: 150px; }\
         </style></head>\
         <body><div class='box'>Content</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // min-width: 200px > max-width: 150px. Per spec, min-width wins because
    // it is applied after max-width.
    assert!(
        (div.dimensions.content.width - 200.0).abs() < 1.0,
        "min-width (200) should win over max-width (150), got {:.1}",
        div.dimensions.content.width
    );
}

/// [§ 10.7](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
///
/// max-height should clamp an explicit height.
#[test]
fn test_max_height_clamps() {
    let root = layout_html(
        "<html><head><style>\
         .box { height: 500px; max-height: 100px; }\
         </style></head>\
         <body><div class='box'>Content</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // height: 500px > max-height: 100px, so height is clamped to 100px.
    assert!(
        (div.dimensions.content.height - 100.0).abs() < 1.0,
        "div height should be clamped to 100px by max-height, got {:.1}",
        div.dimensions.content.height
    );
}

/// [§ 10.7](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
///
/// min-height should expand an auto-height box.
#[test]
fn test_min_height_expands() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .box { min-height: 200px; }\
         </style></head>\
         <body><div class='box'>Short</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // Auto height from text content would be small (~16px or so). min-height
    // should expand it to at least 200px.
    assert!(
        div.dimensions.content.height >= 200.0 - 1.0,
        "div height should be at least 200px from min-height, got {:.1}",
        div.dimensions.content.height
    );
}

// ---------------------------------------------------------------------------
// Containing block ancestor walk tests
//
// [§ 10.1 Definition of "containing block"](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
//
// "If the element has 'position: absolute', the containing block is
// established by the nearest ancestor with a 'position' of 'absolute',
// 'relative', 'fixed', or 'sticky'."
// ---------------------------------------------------------------------------

/// [§ 10.1](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
///
/// An absolutely positioned child of a non-positioned parent should use the
/// nearest positioned ancestor (grandparent) as its containing block.
#[test]
fn test_absolute_uses_positioned_grandparent() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .grandparent { position: relative; width: 400px; height: 300px; padding: 10px; }\
         .parent { width: 200px; height: 100px; }\
         .abs { position: absolute; top: 0; left: 0; width: 50px; height: 50px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div class='grandparent'>\
           <div class='parent'>\
             <div class='abs'>Abs</div>\
           </div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grandparent = &body.children[0];
    let parent = &grandparent.children[0];

    // The abs child is a child of .parent in the DOM, but its containing
    // block should be .grandparent (the nearest positioned ancestor).
    let abs_child = &parent.children[0];

    // With top:0, left:0 relative to grandparent's padding box:
    let gp_padding_x = grandparent.dimensions.content.x - grandparent.dimensions.padding.left;
    let gp_padding_y = grandparent.dimensions.content.y - grandparent.dimensions.padding.top;

    assert!(
        (abs_child.dimensions.content.x - gp_padding_x).abs() < 1.0,
        "abs child x should be at grandparent's padding box left ({gp_padding_x:.1}), got {:.1}",
        abs_child.dimensions.content.x
    );
    assert!(
        (abs_child.dimensions.content.y - gp_padding_y).abs() < 1.0,
        "abs child y should be at grandparent's padding box top ({gp_padding_y:.1}), got {:.1}",
        abs_child.dimensions.content.y
    );
}

/// [§ 10.1](https://www.w3.org/TR/CSS2/visudet.html#containing-block-details)
///
/// When no positioned ancestor exists, the absolutely positioned element
/// should use the initial containing block (viewport).
#[test]
fn test_absolute_no_positioned_ancestor_uses_viewport() {
    let root = layout_html(
        "<html><head><style>\
         div { margin: 0; padding: 0; }\
         .abs { position: absolute; top: 0; left: 0; width: 80px; height: 40px; }\
         </style></head>\
         <body style='margin: 0; padding: 0;'>\
         <div><div class='abs'>Abs</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let wrapper = &body.children[0];
    let abs_child = &wrapper.children[0];

    // No positioned ancestor exists (body and wrapper are static), so the
    // containing block is the viewport (origin 0,0).
    assert!(
        abs_child.dimensions.content.x.abs() < 1.0,
        "abs child x should be at viewport left (0), got {:.1}",
        abs_child.dimensions.content.x
    );
    assert!(
        abs_child.dimensions.content.y.abs() < 1.0,
        "abs child y should be at viewport top (0), got {:.1}",
        abs_child.dimensions.content.y
    );
}

// ---------------------------------------------------------------------------
// box-sizing: border-box tests
//
// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
//
// "The box-sizing property defines whether the width and height (and
// respective min/max properties) on an element include padding and
// borders or not."
// ---------------------------------------------------------------------------

/// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
///
/// With `box-sizing: border-box`, `width: 200px` and `padding: 20px` means
/// the content width is 200 - 20 - 20 = 160px.
#[test]
fn test_border_box_width_includes_padding() {
    let root = layout_html(
        "<html><head><style>\
         div { width: 200px; padding: 20px; box-sizing: border-box; }\
         body { margin: 0; }\
         </style></head>\
         <body><div>Hello</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    assert!(
        (div.dimensions.content.width - 160.0).abs() < 0.1,
        "border-box width 200 with padding 20 should give content width 160, got {:.1}",
        div.dimensions.content.width
    );
    assert!(
        (div.dimensions.padding.left - 20.0).abs() < 0.1,
        "padding-left should be 20, got {:.1}",
        div.dimensions.padding.left
    );
    assert!(
        (div.dimensions.padding.right - 20.0).abs() < 0.1,
        "padding-right should be 20, got {:.1}",
        div.dimensions.padding.right
    );
}

/// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
///
/// With `box-sizing: border-box`, `width: 200px` and `border: 5px solid`
/// means content width is 200 - 5 - 5 = 190px.
#[test]
fn test_border_box_width_includes_border() {
    let root = layout_html(
        "<html><head><style>\
         div { width: 200px; border: 5px solid black; box-sizing: border-box; }\
         body { margin: 0; }\
         </style></head>\
         <body><div>Hello</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    assert!(
        (div.dimensions.content.width - 190.0).abs() < 0.1,
        "border-box width 200 with border 5 should give content width 190, got {:.1}",
        div.dimensions.content.width
    );
}

/// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
///
/// With `box-sizing: border-box`, `margin: auto` centering should center
/// based on the border-box (200px) total, not the content width.
#[test]
fn test_border_box_auto_margins_center() {
    let root = layout_html(
        "<html><head><style>\
         div { width: 200px; padding: 20px; margin: 0 auto; box-sizing: border-box; }\
         body { margin: 0; }\
         </style></head>\
         <body><div>Hello</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    // Viewport is 800px. border-box is 200px. Remaining = 600px.
    // Auto margins should each be 300px.
    assert!(
        (div.dimensions.margin.left - 300.0).abs() < 0.1,
        "margin-left should be 300, got {:.1}",
        div.dimensions.margin.left
    );
    assert!(
        (div.dimensions.margin.right - 300.0).abs() < 0.1,
        "margin-right should be 300, got {:.1}",
        div.dimensions.margin.right
    );
}

/// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
///
/// With `box-sizing: border-box`, `height: 100px` and `padding: 10px`
/// means content height is 100 - 10 - 10 = 80px.
#[test]
fn test_border_box_height() {
    let root = layout_html(
        "<html><head><style>\
         div { height: 100px; padding: 10px; box-sizing: border-box; }\
         body { margin: 0; }\
         </style></head>\
         <body><div>Hello</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    assert!(
        (div.dimensions.content.height - 80.0).abs() < 0.1,
        "border-box height 100 with padding 10 should give content height 80, got {:.1}",
        div.dimensions.content.height
    );
}

/// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
///
/// Default behavior (content-box): `width: 200px` and `padding: 20px`
/// means content width is 200px and the total box is 240px.
#[test]
fn test_content_box_default() {
    let root = layout_html(
        "<html><head><style>\
         div { width: 200px; padding: 20px; }\
         body { margin: 0; }\
         </style></head>\
         <body><div>Hello</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    assert!(
        (div.dimensions.content.width - 200.0).abs() < 0.1,
        "content-box width should be 200, got {:.1}",
        div.dimensions.content.width
    );
}

/// [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
///
/// With `box-sizing: border-box`, `max-width: 200px` and `padding: 20px`
/// means the border-box is capped at 200px (content = 160px).
#[test]
fn test_border_box_max_width() {
    let root = layout_html(
        "<html><head><style>\
         div { width: 400px; max-width: 200px; padding: 20px; box-sizing: border-box; }\
         body { margin: 0; }\
         </style></head>\
         <body><div>Hello</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    assert!(
        (div.dimensions.content.width - 160.0).abs() < 0.1,
        "border-box max-width 200 with padding 20 should give content width 160, got {:.1}",
        div.dimensions.content.width
    );
}

// ---------------------------------------------------------------------------
// Float layout tests
//
// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
//
// "A float is a box that is shifted to the left or right on the current line.
// The most interesting characteristic of a float is that content may flow along
// its side (or be prohibited from doing so by the 'clear' property)."
// ---------------------------------------------------------------------------

/// Helper: find the first child box with float_side set.
fn find_float_child(parent: &LayoutBox) -> &LayoutBox {
    parent
        .children
        .iter()
        .find(|c| c.float_side.is_some())
        .expect("expected a floated child")
}

/// Helper: find all children with float_side set.
fn find_float_children(parent: &LayoutBox) -> Vec<&LayoutBox> {
    parent
        .children
        .iter()
        .filter(|c| c.float_side.is_some())
        .collect()
}

/// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
///
/// A float:left element with explicit width should be positioned at the left
/// edge of the containing block's content area.
#[test]
fn test_float_left_basic() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .floated { float: left; width: 100px; height: 50px; }</style><div class='floated'></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let floated = find_float_child(body);

    assert!(
        (floated.dimensions.content.x - 0.0).abs() < 0.1,
        "float:left should be at x=0, got {:.1}",
        floated.dimensions.content.x
    );
    assert!(
        (floated.dimensions.content.y - 0.0).abs() < 0.1,
        "float:left should be at y=0, got {:.1}",
        floated.dimensions.content.y
    );
    assert!(
        (floated.dimensions.content.width - 100.0).abs() < 0.1,
        "floated width should be 100, got {:.1}",
        floated.dimensions.content.width
    );
    assert!(
        (floated.dimensions.content.height - 50.0).abs() < 0.1,
        "floated height should be 50, got {:.1}",
        floated.dimensions.content.height
    );
}

/// [§ 9.5.1 Rule 9](https://www.w3.org/TR/CSS2/visuren.html#float-position)
///
/// "A right-floating box as far to the right as possible."
#[test]
fn test_float_right_basic() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .floated { float: right; width: 100px; height: 50px; }</style><div class='floated'></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let floated = find_float_child(body);

    // Viewport is 800px wide, float should be at x = 800 - 100 = 700.
    assert!(
        (floated.dimensions.content.x - 700.0).abs() < 0.1,
        "float:right should be at x=700 in 800px viewport, got {:.1}",
        floated.dimensions.content.x
    );
}

/// [§ 9.5 Floats](https://www.w3.org/TR/CSS2/visuren.html#floats)
///
/// "Since a float is not in the flow, non-positioned block boxes created
/// before and after the float box flow vertically as if the float did not
/// exist."
///
/// A floated box should NOT advance the parent's current_y for subsequent
/// in-flow siblings.
#[test]
fn test_float_no_advance_y() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .floated { float: left; width: 100px; height: 50px; } .block { margin: 0; }</style><div class='floated'></div><div class='block'>After</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    // Find the in-flow block child (not the float).
    let block = body
        .children
        .iter()
        .find(|c| c.float_side.is_none() && c.display.outer == OuterDisplayType::Block)
        .expect("expected an in-flow block child");

    assert!(
        (block.dimensions.content.y - 0.0).abs() < 0.1,
        "in-flow block after float should be at y=0, got {:.1}",
        block.dimensions.content.y
    );
}

/// [§ 9.5.2 Controlling flow next to floats: the 'clear' property](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
///
/// "For clear: left → below bottom edge of all left-floating boxes."
#[test]
fn test_clear_left() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .floated { float: left; width: 100px; height: 80px; } .cleared { clear: left; margin: 0; }</style><div class='floated'></div><div class='cleared'>Cleared</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    // The cleared div is the second block-level child (first in-flow block).
    let cleared = body
        .children
        .iter()
        .find(|c| c.clear_side.is_some())
        .expect("expected a cleared child");

    // The cleared div should be pushed below the float's bottom edge (80px).
    assert!(
        cleared.dimensions.content.y >= 79.9,
        "clear:left should push below float bottom (80), got y={:.1}",
        cleared.dimensions.content.y
    );
}

/// [§ 9.5.2](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
///
/// "For clear: both → below bottom edge of all floating boxes."
#[test]
fn test_clear_both() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .fl { float: left; width: 100px; height: 60px; } .fr { float: right; width: 100px; height: 80px; } .cleared { clear: both; margin: 0; }</style><div class='fl'></div><div class='fr'></div><div class='cleared'>Cleared</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let cleared = body
        .children
        .iter()
        .find(|c| c.clear_side.is_some())
        .expect("expected a cleared child");

    // clear:both should push below the tallest float (right at 80px).
    assert!(
        cleared.dimensions.content.y >= 79.9,
        "clear:both should push below tallest float (80), got y={:.1}",
        cleared.dimensions.content.y
    );
}

/// [§ 9.7 Relationships between 'display', 'position', and 'float'](https://www.w3.org/TR/CSS2/visuren.html#dis-pos-flo)
///
/// "Otherwise, if 'float' has a value other than 'none', the box is floated
/// and 'display' is set according to the table [inline → block]."
#[test]
fn test_float_display_blockification() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } span.fl { float: left; width: 80px; height: 40px; }</style><span class='fl'>Float</span></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let floated_span = find_float_child(body);

    // The span should have been blockified: width should be 80 as specified.
    assert!(
        (floated_span.dimensions.content.width - 80.0).abs() < 0.1,
        "floated span should be blockified with width=80, got {:.1}",
        floated_span.dimensions.content.width
    );
    assert!(
        (floated_span.dimensions.content.height - 40.0).abs() < 0.1,
        "floated span should be blockified with height=40, got {:.1}",
        floated_span.dimensions.content.height
    );
}

/// [§ 10.6.7 'Auto' heights for block formatting context roots](https://www.w3.org/TR/CSS2/visudet.html#root-height)
///
/// "If the element has any floating descendants whose bottom margin edge
/// is below the element's bottom content edge, then the height is
/// increased to include those edges."
#[test]
fn test_float_height_extension() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .container { background-color: #ccc; } .floated { float: left; width: 100px; height: 120px; }</style><div class='container'><div class='floated'></div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    // The container's auto height should extend to include the float (120px).
    assert!(
        container.dimensions.content.height >= 119.9,
        "container auto height should extend to include float (120), got {:.1}",
        container.dimensions.content.height
    );
}

/// [§ 9.5.1 Rules 2, 3, 7](https://www.w3.org/TR/CSS2/visuren.html#float-position)
///
/// Multiple left floats should stack horizontally (not overlap).
#[test]
fn test_multiple_floats_stack() {
    let root = layout_html(
        "<html><body><style>body { margin: 0; } .fl { float: left; width: 100px; height: 50px; }</style><div class='fl'>A</div><div class='fl'>B</div><div class='fl'>C</div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let floats = find_float_children(body);

    assert!(
        floats.len() >= 3,
        "expected at least 3 float children, got {}",
        floats.len()
    );

    assert!(
        (floats[0].dimensions.content.x - 0.0).abs() < 0.1,
        "first float at x=0, got {:.1}",
        floats[0].dimensions.content.x
    );
    assert!(
        (floats[1].dimensions.content.x - 100.0).abs() < 0.1,
        "second float at x=100, got {:.1}",
        floats[1].dimensions.content.x
    );
    assert!(
        (floats[2].dimensions.content.x - 200.0).abs() < 0.1,
        "third float at x=200, got {:.1}",
        floats[2].dimensions.content.x
    );
}

// ---------------------------------------------------------------------------
// Inline-block tests
//
// [§ 10.3.9 'Inline-block', non-replaced elements in normal flow](https://www.w3.org/TR/CSS2/visudet.html#inlineblock-width)
//
// "This value causes an element to generate an inline-level block
// container. The inside of an inline-block is formatted as a block box,
// and the element itself is formatted as an atomic inline-level box."
// ---------------------------------------------------------------------------

/// [§ 10.3.9](https://www.w3.org/TR/CSS2/visudet.html#inlineblock-width)
///
/// An inline-block with explicit width and height should use those dimensions.
#[test]
fn test_inline_block_explicit_size() {
    let root = layout_html(
        "<html><body><style>\
         body { margin: 0; }\
         .ib { display: inline-block; width: 100px; height: 50px; }\
         </style>\
         <div><span class='ib'></span></div>\
         </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    // The inline-block child should be in the container's children.
    let ib = &container.children[0];
    assert!(
        (ib.dimensions.content.width - 100.0).abs() < 0.1,
        "inline-block width should be 100, got {:.1}",
        ib.dimensions.content.width
    );
    assert!(
        (ib.dimensions.content.height - 50.0).abs() < 0.1,
        "inline-block height should be 50, got {:.1}",
        ib.dimensions.content.height
    );
}

/// [§ 10.3.9](https://www.w3.org/TR/CSS2/visudet.html#inlineblock-width)
///
/// "If 'width' is 'auto', the used value is the shrink-to-fit width."
///
/// An inline-block with auto width containing text should shrink to fit
/// the text content.
#[test]
fn test_inline_block_shrink_to_fit() {
    let root = layout_html(
        "<html><body><style>\
         body { margin: 0; }\
         .ib { display: inline-block; }\
         </style>\
         <div><span class='ib'>Hello</span></div>\
         </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];
    let ib = &container.children[0];

    // The inline-block should shrink to the text width.
    // ApproximateFontMetrics: 5 chars × 0.6 × 16.0 = 48.0
    let expected_width = 5.0 * 0.6 * 16.0;
    assert!(
        (ib.dimensions.content.width - expected_width).abs() < 1.0,
        "inline-block should shrink-to-fit (expected ~{expected_width:.0}), got {:.1}",
        ib.dimensions.content.width
    );
}

/// [§ 9.2.4 Atomic inline-level boxes](https://www.w3.org/TR/css-display-3/#atomic-inline)
///
/// Two inline-blocks side by side should be on the same line, positioned
/// horizontally.
#[test]
fn test_inline_block_multiple_on_one_line() {
    let root = layout_html(
        "<html><body><style>\
         body { margin: 0; }\
         .ib1 { display: inline-block; width: 100px; height: 50px; }\
         .ib2 { display: inline-block; width: 150px; height: 50px; }\
         </style>\
         <div>\
           <span class='ib1'></span>\
           <span class='ib2'></span>\
         </div>\
         </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    // Find the inline-block children. The container's children may include
    // anonymous inline text nodes for whitespace, so search for Principal
    // boxes with inline-block display.
    let inline_blocks: Vec<&LayoutBox> = container
        .children
        .iter()
        .filter(|c| {
            c.display.outer == OuterDisplayType::Inline
                && c.display.inner == InnerDisplayType::FlowRoot
        })
        .collect();
    assert!(
        inline_blocks.len() >= 2,
        "expected at least 2 inline-block children, got {} (total children: {})",
        inline_blocks.len(),
        container.children.len()
    );
    let ib1 = inline_blocks[0];
    let ib2 = inline_blocks[1];

    // ib1 should start at x=0 (or near it, within the content box).
    assert!(
        ib1.dimensions.content.x < 10.0,
        "first inline-block should be near the left edge, got x={:.1}",
        ib1.dimensions.content.x
    );

    // ib2 should be to the right of ib1. There may be a space between
    // them from the whitespace text node.
    assert!(
        ib2.dimensions.content.x > ib1.dimensions.content.x + 90.0,
        "second inline-block should be to the right of the first, \
         ib1.x={:.1}, ib2.x={:.1}",
        ib1.dimensions.content.x,
        ib2.dimensions.content.x
    );
}

/// [§ 9.4.2](https://www.w3.org/TR/CSS2/visuren.html#inline-formatting)
///
/// An inline-block that doesn't fit on the current line should wrap to
/// the next line.
#[test]
fn test_inline_block_line_wrapping() {
    // Viewport is 800px wide. Two 500px inline-blocks won't fit on one line.
    let root = layout_html(
        "<html><body><style>\
         body { margin: 0; }\
         .ib { display: inline-block; width: 500px; height: 40px; }\
         </style>\
         <div>\
           <span class='ib'>A</span>\
           <span class='ib'>B</span>\
         </div>\
         </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    let ib1 = &container.children[0];
    let ib2 = &container.children[1];

    // ib2 should be on a different line (higher y value) than ib1.
    assert!(
        ib2.dimensions.content.y > ib1.dimensions.content.y + 20.0,
        "second inline-block should wrap to next line, \
         ib1.y={:.1}, ib2.y={:.1}",
        ib1.dimensions.content.y,
        ib2.dimensions.content.y
    );
}

/// [§ 16.2 Alignment: the 'text-align' property](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
///
/// "Inline-level content is centered within the line box."
///
/// An inline-block inside a `text-align: center` container should be
/// horizontally centered.
#[test]
fn test_inline_block_text_align_center() {
    let root = layout_html(
        "<html><body><style>\
         body { margin: 0; }\
         .container { text-align: center; }\
         .ib { display: inline-block; width: 200px; height: 50px; }\
         </style>\
         <div class='container'>\
           <span class='ib'></span>\
         </div>\
         </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let container = &body.children[0];

    // Find the inline-block child (may be mixed with anonymous text nodes).
    let ib = container
        .children
        .iter()
        .find(|c| {
            c.display.outer == OuterDisplayType::Inline
                && c.display.inner == InnerDisplayType::FlowRoot
        })
        .expect("should find an inline-block child");

    // Container is 800px wide. The 200px inline-block should be centered:
    // offset = (800 - 200) / 2 = 300.
    let expected_x = (800.0 - 200.0) / 2.0;
    assert!(
        (ib.dimensions.content.x - expected_x).abs() < 5.0,
        "inline-block should be centered (expected x≈{expected_x:.0}), got x={:.1}",
        ib.dimensions.content.x
    );
}

// ---------------------------------------------------------------------------
// List item / marker tests
//
// [§ 2.5 List Items](https://www.w3.org/TR/css-display-3/#list-items)
// [§ 3 Markers](https://www.w3.org/TR/css-lists-3/#markers)
// ---------------------------------------------------------------------------

#[test]
fn test_list_item_display() {
    // [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists)
    // "li { display: list-item; }"
    assert_eq!(
        default_display_for_element("li"),
        Some(DisplayValue::list_item()),
        "default display for <li> should be list-item"
    );
}

#[test]
fn test_ul_marker_disc() {
    // [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists)
    //
    // <ul> sets list-style-type: disc (inherited by <li>).
    // The list item should generate a bullet marker "\u{2022} ".
    let root = layout_html("<ul><li>Item</li></ul>");

    // Document > html > body > ul > li
    let body = box_at_depth(&root, 2);
    let ul = &body.children[0];
    assert!(!ul.children.is_empty(), "ul should have children");
    let li = &ul.children[0];
    assert_eq!(
        li.display.outer,
        OuterDisplayType::ListItem,
        "li should have display: list-item"
    );
    assert_eq!(
        li.marker_text.as_deref(),
        Some("\u{2022} "),
        "ul > li should have disc marker"
    );
}

#[test]
fn test_ol_marker_decimal() {
    // [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists)
    //
    // <ol> sets list-style-type: decimal (inherited by <li>).
    // The list items should generate "1. ", "2. ", etc.
    let root = layout_html("<ol><li>First</li><li>Second</li></ol>");

    // Document > html > body > ol > [li, li]
    let body = box_at_depth(&root, 2);
    let ol = &body.children[0];
    assert!(ol.children.len() >= 2, "ol should have at least 2 children");
    let li1 = &ol.children[0];
    let li2 = &ol.children[1];
    assert_eq!(
        li1.marker_text.as_deref(),
        Some("1. "),
        "first li should have marker '1. '"
    );
    assert_eq!(
        li2.marker_text.as_deref(),
        Some("2. "),
        "second li should have marker '2. '"
    );
}

#[test]
fn test_list_style_type_none() {
    // [§ 3.1 'list-style-type'](https://www.w3.org/TR/css-lists-3/#list-style-type)
    //
    // list-style-type: none suppresses marker generation.
    let root =
        layout_html("<style>ul { list-style-type: none; }</style><ul><li>No bullet</li></ul>");

    let body = box_at_depth(&root, 2);
    let ul = &body.children[0];
    let li = &ul.children[0];
    assert!(
        li.marker_text.is_none(),
        "list-style-type: none should suppress marker, got {:?}",
        li.marker_text
    );
}

#[test]
fn test_list_style_type_circle() {
    // [§ 3.1 'list-style-type'](https://www.w3.org/TR/css-lists-3/#list-style-type)
    //
    // Setting list-style-type: circle should produce a white circle marker.
    let root = layout_html("<style>ul { list-style-type: circle; }</style><ul><li>Item</li></ul>");

    let body = box_at_depth(&root, 2);
    let ul = &body.children[0];
    let li = &ul.children[0];
    assert_eq!(
        li.marker_text.as_deref(),
        Some("\u{25CB} "),
        "list-style-type: circle should produce white circle marker"
    );
}

#[test]
fn test_ol_start_attribute() {
    // [§ 4.4.5 The ol element](https://html.spec.whatwg.org/multipage/grouping-content.html#the-ol-element)
    //
    // The `start` attribute on <ol> sets the starting ordinal.
    let root = layout_html("<ol start=\"5\"><li>A</li><li>B</li></ol>");

    let body = box_at_depth(&root, 2);
    let ol = &body.children[0];
    let li1 = &ol.children[0];
    let li2 = &ol.children[1];
    assert_eq!(li1.marker_text.as_deref(), Some("5. "));
    assert_eq!(li2.marker_text.as_deref(), Some("6. "));
}

// ---------------------------------------------------------------------------
// Overflow clipping tests
//
// [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
//
// "This property specifies whether content of a block container element
// is clipped when it overflows the element's box."
// ---------------------------------------------------------------------------

/// Helper: parse HTML, build layout + styles, paint, and return the display list.
fn paint_html(html: &str) -> koala_css::DisplayList {
    use koala_css::cascade::compute_styles;
    use koala_css::{CSSParser, CSSTokenizer, Painter, Stylesheet};
    use std::collections::HashMap;

    let mut tokenizer = koala_html::HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let parser = koala_html::HTMLParser::new(tokenizer.into_tokens());
    let (dom, _) = parser.run_with_issues();

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
        width: 800.0,
        height: 600.0,
    };
    layout_tree.layout(viewport, viewport, &ApproximateFontMetrics, viewport);

    let painter = Painter::new(&styles);
    painter.paint(&layout_tree)
}

#[test]
fn test_overflow_hidden_emits_push_pop_clip() {
    // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    //
    // An element with `overflow: hidden` should emit PushClip before its
    // content and PopClip after, with the clip rect matching its padding box.
    use koala_css::DisplayCommand;

    let display_list = paint_html(
        "<style>div { overflow: hidden; width: 100px; height: 50px; }</style>\
         <div>Hello world</div>",
    );

    let commands = display_list.commands();
    let push_clips: Vec<_> = commands
        .iter()
        .filter(|c| matches!(c, DisplayCommand::PushClip { .. }))
        .collect();
    let pop_clips: Vec<_> = commands
        .iter()
        .filter(|c| matches!(c, DisplayCommand::PopClip))
        .collect();

    assert!(
        !push_clips.is_empty(),
        "overflow: hidden should produce at least one PushClip"
    );
    assert_eq!(
        push_clips.len(),
        pop_clips.len(),
        "PushClip and PopClip should be balanced"
    );

    // Verify the PushClip dimensions match the div's width/height
    if let DisplayCommand::PushClip { width, height, .. } = push_clips[0] {
        assert!(
            (*width - 100.0).abs() < 1.0,
            "clip width should be ~100px, got {width}"
        );
        assert!(
            (*height - 50.0).abs() < 1.0,
            "clip height should be ~50px, got {height}"
        );
    }
}

#[test]
fn test_default_overflow_visible_no_clip() {
    // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    //
    // "Initial: visible"
    //
    // Without overflow: hidden, no PushClip/PopClip should appear.
    use koala_css::DisplayCommand;

    let display_list = paint_html(
        "<style>div { width: 100px; height: 50px; }</style>\
         <div>Hello world</div>",
    );

    let commands = display_list.commands();
    let has_clip = commands
        .iter()
        .any(|c| matches!(c, DisplayCommand::PushClip { .. } | DisplayCommand::PopClip));

    assert!(
        !has_clip,
        "default overflow (visible) should not produce any clip commands"
    );
}

#[test]
fn test_nested_overflow_hidden() {
    // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
    //
    // Two nested containers both with overflow: hidden should produce
    // two PushClip/PopClip pairs in correct nesting order.
    use koala_css::DisplayCommand;

    let display_list = paint_html(
        "<style>.clip { overflow: hidden; width: 200px; height: 100px; }</style>\
         <div class=\"clip\"><div class=\"clip\">inner</div></div>",
    );

    let commands = display_list.commands();
    let push_count = commands
        .iter()
        .filter(|c| matches!(c, DisplayCommand::PushClip { .. }))
        .count();
    let pop_count = commands
        .iter()
        .filter(|c| matches!(c, DisplayCommand::PopClip))
        .count();

    assert_eq!(
        push_count, 2,
        "nested overflow: hidden should produce 2 PushClip"
    );
    assert_eq!(
        pop_count, 2,
        "nested overflow: hidden should produce 2 PopClip"
    );

    // Verify nesting order: PushClip, PushClip, ..., PopClip, PopClip
    let mut depth = 0i32;
    let mut max_depth = 0i32;
    for cmd in commands {
        match cmd {
            DisplayCommand::PushClip { .. } => {
                depth += 1;
                max_depth = max_depth.max(depth);
            }
            DisplayCommand::PopClip => {
                depth -= 1;
                assert!(depth >= 0, "PopClip without matching PushClip");
            }
            _ => {}
        }
    }
    assert_eq!(depth, 0, "clip stack should be balanced at end");
    assert_eq!(
        max_depth, 2,
        "max clip depth should be 2 for nested overflow"
    );
}

/// [§ 8.3 'align-items: center'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
///
/// A flex container with height 200px and align-items: center. A child
/// with height 50px should be vertically centered at y_offset = 75px
/// from the container's content top.
#[test]
fn test_flex_align_items_center() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; height: 200px; align-items: center; }\
         .item { width: 100px; height: 50px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex_container = &body.children[0];
    let item = &flex_container.children[0];

    // Container content starts at container's content.y
    let container_y = flex_container.dimensions.content.y;
    let item_y = item.dimensions.content.y;

    // Item should be centered: offset = (200 - 50) / 2 = 75
    let expected_offset = 75.0;
    assert!(
        (item_y - container_y - expected_offset).abs() < 1.0,
        "align-items: center should center child. Expected offset ~{expected_offset}, got {:.1}",
        item_y - container_y
    );
}

/// [§ 8.3 'align-items: flex-end'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
///
/// A flex container with height 200px and align-items: flex-end. A child
/// with height 50px should be aligned to the bottom of the container.
#[test]
fn test_flex_align_items_flex_end() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; height: 200px; align-items: flex-end; }\
         .item { width: 100px; height: 50px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex_container = &body.children[0];
    let item = &flex_container.children[0];

    let container_y = flex_container.dimensions.content.y;
    let item_y = item.dimensions.content.y;

    // Item should be at bottom: offset = 200 - 50 = 150
    let expected_offset = 150.0;
    assert!(
        (item_y - container_y - expected_offset).abs() < 1.0,
        "align-items: flex-end should align child to bottom. Expected offset ~{expected_offset}, got {:.1}",
        item_y - container_y
    );
}

/// [§ 8.3 'align-items: stretch'](https://www.w3.org/TR/css-flexbox-1/#align-items-property)
///
/// Default align-items (stretch). A child with no explicit height in a
/// container with height 200px should be stretched to fill 200px.
#[test]
fn test_flex_align_items_stretch() {
    let root = layout_html(
        "<html><head><style>\
         .flex { display: flex; width: 400px; height: 200px; }\
         .item { width: 100px; }\
         </style></head>\
         <body><div class='flex'><div class='item'>A</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let flex_container = &body.children[0];
    let item = &flex_container.children[0];

    assert!(
        (item.dimensions.content.height - 200.0).abs() < 1.0,
        "align-items: stretch (default) should stretch child to container height. Got {:.1}",
        item.dimensions.content.height
    );
}

// ---------------------------------------------------------------------------
// Form element rendering tests
//
// [§ 15.5.12–15.5.15 Form controls](https://html.spec.whatwg.org/multipage/rendering.html)
// ---------------------------------------------------------------------------

/// Verify <input> gets inline-block display and replaced-element intrinsic
/// dimensions (~173px wide, ~20px tall for text inputs).
#[test]
fn test_input_inline_block_display() {
    let root = layout_html("<body><input type='text'></body>");

    // The input should be inside an anonymous block wrapper (since body is
    // block and input is inline-block, it gets wrapped in an anonymous block
    // that establishes an inline formatting context).
    // Navigate to the input's layout box by finding the replaced element.
    fn find_replaced(b: &LayoutBox) -> Option<&LayoutBox> {
        if b.is_replaced {
            return Some(b);
        }
        for child in &b.children {
            if let Some(found) = find_replaced(child) {
                return Some(found);
            }
        }
        None
    }

    let input = find_replaced(&root).expect("should find a replaced <input> element");

    assert_eq!(
        input.display.outer,
        OuterDisplayType::Inline,
        "input outer display should be Inline (inline-block)"
    );
    assert_eq!(
        input.display.inner,
        InnerDisplayType::FlowRoot,
        "input inner display should be FlowRoot (inline-block)"
    );

    // Content width should be the intrinsic 173px (text input default).
    // Account for border (2px inset = 2px each side) and padding (1px 2px).
    let content_w = input.dimensions.content.width;
    assert!(
        (content_w - 173.0).abs() < 1.0,
        "input content width should be ~173px, got {content_w:.1}"
    );
}

/// Verify <button> gets inline-block display with normal (non-replaced) layout.
#[test]
fn test_button_inline_block_display() {
    let root = layout_html("<body><button>OK</button></body>");

    // Find the button box by display type.
    fn find_inline_block(b: &LayoutBox) -> Option<&LayoutBox> {
        if b.display.outer == OuterDisplayType::Inline
            && b.display.inner == InnerDisplayType::FlowRoot
            && !b.is_replaced
        {
            return Some(b);
        }
        for child in &b.children {
            if let Some(found) = find_inline_block(child) {
                return Some(found);
            }
        }
        None
    }

    let button = find_inline_block(&root).expect("should find an inline-block <button> element");

    // Button should NOT be replaced — it has child content.
    assert!(
        !button.is_replaced,
        "button should not be a replaced element"
    );

    // Button should have non-zero width from shrink-to-fit on its text content.
    assert!(
        button.dimensions.content.width > 0.0,
        "button should have positive width from text content, got {:.1}",
        button.dimensions.content.width
    );
}

/// Verify <input type="checkbox"> gets small intrinsic dimensions (13x13).
#[test]
fn test_input_checkbox_intrinsic_size() {
    let root = layout_html("<body><input type='checkbox'></body>");

    fn find_replaced(b: &LayoutBox) -> Option<&LayoutBox> {
        if b.is_replaced {
            return Some(b);
        }
        for child in &b.children {
            if let Some(found) = find_replaced(child) {
                return Some(found);
            }
        }
        None
    }

    let checkbox = find_replaced(&root).expect("should find a replaced <input type=checkbox>");

    let w = checkbox.dimensions.content.width;
    let h = checkbox.dimensions.content.height;
    assert!(
        (w - 13.0).abs() < 1.0,
        "checkbox width should be ~13px, got {w:.1}"
    );
    assert!(
        (h - 13.0).abs() < 1.0,
        "checkbox height should be ~13px, got {h:.1}"
    );
}

// ===========================================================================
// CSS Grid Layout tests
//
// [§ 12 Grid Sizing](https://www.w3.org/TR/css-grid-1/#layout-algorithm)
// ===========================================================================

/// `display: grid` produces a grid container with the correct display value.
#[test]
fn test_grid_container_recognized() {
    let root = layout_html(
        "<html><head><style>\
         .grid { display: grid; width: 300px; }\
         </style></head>\
         <body><div class='grid'><div>A</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    assert_eq!(
        grid.display.inner,
        InnerDisplayType::Grid,
        "should be InnerDisplayType::Grid"
    );
}

/// Grid template columns and gap values are extracted to LayoutBox.
#[test]
fn test_grid_properties_parsed() {
    let root = layout_html(
        "<html><head><style>\
         .grid { display: grid; grid-template-columns: 100px 200px; gap: 10px; width: 310px; }\
         </style></head>\
         <body><div class='grid'><div>A</div><div>B</div></div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];

    assert_eq!(grid.grid_template_columns.sizes.len(), 2);
    assert_eq!(grid.column_gap, 10.0);
    assert_eq!(grid.row_gap, 10.0);
}

/// 4 items in a 2-column fixed grid: 2 rows, correct positions.
#[test]
fn test_grid_2x2_fixed_columns() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 150px 150px; width: 300px; }\
         .item { height: 50px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
            <div class='item'>C</div>\
            <div class='item'>D</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    assert!(
        grid.children.len() >= 4,
        "grid should have 4 children, got {}",
        grid.children.len()
    );

    let a = &grid.children[0];
    let b = &grid.children[1];
    let c = &grid.children[2];
    let d = &grid.children[3];

    // A at (col 0, row 0), B at (col 1, row 0)
    assert!(
        (a.dimensions.content.width - 150.0).abs() < 1.0,
        "A width should be 150px, got {:.1}",
        a.dimensions.content.width
    );
    assert!(
        (b.dimensions.content.x - a.dimensions.content.x - 150.0).abs() < 1.0,
        "B should be 150px right of A: A.x={:.1}, B.x={:.1}",
        a.dimensions.content.x,
        b.dimensions.content.x
    );

    // C at (col 0, row 1), D at (col 1, row 1)
    assert!(
        (c.dimensions.content.y - a.dimensions.content.y - 50.0).abs() < 1.0,
        "C should be one row below A: A.y={:.1}, C.y={:.1}",
        a.dimensions.content.y,
        c.dimensions.content.y
    );
    assert!(
        (d.dimensions.content.x - c.dimensions.content.x - 150.0).abs() < 1.0,
        "D should be 150px right of C"
    );
}

/// fr units distribute available space proportionally.
#[test]
fn test_grid_fr_units() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 1fr 2fr; width: 300px; }\
         .item { height: 40px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    let a = &grid.children[0];
    let b = &grid.children[1];

    // 1fr + 2fr = 3fr total, 300px available
    // A = 100px, B = 200px
    assert!(
        (a.dimensions.content.width - 100.0).abs() < 2.0,
        "A should be ~100px wide, got {:.1}",
        a.dimensions.content.width
    );
    assert!(
        (b.dimensions.content.width - 200.0).abs() < 2.0,
        "B should be ~200px wide, got {:.1}",
        b.dimensions.content.width
    );
}

/// Mixed fixed and fr units.
#[test]
fn test_grid_mixed_fixed_fr() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 100px 1fr; width: 400px; }\
         .item { height: 40px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    let a = &grid.children[0];
    let b = &grid.children[1];

    // A = 100px fixed, B = remaining 300px
    assert!(
        (a.dimensions.content.width - 100.0).abs() < 2.0,
        "A should be ~100px, got {:.1}",
        a.dimensions.content.width
    );
    assert!(
        (b.dimensions.content.width - 300.0).abs() < 2.0,
        "B should be ~300px, got {:.1}",
        b.dimensions.content.width
    );
}

/// gap property creates spacing between grid items.
#[test]
fn test_grid_gap() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 100px 100px; gap: 20px; width: 220px; }\
         .item { height: 40px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
            <div class='item'>C</div>\
            <div class='item'>D</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    let a = &grid.children[0];
    let b = &grid.children[1];
    let c = &grid.children[2];

    // Column gap: B.x = A.x + 100 + 20
    let expected_b_x = a.dimensions.content.x + 100.0 + 20.0;
    assert!(
        (b.dimensions.content.x - expected_b_x).abs() < 2.0,
        "B should have 20px column gap from A: expected x={expected_b_x:.1}, got {:.1}",
        b.dimensions.content.x
    );

    // Row gap: C.y = A.y + 40 + 20
    let expected_c_y = a.dimensions.content.y + 40.0 + 20.0;
    assert!(
        (c.dimensions.content.y - expected_c_y).abs() < 2.0,
        "C should have 20px row gap from A: expected y={expected_c_y:.1}, got {:.1}",
        c.dimensions.content.y
    );
}

/// 3 items in a 2-column grid auto-place into 2 rows.
#[test]
fn test_grid_auto_placement() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 100px 100px; width: 200px; }\
         .item { height: 30px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
            <div class='item'>C</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    let a = &grid.children[0];
    let b = &grid.children[1];
    let c = &grid.children[2];

    // A and B on row 0, C on row 1
    assert!(
        (a.dimensions.content.y - b.dimensions.content.y).abs() < 1.0,
        "A and B should be on same row"
    );
    assert!(
        c.dimensions.content.y > a.dimensions.content.y + 20.0,
        "C should be on a lower row than A: C.y={:.1}, A.y={:.1}",
        c.dimensions.content.y,
        a.dimensions.content.y
    );
    // C should be in column 0
    assert!(
        (c.dimensions.content.x - a.dimensions.content.x).abs() < 1.0,
        "C should be in column 0 like A"
    );
}

/// Explicit placement via grid-column and grid-row.
#[test]
fn test_grid_explicit_placement() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 100px 100px 100px; width: 300px; }\
         .item { height: 40px; }\
         .placed { grid-column: 3; grid-row: 2; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
            <div class='item placed'>C</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];

    // Find item C (the one with explicit placement)
    let c = &grid.children[2];

    // C should be at column 3 (0-based index 2), row 2 (0-based index 1)
    let expected_x = grid.dimensions.content.x + 200.0; // col 2 * 100px
    let expected_y = grid.dimensions.content.y + 40.0; // row 1 * 40px
    assert!(
        (c.dimensions.content.x - expected_x).abs() < 2.0,
        "C should be at column 3: expected x={expected_x:.1}, got {:.1}",
        c.dimensions.content.x
    );
    assert!(
        (c.dimensions.content.y - expected_y).abs() < 2.0,
        "C should be at row 2: expected y={expected_y:.1}, got {:.1}",
        c.dimensions.content.y
    );
}

/// grid-column: 1 / 3 spans two columns.
#[test]
fn test_grid_span() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 100px 100px; width: 200px; }\
         .span { grid-column: 1 / 3; height: 40px; }\
         .item { height: 40px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='span'>A</div>\
            <div class='item'>B</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];
    let a = &grid.children[0];

    // A spans columns 1-3 (0-based 0-2), so its width should be 200px
    assert!(
        (a.dimensions.content.width - 200.0).abs() < 2.0,
        "A should span full width (200px), got {:.1}",
        a.dimensions.content.width
    );
}

/// repeat(3, 100px) expands to 3 columns of 100px.
#[test]
fn test_grid_repeat_function() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: repeat(3, 100px); width: 300px; }\
         .item { height: 30px; }\
         </style></head>\
         <body><div class='grid'>\
            <div class='item'>A</div>\
            <div class='item'>B</div>\
            <div class='item'>C</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];

    // Should have 3 columns
    assert_eq!(
        grid.grid_template_columns.sizes.len(),
        3,
        "repeat(3, 100px) should produce 3 track sizes"
    );

    // All items on the same row
    let a = &grid.children[0];
    let b = &grid.children[1];
    let c = &grid.children[2];

    assert!(
        (a.dimensions.content.y - b.dimensions.content.y).abs() < 1.0,
        "A and B should be on same row"
    );
    assert!(
        (b.dimensions.content.y - c.dimensions.content.y).abs() < 1.0,
        "B and C should be on same row"
    );

    // Each 100px wide
    assert!(
        (a.dimensions.content.width - 100.0).abs() < 2.0,
        "Each item should be 100px wide, got {:.1}",
        a.dimensions.content.width
    );
}

/// Auto rows size to content height.
#[test]
fn test_grid_auto_rows() {
    let root = layout_html(
        "<html><head><style>\
         * { margin: 0; padding: 0; }\
         .grid { display: grid; grid-template-columns: 200px; width: 200px; }\
         </style></head>\
         <body><div class='grid'>\
            <div>Short</div>\
            <div>Also short</div>\
         </div></body></html>",
    );

    let body = box_at_depth(&root, 2);
    let grid = &body.children[0];

    // Container height should be the sum of content heights
    assert!(
        grid.dimensions.content.height > 0.0,
        "grid container height should be positive, got {:.1}",
        grid.dimensions.content.height
    );

    // Second item should be below the first
    if grid.children.len() >= 2 {
        let a = &grid.children[0];
        let b = &grid.children[1];
        assert!(
            b.dimensions.content.y > a.dimensions.content.y,
            "second item should be below first: a.y={:.1}, b.y={:.1}",
            a.dimensions.content.y,
            b.dimensions.content.y
        );
    }
}

// ---------------------------------------------------------------------------
// Table layout tests
//
// [§ 17 Tables](https://www.w3.org/TR/CSS2/tables.html)
//
// Tests for automatic table layout algorithm (§ 17.5.2).
// ---------------------------------------------------------------------------

/// [§ 17.5.2 Automatic table layout](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
///
/// Basic 2x2 table: cells should be positioned in a grid, not stacked
/// vertically like blocks.
///
/// NOTE: The HTML parser implicitly wraps `<tr>` elements in a `<tbody>`
/// per [§ 13.2.6.4.9](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable),
/// so the layout tree is: table > tbody > tr > td.
#[test]
fn test_table_basic_2x2() {
    let root = layout_html(
        "<html><body>
            <table>
                <tr><td>A</td><td>B</td></tr>
                <tr><td>C</td><td>D</td></tr>
            </table>
        </body></html>",
    );

    // Document > html > body > table
    let body = box_at_depth(&root, 2);
    assert!(
        !body.children.is_empty(),
        "body should have at least one child (the table)"
    );
    let table = &body.children[0];
    assert_eq!(
        table.display,
        DisplayValue::table(),
        "table element should have display: table"
    );

    // Table should have positive height (cells rendered).
    assert!(
        table.dimensions.content.height > 0.0,
        "table should have positive height, got {:.1}",
        table.dimensions.content.height
    );

    // Parser wraps <tr> in an implicit <tbody>.
    // table > tbody > [tr, tr]
    let tbody: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tbody"))
        .collect();
    assert_eq!(tbody.len(), 1, "table should have 1 implicit <tbody>");

    let trs: Vec<&LayoutBox> = tbody[0]
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tr"))
        .collect();
    assert_eq!(trs.len(), 2, "tbody should have 2 <tr> children");

    // Row 1 cells: A and B should be on the same row (same y).
    let row1 = trs[0];
    assert!(
        row1.children.len() >= 2,
        "first row should have at least 2 cells"
    );
    let cell_a = &row1.children[0];
    let cell_b = &row1.children[1];
    assert!(
        (cell_a.dimensions.content.y - cell_b.dimensions.content.y).abs() < 1.0,
        "cells A and B should be on the same row: A.y={:.1}, B.y={:.1}",
        cell_a.dimensions.content.y,
        cell_b.dimensions.content.y
    );

    // Cells A and B should be side by side (B.x > A.x).
    assert!(
        cell_b.dimensions.content.x > cell_a.dimensions.content.x,
        "cell B should be to the right of cell A: A.x={:.1}, B.x={:.1}",
        cell_a.dimensions.content.x,
        cell_b.dimensions.content.x
    );

    // Row 2 should be below row 1.
    let row2 = trs[1];
    assert!(
        row2.dimensions.content.y > row1.dimensions.content.y,
        "row 2 should be below row 1: r1.y={:.1}, r2.y={:.1}",
        row1.dimensions.content.y,
        row2.dimensions.content.y
    );
}

/// [§ 17.5.2.2](https://www.w3.org/TR/CSS2/tables.html#auto-table-layout)
///
/// Table with explicit width: columns should fill the table width.
#[test]
fn test_table_explicit_width() {
    let root = layout_html(
        "<html><body>
            <style>table { width: 400px; }</style>
            <table>
                <tr><td>A</td><td>B</td></tr>
            </table>
        </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let table = &body.children[0];

    // Table content width should be 400px (minus margin/padding/border).
    // The block width algorithm resolves explicit widths.
    assert!(
        (table.dimensions.content.width - 400.0).abs() < 1.0,
        "table width should be ~400px, got {:.1}",
        table.dimensions.content.width
    );

    // Both cells should fit within the table width.
    // Parser wraps <tr> in implicit <tbody>.
    let tbody: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tbody"))
        .collect();
    assert!(!tbody.is_empty());
    let trs: Vec<&LayoutBox> = tbody[0]
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tr"))
        .collect();
    assert!(!trs.is_empty());
    let row = trs[0];
    if row.children.len() >= 2 {
        let cell_a = &row.children[0];
        let cell_b = &row.children[1];
        // Each column should get roughly half the table width (minus border-spacing).
        let a_width = cell_a.dimensions.content.width;
        let b_width = cell_b.dimensions.content.width;
        assert!(
            a_width > 50.0 && b_width > 50.0,
            "each cell should have reasonable width: A={a_width:.1}, B={b_width:.1}"
        );
    }
}

/// [§ 17.2.1](https://www.w3.org/TR/CSS2/tables.html#anonymous-boxes)
///
/// Table with colspan: merged cells should span multiple columns.
#[test]
fn test_table_colspan() {
    let root = layout_html(
        "<html><body>
            <style>table { width: 400px; }</style>
            <table>
                <tr><td colspan=\"2\">Header</td></tr>
                <tr><td>Left</td><td>Right</td></tr>
            </table>
        </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let table = &body.children[0];

    // Parser wraps <tr> in implicit <tbody>.
    let tbody: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tbody"))
        .collect();
    assert_eq!(tbody.len(), 1, "table should have 1 implicit <tbody>");

    let trs: Vec<&LayoutBox> = tbody[0]
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tr"))
        .collect();
    assert_eq!(trs.len(), 2, "table should have 2 rows");

    // Row 1: single cell with colspan=2 should be wider than either cell
    // in row 2.
    let header_cell = &trs[0].children[0];
    let left_cell = &trs[1].children[0];
    let right_cell = &trs[1].children[1];

    assert!(
        header_cell.dimensions.content.width
            > left_cell.dimensions.content.width + 1.0,
        "colspan=2 cell should be wider than single cell: header={:.1}, left={:.1}",
        header_cell.dimensions.content.width,
        left_cell.dimensions.content.width
    );

    // The colspan cell width should be approximately equal to the sum of
    // the two single cells plus the border-spacing between them.
    let expected_width =
        left_cell.dimensions.content.width + right_cell.dimensions.content.width;
    // Allow for border-spacing (2px) + padding differences.
    assert!(
        (header_cell.dimensions.content.width - expected_width).abs() < 10.0,
        "colspan cell width should be ~sum of single cells: header={:.1}, expected~{:.1}",
        header_cell.dimensions.content.width,
        expected_width
    );
}

/// [§ 17.2.1](https://www.w3.org/TR/CSS2/tables.html#table-display)
///
/// Table with row groups: <thead> + <tbody> rows should be laid out correctly.
#[test]
fn test_table_row_groups() {
    let root = layout_html(
        "<html><body>
            <table>
                <thead><tr><td>H1</td><td>H2</td></tr></thead>
                <tbody>
                    <tr><td>A</td><td>B</td></tr>
                    <tr><td>C</td><td>D</td></tr>
                </tbody>
            </table>
        </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let table = &body.children[0];

    // Table should have positive height.
    assert!(
        table.dimensions.content.height > 0.0,
        "table with row groups should have positive height"
    );

    // Find the <thead> and <tbody> children.
    let thead: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("thead"))
        .collect();
    let tbody: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tbody"))
        .collect();

    assert_eq!(thead.len(), 1, "should have 1 thead");
    assert_eq!(tbody.len(), 1, "should have 1 tbody");

    // The thead row's cells should be above the tbody rows' cells.
    // thead's first <tr> cells.
    let thead_tr = &thead[0].children[0];
    let thead_cell_y = thead_tr.children[0].dimensions.content.y;

    // tbody's first <tr> cells.
    let tbody_tr = &tbody[0].children[0];
    let tbody_cell_y = tbody_tr.children[0].dimensions.content.y;

    assert!(
        tbody_cell_y > thead_cell_y,
        "tbody cells should be below thead cells: thead_y={:.1}, tbody_y={:.1}",
        thead_cell_y,
        tbody_cell_y
    );
}

/// Table cells with nested content (paragraphs and text).
#[test]
fn test_table_nested_content() {
    let root = layout_html(
        "<html><body>
            <table>
                <tr>
                    <td><p>Paragraph in cell</p></td>
                    <td>Plain text</td>
                </tr>
            </table>
        </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let table = &body.children[0];

    assert!(
        table.dimensions.content.height > 0.0,
        "table with nested content should have positive height"
    );

    // Find the row (parser wraps <tr> in implicit <tbody>).
    let tbody: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tbody"))
        .collect();
    assert_eq!(tbody.len(), 1);
    let trs: Vec<&LayoutBox> = tbody[0]
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tr"))
        .collect();
    assert_eq!(trs.len(), 1);

    // Both cells should be on the same row and have positive dimensions.
    let row = trs[0];
    assert!(row.children.len() >= 2, "row should have at least 2 cells");
    let cell_1 = &row.children[0];
    let cell_2 = &row.children[1];

    assert!(
        cell_1.dimensions.content.height > 0.0,
        "cell with paragraph should have positive height"
    );
    assert!(
        cell_2.dimensions.content.height > 0.0,
        "cell with text should have positive height"
    );
    assert!(
        (cell_1.dimensions.content.y - cell_2.dimensions.content.y).abs() < 1.0,
        "both cells should be on the same row"
    );
}

/// [§ 15.3.10 Tables](https://html.spec.whatwg.org/multipage/rendering.html#tables-2)
///
/// Verify that <table> gets `display: table` from default_display_for_element.
#[test]
fn test_default_display_table() {
    assert_eq!(
        default_display_for_element("table"),
        Some(DisplayValue::table())
    );
}

/// <th> elements should render with bold text (font-weight: bold from UA).
#[test]
fn test_table_th_bold() {
    let root = layout_html(
        "<html><body>
            <table>
                <tr><th>Header</th><td>Data</td></tr>
            </table>
        </body></html>",
    );

    let body = box_at_depth(&root, 2);
    let table = &body.children[0];
    // Parser wraps <tr> in implicit <tbody>.
    let tbody: Vec<&LayoutBox> = table
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tbody"))
        .collect();
    assert!(!tbody.is_empty());
    let trs: Vec<&LayoutBox> = tbody[0]
        .children
        .iter()
        .filter(|c| c.tag_name.as_deref() == Some("tr"))
        .collect();
    assert!(!trs.is_empty());
    let row = trs[0];
    let th = &row.children[0];
    let td = &row.children[1];

    // <th> should have bold font-weight (700).
    assert!(
        th.font_weight >= 700,
        "th should be bold (font-weight >= 700), got {}",
        th.font_weight
    );
    // <td> should have normal font-weight (400).
    assert!(
        td.font_weight <= 400,
        "td should be normal weight (<= 400), got {}",
        td.font_weight
    );
}

// ---------------------------------------------------------------------------
// box-shadow tests
//
// [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
//
// "The 'box-shadow' property attaches one or more drop-shadows to the box."
// ---------------------------------------------------------------------------

/// `box-shadow: none` should produce an empty shadow vec.
#[test]
fn test_box_shadow_none() {
    let root = layout_html(
        "<style>div { box-shadow: none; }</style><div>Test</div>",
    );
    // Document > html > body > div
    let div = &box_at_depth(&root, 2).children[0];
    assert!(
        div.box_shadow.is_empty(),
        "box-shadow: none should produce empty vec, got {:?}",
        div.box_shadow
    );
}

/// Simple box-shadow with offset-x and offset-y only.
#[test]
fn test_box_shadow_simple() {
    let root = layout_html(
        "<style>div { box-shadow: 5px 10px black; }</style><div>Test</div>",
    );
    let div = &box_at_depth(&root, 2).children[0];
    assert_eq!(div.box_shadow.len(), 1, "should have 1 shadow");

    let s = &div.box_shadow[0];
    assert!((s.offset_x - 5.0).abs() < 0.01, "offset_x should be 5, got {}", s.offset_x);
    assert!((s.offset_y - 10.0).abs() < 0.01, "offset_y should be 10, got {}", s.offset_y);
    assert!((s.blur_radius - 0.0).abs() < 0.01, "blur should default to 0");
    assert!((s.spread_radius - 0.0).abs() < 0.01, "spread should default to 0");
    assert!(!s.inset, "should not be inset");
}

/// box-shadow with all 4 lengths: offset-x, offset-y, blur, spread.
#[test]
fn test_box_shadow_with_blur_and_spread() {
    let root = layout_html(
        "<style>div { box-shadow: 2px 3px 4px 5px red; }</style><div>Test</div>",
    );
    let div = &box_at_depth(&root, 2).children[0];
    assert_eq!(div.box_shadow.len(), 1);

    let s = &div.box_shadow[0];
    assert!((s.offset_x - 2.0).abs() < 0.01);
    assert!((s.offset_y - 3.0).abs() < 0.01);
    assert!((s.blur_radius - 4.0).abs() < 0.01);
    assert!((s.spread_radius - 5.0).abs() < 0.01);
    // red = #ff0000
    assert_eq!(s.color.r, 255);
    assert_eq!(s.color.g, 0);
    assert_eq!(s.color.b, 0);
}

/// box-shadow with `inset` keyword.
#[test]
fn test_box_shadow_inset() {
    let root = layout_html(
        "<style>div { box-shadow: inset 3px 4px 5px black; }</style><div>Test</div>",
    );
    let div = &box_at_depth(&root, 2).children[0];
    assert_eq!(div.box_shadow.len(), 1);
    assert!(div.box_shadow[0].inset, "should be inset shadow");
    assert!((div.box_shadow[0].offset_x - 3.0).abs() < 0.01);
    assert!((div.box_shadow[0].offset_y - 4.0).abs() < 0.01);
}

/// Multiple comma-separated shadows.
#[test]
fn test_box_shadow_multiple() {
    let root = layout_html(
        "<style>div { box-shadow: 1px 2px red, 3px 4px 5px blue; }</style><div>Test</div>",
    );
    let div = &box_at_depth(&root, 2).children[0];
    assert_eq!(div.box_shadow.len(), 2, "should have 2 shadows");

    // First shadow: 1px 2px red
    assert!((div.box_shadow[0].offset_x - 1.0).abs() < 0.01);
    assert!((div.box_shadow[0].offset_y - 2.0).abs() < 0.01);
    assert_eq!(div.box_shadow[0].color.r, 255);
    assert_eq!(div.box_shadow[0].color.g, 0);

    // Second shadow: 3px 4px 5px blue
    assert!((div.box_shadow[1].offset_x - 3.0).abs() < 0.01);
    assert!((div.box_shadow[1].offset_y - 4.0).abs() < 0.01);
    assert!((div.box_shadow[1].blur_radius - 5.0).abs() < 0.01);
    assert_eq!(div.box_shadow[1].color.b, 255);
}

/// When no color is specified, box-shadow defaults to the element's `color`.
#[test]
fn test_box_shadow_default_color() {
    let root = layout_html(
        "<style>div { color: green; box-shadow: 5px 5px; }</style><div>Test</div>",
    );
    let div = &box_at_depth(&root, 2).children[0];
    assert_eq!(div.box_shadow.len(), 1);

    // "green" = #008000
    let s = &div.box_shadow[0];
    assert_eq!(s.color.r, 0);
    assert_eq!(s.color.g, 128);
    assert_eq!(s.color.b, 0);
}

/// box-shadow is NOT inherited — child should not have parent's shadow.
#[test]
fn test_box_shadow_not_inherited() {
    let root = layout_html(
        "<style>.parent { box-shadow: 5px 5px black; }</style>\
         <div class='parent'><div class='child'>Child</div></div>",
    );
    let parent = &box_at_depth(&root, 2).children[0];
    assert_eq!(parent.box_shadow.len(), 1, "parent should have a shadow");

    let child = &parent.children[0];
    assert!(
        child.box_shadow.is_empty(),
        "child should NOT inherit parent's box-shadow, got {:?}",
        child.box_shadow
    );
}

// ---------------------------------------------------------------------------
// CSS Custom Properties (Variables) layout tests
//
// [CSS Custom Properties for Cascading Variables Module Level 1]
// (https://www.w3.org/TR/css-variables-1/)
// ---------------------------------------------------------------------------

/// [§ 3](https://www.w3.org/TR/css-variables-1/#using-variables)
///
/// var() in a shorthand property (margin) should substitute and parse correctly.
#[test]
fn test_var_in_margin_shorthand() {
    let root = layout_html(
        "<style>:root { --s: 20px; } div { margin: var(--s); }</style>\
         <div>Margin test</div>",
    );

    // Document > html > body > div
    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    let m = div.dimensions.margin;
    assert!(
        (m.top - 20.0).abs() < 0.1,
        "margin-top should be 20px from var(), got {}",
        m.top
    );
    assert!(
        (m.right - 20.0).abs() < 0.1,
        "margin-right should be 20px from var(), got {}",
        m.right
    );
    assert!(
        (m.bottom - 20.0).abs() < 0.1,
        "margin-bottom should be 20px from var(), got {}",
        m.bottom
    );
    assert!(
        (m.left - 20.0).abs() < 0.1,
        "margin-left should be 20px from var(), got {}",
        m.left
    );
}

/// [§ 3](https://www.w3.org/TR/css-variables-1/#using-variables)
///
/// var() in width property should work.
#[test]
fn test_var_in_width() {
    let root = layout_html(
        "<style>:root { --w: 200px; } div { width: var(--w); }</style>\
         <div>Width test</div>",
    );

    // Document > html > body > div
    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    assert!(
        (div.dimensions.content.width - 200.0).abs() < 0.1,
        "div width should be 200px from var(), got {}",
        div.dimensions.content.width
    );
}

/// [§ 3](https://www.w3.org/TR/css-variables-1/#using-variables)
///
/// var() fallback should work in layout properties.
#[test]
fn test_var_fallback_in_padding() {
    let root = layout_html(
        "<style>div { padding: var(--undefined, 10px); }</style>\
         <div>Padding test</div>",
    );

    let body = box_at_depth(&root, 2);
    let div = &body.children[0];

    let p = div.dimensions.padding;
    assert!(
        (p.top - 10.0).abs() < 0.1,
        "padding-top should be 10px from fallback, got {}",
        p.top
    );
}

// ---------------------------------------------------------------------------
// text-decoration tests
//
// [CSS Text Decoration Level 3 § 3](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
//
// "`text-decoration-line`: `none | [ underline || overline || line-through ]`"
// ---------------------------------------------------------------------------

/// Helper: collect all `TextRun`s from a `LayoutBox` tree (depth-first).
fn collect_text_runs(layout_box: &LayoutBox) -> Vec<&TextRun> {
    let mut runs = Vec::new();
    for line in &layout_box.line_boxes {
        for fragment in &line.fragments {
            if let FragmentContent::Text(ref text_run) = fragment.content {
                runs.push(text_run);
            }
        }
    }
    for child in &layout_box.children {
        runs.extend(collect_text_runs(child));
    }
    runs
}

/// [§ 15.3.8 Text-level semantics](https://html.spec.whatwg.org/multipage/rendering.html#text-level-semantics)
///
/// `<a>` elements should have `text-decoration: underline` via the UA stylesheet.
#[test]
fn test_text_decoration_ua_link_underline() {
    let root = layout_html("<a href='#'>Link text</a>");
    // Document > html > body — the <a> is inline inside body's anonymous inline content
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty(), "should have at least one text run");

    let link_run = &runs[0];
    assert!(
        link_run.text_decoration.underline,
        "UA stylesheet should give <a> underline decoration, got {:?}",
        link_run.text_decoration
    );
    assert!(
        !link_run.text_decoration.line_through,
        "link should not have line-through"
    );
    assert!(
        !link_run.text_decoration.overline,
        "link should not have overline"
    );
}

/// `text-decoration: underline` on an element should propagate to its text runs.
#[test]
fn test_text_decoration_underline() {
    let root = layout_html(
        "<style>span { text-decoration: underline; }</style>\
         <p><span>Underlined</span></p>",
    );
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty(), "should have text runs");

    let run = &runs[0];
    assert!(
        run.text_decoration.underline,
        "text-decoration: underline should set underline=true, got {:?}",
        run.text_decoration
    );
}

/// `text-decoration: line-through` should set the line_through flag.
#[test]
fn test_text_decoration_line_through() {
    let root = layout_html(
        "<style>.del { text-decoration: line-through; }</style>\
         <p><span class='del'>Deleted</span></p>",
    );
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty());

    let run = &runs[0];
    assert!(
        run.text_decoration.line_through,
        "text-decoration: line-through should set line_through=true, got {:?}",
        run.text_decoration
    );
    assert!(
        !run.text_decoration.underline,
        "line-through should not set underline"
    );
}

/// `text-decoration: overline` should set the overline flag.
#[test]
fn test_text_decoration_overline() {
    let root = layout_html(
        "<style>.over { text-decoration: overline; }</style>\
         <p><span class='over'>Overlined</span></p>",
    );
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty());

    let run = &runs[0];
    assert!(
        run.text_decoration.overline,
        "text-decoration: overline should set overline=true, got {:?}",
        run.text_decoration
    );
    assert!(
        !run.text_decoration.underline,
        "overline should not set underline"
    );
    assert!(
        !run.text_decoration.line_through,
        "overline should not set line_through"
    );
}

/// `text-decoration: none` should clear all decoration flags.
#[test]
fn test_text_decoration_none() {
    let root = layout_html(
        "<style>a { text-decoration: none; }</style>\
         <a href='#'>No underline</a>",
    );
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty());

    let run = &runs[0];
    assert!(
        !run.text_decoration.underline,
        "text-decoration: none should override UA underline, got {:?}",
        run.text_decoration
    );
    assert!(!run.text_decoration.overline);
    assert!(!run.text_decoration.line_through);
}

/// Combined values: `text-decoration: underline line-through`.
///
/// [§ 3.1](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
/// "Specifies what line decorations, if any, are added to the element."
/// Values can be combined.
#[test]
fn test_text_decoration_combined() {
    let root = layout_html(
        "<style>.combo { text-decoration: underline line-through; }</style>\
         <p><span class='combo'>Both</span></p>",
    );
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty());

    let run = &runs[0];
    assert!(
        run.text_decoration.underline,
        "combined should have underline, got {:?}",
        run.text_decoration
    );
    assert!(
        run.text_decoration.line_through,
        "combined should have line_through, got {:?}",
        run.text_decoration
    );
    assert!(
        !run.text_decoration.overline,
        "combined should not have overline"
    );
}

/// `text-decoration-line` longhand property should also work.
#[test]
fn test_text_decoration_line_longhand() {
    let root = layout_html(
        "<style>.x { text-decoration-line: overline underline; }</style>\
         <p><span class='x'>Both lines</span></p>",
    );
    let body = box_at_depth(&root, 2);
    let runs = collect_text_runs(body);
    assert!(!runs.is_empty());

    let run = &runs[0];
    assert!(run.text_decoration.underline, "longhand should set underline");
    assert!(run.text_decoration.overline, "longhand should set overline");
    assert!(!run.text_decoration.line_through, "longhand should not set line_through");
}

/// `text-decoration` is NOT inherited — a child element without its own
/// declaration should not inherit the parent's text-decoration.
///
/// [CSS Text Decoration Level 3 § 3.1](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
/// "Applies to: all elements" / "Inherited: no"
#[test]
fn test_text_decoration_not_inherited() {
    let root = layout_html(
        "<style>\
           .parent { text-decoration: underline; }\
         </style>\
         <div class='parent'><div class='child'>Child text</div></div>",
    );
    let parent = &box_at_depth(&root, 2).children[0];
    let child = &parent.children[0];

    assert!(
        parent.text_decoration.underline,
        "parent should have underline, got {:?}",
        parent.text_decoration
    );
    assert!(
        !child.text_decoration.underline,
        "child should NOT inherit text-decoration, got {:?}",
        child.text_decoration
    );
}

/// The LayoutBox.text_decoration field on the element directly should reflect
/// the computed style.
#[test]
fn test_text_decoration_on_layout_box() {
    let root = layout_html(
        "<style>div { text-decoration: overline line-through; }</style>\
         <div>Decorated</div>",
    );
    let div = &box_at_depth(&root, 2).children[0];

    assert!(
        div.text_decoration.overline,
        "LayoutBox should have overline, got {:?}",
        div.text_decoration
    );
    assert!(
        div.text_decoration.line_through,
        "LayoutBox should have line_through, got {:?}",
        div.text_decoration
    );
    assert!(
        !div.text_decoration.underline,
        "LayoutBox should not have underline"
    );
}
