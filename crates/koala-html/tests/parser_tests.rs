//! Integration tests for the HTML parser.

use koala_dom::{DomTree, Node, NodeId, NodeType};
use koala_html::{HTMLParser, HTMLTokenizer};

/// Helper to parse HTML and return the DOM tree
fn parse(html: &str) -> DomTree {
    let mut tokenizer = HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let parser = HTMLParser::new(tokenizer.into_tokens());
    parser.run()
}

/// Helper to get element by tag name (first match, depth-first)
fn find_element(tree: &DomTree, from: NodeId, tag: &str) -> Option<NodeId> {
    if let Some(data) = tree.as_element(from)
        && data.tag_name == tag
    {
        return Some(from);
    }
    for &child_id in tree.children(from) {
        if let Some(found) = find_element(tree, child_id, tag) {
            return Some(found);
        }
    }
    None
}

/// Helper to get text content of a node (concatenated)
fn text_content(tree: &DomTree, id: NodeId) -> String {
    let mut result = String::new();
    if let Some(node) = tree.get(id) {
        match &node.node_type {
            NodeType::Text(data) => result.push_str(data),
            _ => {
                for &child_id in tree.children(id) {
                    result.push_str(&text_content(tree, child_id));
                }
            }
        }
    }
    result
}

/// Helper to get a node reference
fn get_node(tree: &DomTree, id: NodeId) -> &Node {
    tree.get(id).expect("Node not found")
}

#[test]
fn test_document_structure() {
    let tree = parse("<!DOCTYPE html><html><head></head><body></body></html>");

    // Root should be Document
    let root = get_node(&tree, NodeId::ROOT);
    assert!(matches!(root.node_type, NodeType::Document));

    // Document should have html child
    let html_id = find_element(&tree, NodeId::ROOT, "html");
    assert!(html_id.is_some());

    // html should have head and body
    let html_id = html_id.unwrap();
    let head_id = find_element(&tree, html_id, "head");
    let body_id = find_element(&tree, html_id, "body");
    assert!(head_id.is_some());
    assert!(body_id.is_some());
}

#[test]
fn test_text_node() {
    let tree = parse("<html><body>Hello World</body></html>");
    let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

    let text = text_content(&tree, body_id);
    assert_eq!(text, "Hello World");
}

#[test]
fn test_comment_node() {
    let tree = parse("<html><body><!-- test comment --></body></html>");
    let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

    // Body should have a comment child
    let has_comment = tree.children(body_id).iter().any(|&child_id| {
        tree.get(child_id).map_or(
            false,
            |node| matches!(&node.node_type, NodeType::Comment(data) if data == " test comment "),
        )
    });
    assert!(has_comment);
}

#[test]
fn test_nested_elements() {
    let tree = parse("<html><body><div><p>Text</p></div></body></html>");

    let div_id = find_element(&tree, NodeId::ROOT, "div").unwrap();
    let p_id = find_element(&tree, div_id, "p").unwrap();
    let text = text_content(&tree, p_id);

    assert_eq!(text, "Text");
}

