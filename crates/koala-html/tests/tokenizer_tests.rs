//! Integration tests for the HTML tokenizer.

use koala_html::{HTMLTokenizer, Token};

/// Helper to tokenize a string and return the tokens
fn tokenize(input: &str) -> Vec<Token> {
    let mut tokenizer = HTMLTokenizer::new(input.to_string());
    tokenizer.run();
    tokenizer.into_tokens()
}

#[test]
fn test_plain_text() {
    let tokens = tokenize("Hello");
    assert_eq!(tokens.len(), 6); // 5 chars + EOF
    assert!(matches!(tokens[0], Token::Character { data: 'H' }));
    assert!(matches!(tokens[4], Token::Character { data: 'o' }));
    assert!(matches!(tokens[5], Token::EndOfFile));
}

#[test]
fn test_doctype() {
    let tokens = tokenize("<!DOCTYPE html>");
    assert_eq!(tokens.len(), 2); // DOCTYPE + EOF
    match &tokens[0] {
        Token::Doctype {
            name, force_quirks, ..
        } => {
            assert_eq!(name.as_deref(), Some("html"));
            assert!(!force_quirks);
        }
        _ => panic!("Expected DOCTYPE token"),
    }
}

#[test]
fn test_start_tag() {
    let tokens = tokenize("<div>");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        Token::StartTag {
            name,
            self_closing,
            attributes,
        } => {
            assert_eq!(name, "div");
            assert!(!self_closing);
            assert!(attributes.is_empty());
        }
        _ => panic!("Expected StartTag token"),
    }
}

#[test]
fn test_end_tag() {
    let tokens = tokenize("</div>");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        Token::EndTag { name, .. } => {
            assert_eq!(name, "div");
        }
        _ => panic!("Expected EndTag token"),
    }
}

#[test]
fn test_self_closing_tag() {
    let tokens = tokenize("<br/>");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        Token::StartTag {
            name, self_closing, ..
        } => {
            assert_eq!(name, "br");
            assert!(self_closing);
        }
        _ => panic!("Expected self-closing StartTag token"),
    }
}

#[test]
fn test_comment() {
    let tokens = tokenize("<!-- hello -->");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        Token::Comment { data } => {
            assert_eq!(data, " hello ");
        }
        _ => panic!("Expected Comment token"),
    }
}

#[test]
fn test_attribute_double_quoted() {
    let tokens = tokenize(r#"<div class="foo">"#);
    match &tokens[0] {
        Token::StartTag {
            name, attributes, ..
        } => {
            assert_eq!(name, "div");
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].name, "class");
            assert_eq!(attributes[0].value, "foo");
        }
        _ => panic!("Expected StartTag token"),
    }
}

#[test]
fn test_attribute_single_quoted() {
    let tokens = tokenize("<div class='bar'>");
    match &tokens[0] {
        Token::StartTag {
            name, attributes, ..
        } => {
            assert_eq!(name, "div");
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].name, "class");
            assert_eq!(attributes[0].value, "bar");
        }
        _ => panic!("Expected StartTag token"),
    }
}

#[test]
fn test_attribute_unquoted() {
    let tokens = tokenize("<div class=baz>");
    match &tokens[0] {
        Token::StartTag {
            name, attributes, ..
        } => {
            assert_eq!(name, "div");
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].name, "class");
            assert_eq!(attributes[0].value, "baz");
        }
        _ => panic!("Expected StartTag token"),
    }
}

#[test]
fn test_boolean_attribute() {
    let tokens = tokenize("<input disabled>");
    match &tokens[0] {
        Token::StartTag {
            name, attributes, ..
        } => {
            assert_eq!(name, "input");
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].name, "disabled");
            assert_eq!(attributes[0].value, "");
        }
        _ => panic!("Expected StartTag token"),
    }
}

#[test]
fn test_multiple_attributes() {
    let tokens = tokenize(r#"<input type="text" id="name" disabled>"#);
    match &tokens[0] {
        Token::StartTag {
            name, attributes, ..
        } => {
            assert_eq!(name, "input");
            assert_eq!(attributes.len(), 3);
            assert_eq!(attributes[0].name, "type");
            assert_eq!(attributes[0].value, "text");
            assert_eq!(attributes[1].name, "id");
            assert_eq!(attributes[1].value, "name");
            assert_eq!(attributes[2].name, "disabled");
            assert_eq!(attributes[2].value, "");
        }
        _ => panic!("Expected StartTag token"),
    }
}

#[test]
fn test_tag_with_text_content() {
    let tokens = tokenize("<p>Hi</p>");
    assert_eq!(tokens.len(), 5); // <p>, H, i, </p>, EOF
    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "p"));
    assert!(matches!(tokens[1], Token::Character { data: 'H' }));
    assert!(matches!(tokens[2], Token::Character { data: 'i' }));
    assert!(matches!(&tokens[3], Token::EndTag { name, .. } if name == "p"));
    assert!(matches!(tokens[4], Token::EndOfFile));
}

