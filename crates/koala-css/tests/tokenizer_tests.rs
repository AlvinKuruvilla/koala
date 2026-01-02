//! Integration tests for the CSS tokenizer.

use koala_css::tokenizer::{CSSToken, CSSTokenizer, HashType, NumericType};

/// Helper to tokenize a string and return the tokens
fn tokenize(input: &str) -> Vec<CSSToken> {
    let mut tokenizer = CSSTokenizer::new(input);
    tokenizer.run();
    tokenizer.into_tokens()
}

#[test]
fn test_whitespace() {
    let tokens = tokenize("   \t\n  ");
    assert_eq!(tokens.len(), 2); // whitespace + EOF
    assert!(matches!(tokens[0], CSSToken::Whitespace));
    assert!(matches!(tokens[1], CSSToken::EOF));
}

#[test]
fn test_ident() {
    let tokens = tokenize("color");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Ident(name) => assert_eq!(name, "color"),
        _ => panic!("Expected Ident token"),
    }
}

#[test]
fn test_ident_with_hyphen() {
    let tokens = tokenize("background-color");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Ident(name) => assert_eq!(name, "background-color"),
        _ => panic!("Expected Ident token"),
    }
}

#[test]
fn test_ident_with_underscore() {
    let tokens = tokenize("_private");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Ident(name) => assert_eq!(name, "_private"),
        _ => panic!("Expected Ident token"),
    }
}

#[test]
fn test_function() {
    let tokens = tokenize("rgb(");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Function(name) => assert_eq!(name, "rgb"),
        _ => panic!("Expected Function token"),
    }
}

#[test]
fn test_at_keyword() {
    let tokens = tokenize("@media");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::AtKeyword(name) => assert_eq!(name, "media"),
        _ => panic!("Expected AtKeyword token"),
    }
}

#[test]
fn test_hash_id() {
    let tokens = tokenize("#header");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Hash { value, hash_type } => {
            assert_eq!(value, "header");
            assert_eq!(*hash_type, HashType::Id);
        }
        _ => panic!("Expected Hash token"),
    }
}

#[test]
fn test_hash_hex_color() {
    // #ff0000 starts with 'f' which is an ident-start code point,
    // so it's treated as an id-type hash per the spec
    let tokens = tokenize("#ff0000");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Hash { value, hash_type } => {
            assert_eq!(value, "ff0000");
            assert_eq!(*hash_type, HashType::Id);
        }
        _ => panic!("Expected Hash token"),
    }
}

#[test]
fn test_hash_numeric_unrestricted() {
    // #123 starts with a digit, which is NOT an ident-start code point,
    // so it's unrestricted type
    let tokens = tokenize("#123");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Hash { value, hash_type } => {
            assert_eq!(value, "123");
            assert_eq!(*hash_type, HashType::Unrestricted);
        }
        _ => panic!("Expected Hash token"),
    }
}

#[test]
fn test_string_double_quote() {
    let tokens = tokenize("\"hello world\"");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::String(value) => assert_eq!(value, "hello world"),
        _ => panic!("Expected String token"),
    }
}

#[test]
fn test_string_single_quote() {
    let tokens = tokenize("'hello world'");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::String(value) => assert_eq!(value, "hello world"),
        _ => panic!("Expected String token"),
    }
}

#[test]
fn test_integer() {
    let tokens = tokenize("42");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Number {
            value,
            int_value,
            numeric_type,
        } => {
            assert_eq!(*value, 42.0);
            assert_eq!(*int_value, Some(42));
            assert_eq!(*numeric_type, NumericType::Integer);
        }
        _ => panic!("Expected Number token"),
    }
}

#[test]
fn test_negative_integer() {
    let tokens = tokenize("-10");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Number {
            value,
            int_value,
            numeric_type,
        } => {
            assert_eq!(*value, -10.0);
            assert_eq!(*int_value, Some(-10));
            assert_eq!(*numeric_type, NumericType::Integer);
        }
        _ => panic!("Expected Number token"),
    }
}

#[test]
fn test_float() {
    let tokens = tokenize("3.14");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Number {
            value,
            numeric_type,
            ..
        } => {
            assert!((value - 3.14).abs() < 0.001);
            assert_eq!(*numeric_type, NumericType::Number);
        }
        _ => panic!("Expected Number token"),
    }
}

#[test]
fn test_percentage() {
    let tokens = tokenize("50%");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Percentage { value, .. } => {
            assert_eq!(*value, 50.0);
        }
        _ => panic!("Expected Percentage token"),
    }
}