#[test]
fn test_element_attributes() {
    let tree = parse(r#"<html><body><div id="main" class="container"></div></body></html>"#);
    let div_id = find_element(&tree, NodeId::ROOT, "div").unwrap();
    let div = get_node(&tree, div_id);

    if let NodeType::Element(data) = &div.node_type {
        assert_eq!(data.attrs.get("id"), Some(&"main".to_string()));
        assert_eq!(data.attrs.get("class"), Some(&"container".to_string()));
    } else {
        panic!("Expected Element");
    }
}

#[test]
fn test_void_elements() {
    let tree = parse(r#"<html><body><input type="text"><br></body></html>"#);
    let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

    // Both input and br should be children of body (void elements don't nest)
    let element_names: Vec<_> = tree
        .children(body_id)
        .iter()
        .filter_map(|&child_id| tree.as_element(child_id).map(|data| data.tag_name.as_str()))
        .collect();

    assert!(element_names.contains(&"input"));
    assert!(element_names.contains(&"br"));
}

#[test]
fn test_title_element() {
    let tree = parse("<html><head><title>My Page</title></head><body></body></html>");
    let title_id = find_element(&tree, NodeId::ROOT, "title").unwrap();
    let text = text_content(&tree, title_id);

    assert_eq!(text, "My Page");
}

#[test]
fn test_meta_element() {
    let tree = parse(r#"<html><head><meta charset="UTF-8"></head><body></body></html>"#);
    let meta_id = find_element(&tree, NodeId::ROOT, "meta").unwrap();
    let meta = get_node(&tree, meta_id);

    if let NodeType::Element(data) = &meta.node_type {
        assert_eq!(data.attrs.get("charset"), Some(&"UTF-8".to_string()));
    } else {
        panic!("Expected Element");
    }
}

#[test]
fn test_whitespace_preserved_in_text() {
    let tree = parse("<html><body>  hello  world  </body></html>");
    let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();
    let text = text_content(&tree, body_id);

    // Whitespace should be preserved
    assert_eq!(text, "  hello  world  ");
}

#[test]
fn test_multiple_text_nodes_merged() {
    // Adjacent character tokens should become a single text node
    let tree = parse("<html><body>abc</body></html>");
    let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();

    // Should have exactly one text node child (merged from a, b, c)
    let text_nodes: Vec<_> = tree
        .children(body_id)
        .iter()
        .filter(|&&child_id| {
            tree.get(child_id)
                .map(|n| matches!(n.node_type, NodeType::Text(_)))
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(text_nodes.len(), 1);
    assert_eq!(text_content(&tree, body_id), "abc");
}

#[test]
fn test_simple_html_file() {
    // Test parsing of the actual simple.html structure
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <!-- This is a comment -->
    <title>Test</title>
</head>
<body class="main" id="content">
    <!-- TODO: add more content -->
    <div data-value='single quoted'>Hello</div>
    <input type="text" disabled />
</body>
</html>"#;

    let tree = parse(html);

    // Check basic structure
    let root = get_node(&tree, NodeId::ROOT);
    assert!(matches!(root.node_type, NodeType::Document));

    let html_id = find_element(&tree, NodeId::ROOT, "html").unwrap();
    let html_elem = get_node(&tree, html_id);
    if let NodeType::Element(data) = &html_elem.node_type {
        assert_eq!(data.attrs.get("lang"), Some(&"en".to_string()));
    }

    // Check head elements
    let title_id = find_element(&tree, NodeId::ROOT, "title").unwrap();
    assert_eq!(text_content(&tree, title_id), "Test");

    let meta_id = find_element(&tree, NodeId::ROOT, "meta").unwrap();
    let meta = get_node(&tree, meta_id);
    if let NodeType::Element(data) = &meta.node_type {
        assert_eq!(data.attrs.get("charset"), Some(&"UTF-8".to_string()));
    }

    // Check body elements
    let body_id = find_element(&tree, NodeId::ROOT, "body").unwrap();
    let body = get_node(&tree, body_id);
    if let NodeType::Element(data) = &body.node_type {
        assert_eq!(data.attrs.get("class"), Some(&"main".to_string()));
        assert_eq!(data.attrs.get("id"), Some(&"content".to_string()));
    }

    // Check div with single-quoted attribute
    let div_id = find_element(&tree, NodeId::ROOT, "div").unwrap();
    let div = get_node(&tree, div_id);
    if let NodeType::Element(data) = &div.node_type {
        assert_eq!(
            data.attrs.get("data-value"),
            Some(&"single quoted".to_string())
        );
    }
    assert_eq!(text_content(&tree, div_id), "Hello");

    // Check input with boolean attribute
    let input_id = find_element(&tree, NodeId::ROOT, "input").unwrap();
    let input = get_node(&tree, input_id);
    if let NodeType::Element(data) = &input.node_type {
        assert_eq!(data.attrs.get("type"), Some(&"text".to_string()));
        assert_eq!(data.attrs.get("disabled"), Some(&"".to_string()));
    }
}

// ========== Raw text element tests at parser level ==========

#[test]
fn test_style_element_content_preserved() {
    // Style content should be preserved as text, not parsed as HTML
    let html = r#"<!DOCTYPE html>
<html>
<head>
<style>
body { color: red; }
.container { margin: 0; }
</style>
</head>
<body></body>
</html>"#;

    let tree = parse(html);
    let style = find_element(&tree, tree.root(), "style").unwrap();
    let content = text_content(&tree, style);

    // The CSS should be preserved as text
    assert!(content.contains("body { color: red; }"));
    assert!(content.contains(".container { margin: 0; }"));
}

#[test]
fn test_style_with_html_like_content() {
    // HTML-like content inside style should NOT be interpreted as tags
    let html = "<html><head><style><div>not a tag</div></style></head><body></body></html>";

    let tree = parse(html);
    let style = find_element(&tree, tree.root(), "style").unwrap();
    let content = text_content(&tree, style);

    // The <div> should appear as literal text
    assert_eq!(content, "<div>not a tag</div>");

    // There should be no div element in the document (since it's inside style)
    let body = find_element(&tree, tree.root(), "body").unwrap();
    let div_in_body = find_element(&tree, body, "div");
    assert!(div_in_body.is_none());
}

#[test]
fn test_title_content_preserved() {
    let html = "<html><head><title>My <test> Title</title></head><body></body></html>";

    let tree = parse(html);
    let title = find_element(&tree, tree.root(), "title").unwrap();
    let content = text_content(&tree, title);

    // Title content including < should be preserved
    assert_eq!(content, "My <test> Title");
}

// ========== Adoption Agency Algorithm Tests ==========

/// Helper to find all elements with a given tag name under a subtree
fn find_all_elements(tree: &DomTree, from: NodeId, tag: &str) -> Vec<NodeId> {
    let mut result = Vec::new();
    if let Some(data) = tree.as_element(from) {
        if data.tag_name == tag {
            result.push(from);
        }
    }
    for &child_id in tree.children(from) {
        result.extend(find_all_elements(tree, child_id, tag));
    }
    result
}

#[test]
fn test_adoption_agency_simple_misnesting() {
    // Classic misnesting: <p><b>X<i>Y</b>Z</i></p>
    //
    // Per spec, should produce:
    //   <p>
    //     <b>X<i>Y</i></b>
    //     <i>Z</i>
    //   </p>
    //
    // The <b> is closed, splitting the <i> at that point.
    // A new <i> is created to wrap Z.
    let tree = parse("<html><body><p><b>X<i>Y</b>Z</i></p></body></html>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();
    let p = find_element(&tree, body, "p").unwrap();

    // The p should contain a <b> and an <i>
    let p_children = tree.children(p);

    // Find the b element under p
    let b = find_element(&tree, p, "b").unwrap();
    // b should contain text "X" and an <i> with text "Y"
    assert_eq!(text_content(&tree, b), "XY");

    // Find <i> elements under p
    let i_elements = find_all_elements(&tree, p, "i");
    // There should be at least one <i> inside <b> with "Y" and one outside with "Z"
    assert!(
        i_elements.len() >= 2,
        "Expected at least 2 <i> elements, got {}",
        i_elements.len()
    );

    // The <b> should contain an <i> with "Y"
    let i_in_b = find_element(&tree, b, "i").unwrap();
    assert_eq!(text_content(&tree, i_in_b), "Y");

    // There should be an <i> after <b> (as child of p) with "Z"
    let mut found_z = false;
    for &child in p_children {
        if tree.as_element(child).is_some_and(|e| e.tag_name == "i")
            && text_content(&tree, child) == "Z"
        {
            found_z = true;
            break;
        }
    }
    assert!(found_z, "Expected <i>Z</i> as a direct child of <p>");
}

#[test]
fn test_adoption_agency_no_furthest_block() {
    // When there's no furthest block (no special element between formatting
    // element and top of stack), should just pop to the formatting element.
    //
    // <p><b><i>text</b></p>
    // The </b> sees <b> on stack with <i> above it, but <i> is NOT special.
    // So no furthest block → simple pop.
    let tree = parse("<html><body><p><b><i>text</b></p></body></html>");
    let p = find_element(&tree, NodeId::ROOT, "p").unwrap();
    let b = find_element(&tree, p, "b").unwrap();

    // b should contain <i> with "text"
    let i = find_element(&tree, b, "i").unwrap();
    assert_eq!(text_content(&tree, i), "text");
}

#[test]
fn test_formatting_reconstruction_across_blocks() {
    // When an implicit close occurs (e.g., <p> closes an open <p>), the AFL
    // reconstructs formatting elements that were on the stack.
    //
    // <p><b>bold</p><p>still bold</p></b>
    //
    // The second <p> implicitly closes the first <p>, removing <b> from
    // the stack. But <b> stays in the AFL, so it gets reconstructed.
    let tree = parse("<html><body><p><b>bold</p><p>still bold</p></b></body></html>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();

    // Should have two <p> elements
    let ps = find_all_elements(&tree, body, "p");
    assert!(
        ps.len() >= 2,
        "Expected at least 2 <p> elements, got {}",
        ps.len()
    );

    // First <p> should contain "bold" inside a <b>
    assert_eq!(text_content(&tree, ps[0]), "bold");
    let b_in_p1 = find_element(&tree, ps[0], "b");
    assert!(b_in_p1.is_some(), "Expected <b> inside first <p>");

    // Second <p> should contain "still bold" — and it should be
    // inside a reconstructed <b> (from AFL reconstruction)
    assert_eq!(text_content(&tree, ps[1]), "still bold");
    let b_in_p2 = find_element(&tree, ps[1], "b");
    assert!(
        b_in_p2.is_some(),
        "Expected <b> inside second <p> from formatting reconstruction"
    );
}

#[test]
fn test_properly_nested_formatting() {
    // Well-nested formatting should still work correctly.
    // <p><b>bold <i>bold-italic</i> bold</b></p>
    let tree = parse("<html><body><p><b>bold <i>bold-italic</i> bold</b></p></body></html>");
    let p = find_element(&tree, NodeId::ROOT, "p").unwrap();
    let b = find_element(&tree, p, "b").unwrap();
    let i = find_element(&tree, b, "i").unwrap();

    assert_eq!(text_content(&tree, i), "bold-italic");
    assert_eq!(text_content(&tree, b), "bold bold-italic bold");
}

#[test]
fn test_any_other_end_tag_ignores_special() {
    // </span> when there's a <div> (special) between current node and the
    // <span> on the stack should be ignored per "any other end tag" rules.
    let tree = parse("<html><body><span><div>text</div></span></body></html>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();

    // The span and div should both exist
    let span = find_element(&tree, body, "span").unwrap();
    let div = find_element(&tree, span, "div");
    assert!(div.is_some());
}

#[test]
fn test_nested_anchor_tags() {
    // Second <a> should close the first via adoption agency.
    // <a href="1">first<a href="2">second</a>
    let tree = parse(r#"<html><body><a href="1">first<a href="2">second</a></body></html>"#);
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();

    // Both <a> tags should exist under body
    let anchors = find_all_elements(&tree, body, "a");
    assert!(
        anchors.len() >= 2,
        "Expected at least 2 <a> elements, got {}",
        anchors.len()
    );

    // The first <a> should contain "first"
    let first_a = anchors[0];
    assert_eq!(text_content(&tree, first_a), "first");

    // The second <a> should contain "second"
    let second_a = anchors[1];
    assert_eq!(text_content(&tree, second_a), "second");
}

#[test]
fn test_generate_implied_end_tags() {
    // <p>text should be implicitly closed by a new <p>
    // <p>first<p>second
    let tree = parse("<html><body><p>first<p>second</body></html>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();

    let ps = find_all_elements(&tree, body, "p");
    assert_eq!(ps.len(), 2);
    assert_eq!(text_content(&tree, ps[0]), "first");
    assert_eq!(text_content(&tree, ps[1]), "second");
}

#[test]
fn test_scope_checking_basic() {
    // A stray </p> end tag when p is not in scope should be handled gracefully
    let tree = parse("<html><body></p>text</body></html>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();
    // The stray </p> might create an empty p or be ignored.
    // The text should still appear.
    let full_text = text_content(&tree, body);
    assert!(full_text.contains("text"));
}

// ---------------------------------------------------------------------------
// List element parsing tests
//
// [§ 13.2.6.4.7 "in body"](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
// ---------------------------------------------------------------------------

/// Helper to collect all direct element children with a given tag name.
fn element_children(tree: &DomTree, parent: NodeId, tag: &str) -> Vec<NodeId> {
    tree.children(parent)
        .iter()
        .copied()
        .filter(|&id| tree.as_element(id).is_some_and(|d| d.tag_name == tag))
        .collect()
}

#[test]
fn test_li_implicit_close() {
    // [§ 13.2.6.4.7](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    //
    // "A start tag whose tag name is "li""
    // When a second <li> is encountered, it should implicitly close the first.
    // Result: <ul> has two <li> children, not nested.
    let tree = parse("<ul><li>A<li>B</ul>");
    let ul = find_element(&tree, NodeId::ROOT, "ul").unwrap();
    let lis = element_children(&tree, ul, "li");
    assert_eq!(
        lis.len(),
        2,
        "ul should have 2 <li> children, got {}",
        lis.len()
    );
    assert_eq!(text_content(&tree, lis[0]), "A");
    assert_eq!(text_content(&tree, lis[1]), "B");
}

#[test]
fn test_dd_dt_implicit_close() {
    // [§ 13.2.6.4.7](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    //
    // "A start tag whose tag name is one of: "dd", "dt""
    // <dt> and <dd> implicitly close each other.
    let tree = parse("<dl><dt>T<dd>D<dt>T2</dl>");
    let dl = find_element(&tree, NodeId::ROOT, "dl").unwrap();
    let dts = element_children(&tree, dl, "dt");
    let dds = element_children(&tree, dl, "dd");
    assert_eq!(
        dts.len(),
        2,
        "dl should have 2 <dt> children, got {}",
        dts.len()
    );
    assert_eq!(
        dds.len(),
        1,
        "dl should have 1 <dd> child, got {}",
        dds.len()
    );
    assert_eq!(text_content(&tree, dts[0]), "T");
    assert_eq!(text_content(&tree, dds[0]), "D");
    assert_eq!(text_content(&tree, dts[1]), "T2");
}

#[test]
fn test_li_end_tag_no_scope() {
    // [§ 13.2.6.4.7](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    //
    // "An end tag whose tag name is "li""
    // "If the stack of open elements does not have an li element in
    //  list item scope, then this is a parse error; ignore the token."
    //
    // A stray </li> with no <li> in scope should be ignored.
    let tree = parse("<body></li>text</body>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();
    let full_text = text_content(&tree, body);
    assert!(
        full_text.contains("text"),
        "text should still appear after stray </li>"
    );
}

#[test]
fn test_nested_lists() {
    // Properly nested lists: inner <li> should be children of the inner <ul>,
    // not siblings of the outer <li>.
    let tree = parse("<ul><li>A<ul><li>B</li></ul></li></ul>");
    let outer_ul = find_element(&tree, NodeId::ROOT, "ul").unwrap();
    let outer_lis = element_children(&tree, outer_ul, "li");
    assert_eq!(outer_lis.len(), 1, "outer ul should have 1 <li>");

    let inner_ul = find_element(&tree, outer_lis[0], "ul").unwrap();
    let inner_lis = element_children(&tree, inner_ul, "li");
    assert_eq!(inner_lis.len(), 1, "inner ul should have 1 <li>");
    assert_eq!(text_content(&tree, inner_lis[0]), "B");
}

#[test]
fn test_ol_end_tag_scope_checking() {
    // [§ 13.2.6.4.7](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
    //
    // "An end tag whose tag name is one of: ... "ol" ..."
    // "If the stack of open elements does not have an element in scope that
    //  is an HTML element with the same tag name as that of the token, then
    //  this is a parse error; ignore the token."
    //
    // A stray </ol> with no <ol> in scope should be ignored.
    let tree = parse("<body></ol>text</body>");
    let body = find_element(&tree, NodeId::ROOT, "body").unwrap();
    let full_text = text_content(&tree, body);
    assert!(
        full_text.contains("text"),
        "text should still appear after stray </ol>"
    );
}
