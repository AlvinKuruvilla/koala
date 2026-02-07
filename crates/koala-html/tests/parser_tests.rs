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
