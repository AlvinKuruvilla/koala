//! Integration tests for CSS selector parsing and matching.

use std::collections::HashMap;

use koala_css::selector::{Combinator, SimpleSelector, Specificity, parse_selector};
use koala_dom::{AttributesMap, DomTree, ElementData, NodeId, NodeType};

fn make_element(tag: &str, id: Option<&str>, classes: &[&str]) -> ElementData {
    let mut attrs = HashMap::new();
    if let Some(id_val) = id {
        let _ = attrs.insert("id".to_string(), id_val.to_string());
    }
    if !classes.is_empty() {
        let _ = attrs.insert("class".to_string(), classes.join(" "));
    }
    ElementData {
        tag_name: tag.to_string(),
        attrs,
    }
}

#[test]
fn test_parse_type_selector() {
    let selector = parse_selector("body").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 0, 1));
    assert_eq!(selector.complex.subject.simple_selectors.len(), 1);
    assert!(selector.complex.combinators.is_empty());
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "body"
    ));
}

#[test]
fn test_parse_class_selector() {
    let selector = parse_selector(".highlight").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 1, 0));
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Class(name) if name == "highlight"
    ));
}

#[test]
fn test_parse_id_selector() {
    let selector = parse_selector("#main-content").unwrap();
    assert_eq!(selector.specificity, Specificity(1, 0, 0));
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Id(name) if name == "main-content"
    ));
}

#[test]
fn test_parse_universal_selector() {
    let selector = parse_selector("*").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 0, 0));
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Universal
    ));
}

#[test]
fn test_parse_compound_selector() {
    // div.highlight#main
    let selector = parse_selector("div.highlight#main").unwrap();
    assert_eq!(selector.specificity, Specificity(1, 1, 1));
    assert_eq!(selector.complex.subject.simple_selectors.len(), 3);
    assert!(selector.complex.combinators.is_empty());
}

// Combinator Parsing Tests
// [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)

#[test]
fn test_parse_descendant_combinator() {
    // [§ 16.1 Descendant combinator](https://www.w3.org/TR/selectors-4/#descendant-combinators)
    // "A descendant combinator is whitespace that separates two compound selectors."
    let selector = parse_selector("div p").unwrap();

    // Subject should be "p"
    assert_eq!(selector.complex.subject.simple_selectors.len(), 1);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "p"
    ));

    // Should have one combinator: Descendant with "div"
    assert_eq!(selector.complex.combinators.len(), 1);
    assert_eq!(selector.complex.combinators[0].0, Combinator::Descendant);
    assert!(matches!(
        &selector.complex.combinators[0].1.simple_selectors[0],
        SimpleSelector::Type(name) if name == "div"
    ));

    // Specificity: 0,0,2 (two type selectors)
    assert_eq!(selector.specificity, Specificity(0, 0, 2));
}

#[test]
fn test_parse_child_combinator() {
    // [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
    // "A child combinator is a greater-than sign (>) that separates two compound selectors."
    let selector = parse_selector("ul > li").unwrap();

    // Subject should be "li"
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "li"
    ));

    // Should have one combinator: Child with "ul"
    assert_eq!(selector.complex.combinators.len(), 1);
    assert_eq!(selector.complex.combinators[0].0, Combinator::Child);
    assert!(matches!(
        &selector.complex.combinators[0].1.simple_selectors[0],
        SimpleSelector::Type(name) if name == "ul"
    ));

    assert_eq!(selector.specificity, Specificity(0, 0, 2));
}

#[test]
fn test_parse_next_sibling_combinator() {
    // [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
    // "A next-sibling combinator is a plus sign (+) that separates two compound selectors."
    let selector = parse_selector("h1 + p").unwrap();

    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "p"
    ));

    assert_eq!(selector.complex.combinators.len(), 1);
    assert_eq!(selector.complex.combinators[0].0, Combinator::NextSibling);

    assert_eq!(selector.specificity, Specificity(0, 0, 2));
}