#[test]
fn test_dimension_px() {
    let tokens = tokenize("16px");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Dimension { value, unit, .. } => {
            assert_eq!(*value, 16.0);
            assert_eq!(unit, "px");
        }
        _ => panic!("Expected Dimension token"),
    }
}

#[test]
fn test_dimension_em() {
    let tokens = tokenize("1.5em");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Dimension { value, unit, .. } => {
            assert!((value - 1.5).abs() < 0.001);
            assert_eq!(unit, "em");
        }
        _ => panic!("Expected Dimension token"),
    }
}

#[test]
fn test_colon() {
    let tokens = tokenize(":");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(tokens[0], CSSToken::Colon));
}

#[test]
fn test_semicolon() {
    let tokens = tokenize(";");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(tokens[0], CSSToken::Semicolon));
}

#[test]
fn test_comma() {
    let tokens = tokenize(",");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(tokens[0], CSSToken::Comma));
}

#[test]
fn test_braces() {
    let tokens = tokenize("{}");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(tokens[0], CSSToken::LeftBrace));
    assert!(matches!(tokens[1], CSSToken::RightBrace));
}

#[test]
fn test_brackets() {
    let tokens = tokenize("[]");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(tokens[0], CSSToken::LeftBracket));
    assert!(matches!(tokens[1], CSSToken::RightBracket));
}

#[test]
fn test_parens() {
    let tokens = tokenize("()");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(tokens[0], CSSToken::LeftParen));
    assert!(matches!(tokens[1], CSSToken::RightParen));
}

#[test]
fn test_comment() {
    let tokens = tokenize("/* comment */ color");
    assert_eq!(tokens.len(), 3); // whitespace + ident + EOF
    assert!(matches!(tokens[0], CSSToken::Whitespace));
    match &tokens[1] {
        CSSToken::Ident(name) => assert_eq!(name, "color"),
        _ => panic!("Expected Ident token"),
    }
}

#[test]
fn test_cdo_cdc() {
    let tokens = tokenize("<!-- -->");
    assert_eq!(tokens.len(), 4); // CDO + whitespace + CDC + EOF
    assert!(matches!(tokens[0], CSSToken::CDO));
    assert!(matches!(tokens[1], CSSToken::Whitespace));
    assert!(matches!(tokens[2], CSSToken::CDC));
}

#[test]
fn test_url_unquoted() {
    let tokens = tokenize("url(image.png)");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Url(value) => assert_eq!(value, "image.png"),
        _ => panic!("Expected Url token, got {:?}", tokens[0]),
    }
}

#[test]
fn test_simple_rule() {
    let tokens = tokenize("color: red;");
    // color, :, whitespace, red, ;, EOF
    assert_eq!(tokens.len(), 6);
    match &tokens[0] {
        CSSToken::Ident(name) => assert_eq!(name, "color"),
        _ => panic!("Expected Ident token"),
    }
    assert!(matches!(tokens[1], CSSToken::Colon));
    assert!(matches!(tokens[2], CSSToken::Whitespace));
    match &tokens[3] {
        CSSToken::Ident(name) => assert_eq!(name, "red"),
        _ => panic!("Expected Ident token"),
    }
    assert!(matches!(tokens[4], CSSToken::Semicolon));
}

#[test]
fn test_selector_and_block() {
    let tokens = tokenize("body { }");
    // body, whitespace, {, whitespace, }, EOF
    assert_eq!(tokens.len(), 6);
    match &tokens[0] {
        CSSToken::Ident(name) => assert_eq!(name, "body"),
        _ => panic!("Expected Ident token"),
    }
    assert!(matches!(tokens[2], CSSToken::LeftBrace));
    assert!(matches!(tokens[4], CSSToken::RightBrace));
}

#[test]
fn test_class_selector() {
    let tokens = tokenize(".container");
    assert_eq!(tokens.len(), 3); // delim(.) + ident + EOF
    assert!(matches!(tokens[0], CSSToken::Delim('.')));
    match &tokens[1] {
        CSSToken::Ident(name) => assert_eq!(name, "container"),
        _ => panic!("Expected Ident token"),
    }
}

#[test]
fn test_escaped_character() {
    let tokens = tokenize("\\41 "); // \41 is 'A' in hex
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Ident(name) => assert_eq!(name, "A"),
        _ => panic!("Expected Ident token with escaped char"),
    }
}

#[test]
fn test_scientific_notation() {
    let tokens = tokenize("1e10");
    assert_eq!(tokens.len(), 2);
    match &tokens[0] {
        CSSToken::Number {
            value,
            numeric_type,
            ..
        } => {
            assert_eq!(*value, 1e10);
            assert_eq!(*numeric_type, NumericType::Number);
        }
        _ => panic!("Expected Number token"),
    }
}