#[test]
fn test_simple_html_document() {
    let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>Hello</body>
</html>"#;
    let tokens = tokenize(html);

    // Should have DOCTYPE as first token
    assert!(matches!(&tokens[0], Token::Doctype { name: Some(n), .. } if n == "html"));

    // Should end with EOF
    assert!(matches!(tokens.last(), Some(Token::EndOfFile)));

    // Count tag tokens
    let start_tags: Vec<_> = tokens
        .iter()
        .filter(|t| matches!(t, Token::StartTag { .. }))
        .collect();
    let end_tags: Vec<_> = tokens
        .iter()
        .filter(|t| matches!(t, Token::EndTag { .. }))
        .collect();

    assert_eq!(start_tags.len(), 4); // html, head, title, body
    assert_eq!(end_tags.len(), 4); // /title, /head, /body, /html
}

// ========== Raw text element (RCDATA/RAWTEXT) tests ==========

#[test]
fn test_style_element_rawtext() {
    // Style content should be treated as raw text, not parsed as tags
    let tokens = tokenize("<style>body { color: red; }</style>");

    // Should have: <style>, characters for content, </style>, EOF
    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "style"));

    // Collect the character content
    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(content, "body { color: red; }");

    // Last tokens should be </style> and EOF
    assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "style"));
    assert!(matches!(tokens.last(), Some(Token::EndOfFile)));
}

#[test]
fn test_title_element_rcdata() {
    // Title content should be treated as RCDATA (raw text, but character references are parsed)
    let tokens = tokenize("<title>My Page</title>");

    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "title"));

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(content, "My Page");
    assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "title"));
}

#[test]
fn test_style_with_fake_tags() {
    // Tags inside style should NOT be parsed as tags
    let tokens = tokenize("<style><div>not a tag</div></style>");

    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "style"));

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    // The <div> and </div> should appear as literal text, not as tags
    assert_eq!(content, "<div>not a tag</div>");
    assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "style"));
}

#[test]
fn test_title_with_less_than() {
    // Less-than signs in title should be emitted as characters
    let tokens = tokenize("<title>a < b</title>");

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(content, "a < b");
}

#[test]
fn test_style_with_wrong_end_tag() {
    // </notastyle> inside style should NOT close the style element
    let tokens = tokenize("<style>a</notastyle>b</style>");

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    // The </notastyle> should appear as literal text
    assert_eq!(content, "a</notastyle>b");
}

#[test]
fn test_textarea_element_rcdata() {
    let tokens = tokenize("<textarea><b>bold?</b></textarea>");

    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "textarea"));

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    // Content should be literal text, not parsed tags
    assert_eq!(content, "<b>bold?</b>");
    assert!(matches!(&tokens[tokens.len() - 2], Token::EndTag { name, .. } if name == "textarea"));
}

#[test]
fn test_xmp_element_rawtext() {
    let tokens = tokenize("<xmp><html>is text</html></xmp>");

    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "xmp"));

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(content, "<html>is text</html>");
}

#[test]
fn test_iframe_element_rawtext() {
    let tokens = tokenize("<iframe>some content</iframe>");

    assert!(matches!(&tokens[0], Token::StartTag { name, .. } if name == "iframe"));

    let content: String = tokens[1..tokens.len() - 2]
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(content, "some content");
}

#[test]
fn test_character_reference_bare_ampersand() {
    // [§ 13.2.5.72 Character reference state]
    // Bare ampersand followed by non-alphanumeric should flush as literal '&'
    let tokens = tokenize("a & b");
    // Should be: 'a', ' ', '&', ' ', 'b', EOF
    assert_eq!(tokens.len(), 6);
    assert!(matches!(tokens[0], Token::Character { data: 'a' }));
    assert!(matches!(tokens[1], Token::Character { data: ' ' }));
    assert!(matches!(tokens[2], Token::Character { data: '&' }));
    assert!(matches!(tokens[3], Token::Character { data: ' ' }));
    assert!(matches!(tokens[4], Token::Character { data: 'b' }));
    assert!(matches!(tokens[5], Token::EndOfFile));
}

#[test]
fn test_named_character_reference_amp() {
    // [§ 13.2.5.73 Named character reference state]
    // &amp; should be replaced with &
    let tokens = tokenize("a &amp; b");
    let content: String = tokens
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(content, "a & b");
}

#[test]
fn test_named_character_reference_lt_gt() {
    // &lt; and &gt; should be replaced with < and >
    let tokens = tokenize("&lt;div&gt;");
    let content: String = tokens
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(content, "<div>");
}

#[test]
fn test_named_character_reference_without_semicolon() {
    // Legacy entities without semicolon should still work
    let tokens = tokenize("&amp is ok");
    let content: String = tokens
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(content, "& is ok");
}

#[test]
fn test_named_character_reference_unknown() {
    // Unknown entities should be passed through as-is
    let tokens = tokenize("&notreal;");
    let content: String = tokens
        .iter()
        .filter_map(|t| {
            if let Token::Character { data } = t {
                Some(*data)
            } else {
                None
            }
        })
        .collect();
    // The ampersand and entity name should be emitted as characters
    assert_eq!(content, "&notreal;");
}

#[test]
fn test_named_character_reference_in_attribute() {
    // Entities in attribute values should be replaced
    let tokens = tokenize(r#"<a href="?a=1&amp;b=2">"#);
    match &tokens[0] {
        Token::StartTag { attributes, .. } => {
            assert_eq!(attributes[0].value, "?a=1&b=2");
        }
        _ => panic!("Expected StartTag token"),
    }
}