#[test]
fn test_parse_subsequent_sibling_combinator() {
    // [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
    // "A subsequent-sibling combinator is a tilde (~) that separates two compound selectors."
    let selector = parse_selector("h1 ~ p").unwrap();

    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "p"
    ));

    assert_eq!(selector.complex.combinators.len(), 1);
    assert_eq!(
        selector.complex.combinators[0].0,
        Combinator::SubsequentSibling
    );

    assert_eq!(selector.specificity, Specificity(0, 0, 2));
}

#[test]
fn test_parse_multiple_combinators() {
    // [§ 4.3 Complex selectors](https://www.w3.org/TR/selectors-4/#complex)
    // "A complex selector is a chain of one or more compound selectors separated by combinators."
    let selector = parse_selector("div.container > ul.nav li a.active").unwrap();

    // Subject should be "a.active"
    assert_eq!(selector.complex.subject.simple_selectors.len(), 2);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "a"
    ));
    assert!(matches!(
        &selector.complex.subject.simple_selectors[1],
        SimpleSelector::Class(name) if name == "active"
    ));

    // Should have 3 combinators (in reverse order for matching):
    // [(Descendant, li), (Descendant, ul.nav), (Child, div.container)]
    assert_eq!(selector.complex.combinators.len(), 3);

    // First in chain (closest to subject): Descendant with "li"
    assert_eq!(selector.complex.combinators[0].0, Combinator::Descendant);
    assert!(matches!(
        &selector.complex.combinators[0].1.simple_selectors[0],
        SimpleSelector::Type(name) if name == "li"
    ));

    // Second: Descendant with "ul.nav"
    assert_eq!(selector.complex.combinators[1].0, Combinator::Descendant);
    assert_eq!(selector.complex.combinators[1].1.simple_selectors.len(), 2);

    // Third (leftmost): Child with "div.container"
    assert_eq!(selector.complex.combinators[2].0, Combinator::Child);

    // Specificity: 0 IDs, 3 classes (.container, .nav, .active), 4 types (div, ul, li, a)
    assert_eq!(selector.specificity, Specificity(0, 3, 4));
}

#[test]
fn test_parse_combinator_without_spaces() {
    // Combinators can appear without spaces around them
    let selector = parse_selector("div>p").unwrap();
    assert_eq!(selector.complex.combinators.len(), 1);
    assert_eq!(selector.complex.combinators[0].0, Combinator::Child);
}

#[test]
fn test_parse_invalid_leading_combinator() {
    // [§ 4.3](https://www.w3.org/TR/selectors-4/#complex)
    // A combinator without a left-hand compound is invalid
    assert!(parse_selector("> div").is_none());
    assert!(parse_selector("+ p").is_none());
    assert!(parse_selector("~ span").is_none());
}

#[test]
fn test_is_simple_selector() {
    // Simple selectors have no combinators
    let simple = parse_selector("div.class").unwrap();
    assert!(simple.is_simple());

    // Complex selectors have combinators
    let complex = parse_selector("div p").unwrap();
    assert!(!complex.is_simple());
}

// Simple Selector Matching Tests

#[test]
fn test_match_type_selector() {
    let selector = parse_selector("div").unwrap();
    let div = make_element("div", None, &[]);
    let span = make_element("span", None, &[]);

    assert!(selector.matches(&div));
    assert!(!selector.matches(&span));
}

#[test]
fn test_match_class_selector() {
    let selector = parse_selector(".highlight").unwrap();
    let with_class = make_element("span", None, &["highlight"]);
    let without_class = make_element("span", None, &["other"]);
    let multi_class = make_element("span", None, &["foo", "highlight", "bar"]);

    assert!(selector.matches(&with_class));
    assert!(!selector.matches(&without_class));
    assert!(selector.matches(&multi_class));
}

#[test]
fn test_match_id_selector() {
    let selector = parse_selector("#main-content").unwrap();
    let with_id = make_element("div", Some("main-content"), &[]);
    let wrong_id = make_element("div", Some("other"), &[]);
    let no_id = make_element("div", None, &[]);

    assert!(selector.matches(&with_id));
    assert!(!selector.matches(&wrong_id));
    assert!(!selector.matches(&no_id));
}

#[test]
fn test_match_compound_selector() {
    // All conditions must match
    let selector = parse_selector("div.highlight").unwrap();
    let matches_both = make_element("div", None, &["highlight"]);
    let wrong_tag = make_element("span", None, &["highlight"]);
    let wrong_class = make_element("div", None, &["other"]);

    assert!(selector.matches(&matches_both));
    assert!(!selector.matches(&wrong_tag));
    assert!(!selector.matches(&wrong_class));
}

// Specificity Tests
// [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)

#[test]
fn test_specificity_ordering() {
    // [§ 17](https://www.w3.org/TR/selectors-4/#specificity-rules)
    // "Specificities are compared by comparing the three components in order:
    // the specificity with a larger A value is more specific"
    let id = Specificity(1, 0, 0);
    let class = Specificity(0, 1, 0);
    let type_sel = Specificity(0, 0, 1);

    assert!(id > class);
    assert!(class > type_sel);
    assert!(id > type_sel);

    // Multiple classes beat one class
    let two_classes = Specificity(0, 2, 0);
    assert!(two_classes > class);

    // Class beats multiple types
    let three_types = Specificity(0, 0, 3);
    assert!(class > three_types);
}

#[test]
fn test_complex_selector_specificity() {
    // [§ 17](https://www.w3.org/TR/selectors-4/#specificity-rules)
    // "A selector's specificity is calculated for a given element as follows..."
    // All simple selectors in the complex selector are counted.

    // "div p" = 0,0,2
    let sel1 = parse_selector("div p").unwrap();
    assert_eq!(sel1.specificity, Specificity(0, 0, 2));

    // "#main .content p" = 1,1,1
    let sel2 = parse_selector("#main .content p").unwrap();
    assert_eq!(sel2.specificity, Specificity(1, 1, 1));

    // "div.class#id > ul.nav li" = 1,2,3
    let sel3 = parse_selector("div.class#id > ul.nav li").unwrap();
    assert_eq!(sel3.specificity, Specificity(1, 2, 3));
}

// Combinator Matching Tests (with DOM tree context)
// [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)

/// Helper to create element NodeType
fn make_element_type(tag: &str, id: Option<&str>, classes: &[&str]) -> NodeType {
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
fn test_matches_in_tree_simple() {
    // Simple selector should match without needing tree context
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &["container"]));
    tree.append_child(NodeId::ROOT, div_id);

    let selector = parse_selector("div.container").unwrap();
    assert!(selector.matches_in_tree(&tree, div_id));

    let wrong_selector = parse_selector("span.container").unwrap();
    assert!(!wrong_selector.matches_in_tree(&tree, div_id));
}

#[test]
fn test_matches_descendant_combinator() {
    // [§ 16.1 Descendant combinator](https://www.w3.org/TR/selectors-4/#descendant-combinators)
    // Build: <div class="container"><p><span>text</span></p></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &["container"]));
    let p_id = tree.alloc(make_element_type("p", None, &[]));
    let span_id = tree.alloc(make_element_type("span", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, p_id);
    tree.append_child(p_id, span_id);

    // "div span" should match span (div is ancestor)
    let selector = parse_selector("div span").unwrap();
    assert!(selector.matches_in_tree(&tree, span_id));

    // "div p" should match p
    let selector2 = parse_selector("div p").unwrap();
    assert!(selector2.matches_in_tree(&tree, p_id));

    // ".container span" should match span
    let selector3 = parse_selector(".container span").unwrap();
    assert!(selector3.matches_in_tree(&tree, span_id));

    // "ul span" should NOT match (no ul ancestor)
    let selector4 = parse_selector("ul span").unwrap();
    assert!(!selector4.matches_in_tree(&tree, span_id));
}

#[test]
fn test_matches_child_combinator() {
    // [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
    // Build: <div><p><span>text</span></p></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let p_id = tree.alloc(make_element_type("p", None, &[]));
    let span_id = tree.alloc(make_element_type("span", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, p_id);
    tree.append_child(p_id, span_id);

    // "div > p" should match p (p is direct child of div)
    let selector = parse_selector("div > p").unwrap();
    assert!(selector.matches_in_tree(&tree, p_id));

    // "p > span" should match span
    let selector2 = parse_selector("p > span").unwrap();
    assert!(selector2.matches_in_tree(&tree, span_id));

    // "div > span" should NOT match (span is grandchild, not child)
    let selector3 = parse_selector("div > span").unwrap();
    assert!(!selector3.matches_in_tree(&tree, span_id));
}

#[test]
fn test_matches_next_sibling_combinator() {
    // [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
    // Build: <div><h1></h1><p></p><span></span></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let h1_id = tree.alloc(make_element_type("h1", None, &[]));
    let p_id = tree.alloc(make_element_type("p", None, &[]));
    let span_id = tree.alloc(make_element_type("span", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, h1_id);
    tree.append_child(div_id, p_id);
    tree.append_child(div_id, span_id);

    // "h1 + p" should match p (p immediately follows h1)
    let selector = parse_selector("h1 + p").unwrap();
    assert!(selector.matches_in_tree(&tree, p_id));

    // "p + span" should match span
    let selector2 = parse_selector("p + span").unwrap();
    assert!(selector2.matches_in_tree(&tree, span_id));

    // "h1 + span" should NOT match (span doesn't immediately follow h1)
    let selector3 = parse_selector("h1 + span").unwrap();
    assert!(!selector3.matches_in_tree(&tree, span_id));
}

#[test]
fn test_matches_subsequent_sibling_combinator() {
    // [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
    // Build: <div><h1></h1><p></p><span></span></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let h1_id = tree.alloc(make_element_type("h1", None, &[]));
    let p_id = tree.alloc(make_element_type("p", None, &[]));
    let span_id = tree.alloc(make_element_type("span", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, h1_id);
    tree.append_child(div_id, p_id);
    tree.append_child(div_id, span_id);

    // "h1 ~ span" should match span (span follows h1, not immediately)
    let selector = parse_selector("h1 ~ span").unwrap();
    assert!(selector.matches_in_tree(&tree, span_id));

    // "h1 ~ p" should match p
    let selector2 = parse_selector("h1 ~ p").unwrap();
    assert!(selector2.matches_in_tree(&tree, p_id));

    // "span ~ p" should NOT match (p comes before span)
    let selector3 = parse_selector("span ~ p").unwrap();
    assert!(!selector3.matches_in_tree(&tree, p_id));
}

#[test]
fn test_matches_complex_combinator_chain() {
    // Test multiple combinators: "div.container > ul li a"
    // Build: <div class="container"><ul><li><a>link</a></li></ul></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &["container"]));
    let ul_id = tree.alloc(make_element_type("ul", None, &[]));
    let li_id = tree.alloc(make_element_type("li", None, &[]));
    let a_id = tree.alloc(make_element_type("a", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, ul_id);
    tree.append_child(ul_id, li_id);
    tree.append_child(li_id, a_id);

    // "div.container > ul li a" should match a
    let selector = parse_selector("div.container > ul li a").unwrap();
    assert!(selector.matches_in_tree(&tree, a_id));

    // "div.container > ul > li a" should also match (li is direct child of ul)
    let selector2 = parse_selector("div.container > ul > li a").unwrap();
    assert!(selector2.matches_in_tree(&tree, a_id));

    // "div.container > li a" should NOT match (li is not direct child of div)
    let selector3 = parse_selector("div.container > li a").unwrap();
    assert!(!selector3.matches_in_tree(&tree, a_id));
}
