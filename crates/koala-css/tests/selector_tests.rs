//! Integration tests for CSS selector parsing and matching.

use std::collections::HashMap;

use koala_css::selector::{
    AttributeSelector, Combinator, PseudoClass, SimpleSelector, Specificity, parse_selector,
};
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

// =============================================================================
// Pseudo-class and Pseudo-element Parsing Tests
// [§ 4 Pseudo-classes](https://www.w3.org/TR/selectors-4/#pseudo-classes)
// [§ 11 Pseudo-elements](https://www.w3.org/TR/selectors-4/#pseudo-elements)
// =============================================================================

#[test]
fn test_parse_hover_pseudo_class() {
    // :hover → interactive pseudo-class → NeverMatch
    let selector = parse_selector(":hover").unwrap();
    assert_eq!(selector.complex.subject.simple_selectors.len(), 1);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_pseudo_element_before() {
    // ::before → pseudo-element → NeverMatch
    let selector = parse_selector("::before").unwrap();
    assert_eq!(selector.complex.subject.simple_selectors.len(), 1);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_pseudo_element_after() {
    // ::after → pseudo-element → NeverMatch
    let selector = parse_selector("::after").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_legacy_pseudo_element_before() {
    // :before (single colon, legacy syntax) → NeverMatch
    let selector = parse_selector(":before").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_btn_hover_compound() {
    // .btn:hover → [Class("btn"), NeverMatch]
    let selector = parse_selector(".btn:hover").unwrap();
    assert_eq!(selector.complex.subject.simple_selectors.len(), 2);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Class(name) if name == "btn"
    ));
    assert!(matches!(
        &selector.complex.subject.simple_selectors[1],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_first_child_pseudo() {
    // a:first-child → [Type("a"), PseudoClass(FirstChild)]
    let selector = parse_selector("a:first-child").unwrap();
    assert_eq!(selector.complex.subject.simple_selectors.len(), 2);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Type(name) if name == "a"
    ));
    assert!(matches!(
        &selector.complex.subject.simple_selectors[1],
        SimpleSelector::PseudoClass(PseudoClass::FirstChild)
    ));
}

#[test]
fn test_parse_root_pseudo() {
    // :root → [PseudoClass(Root)]
    let selector = parse_selector(":root").unwrap();
    assert_eq!(selector.complex.subject.simple_selectors.len(), 1);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::Root)
    ));
}

#[test]
fn test_parse_structural_pseudo_classes() {
    assert!(matches!(
        parse_selector(":last-child")
            .unwrap()
            .complex
            .subject
            .simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::LastChild)
    ));
    assert!(matches!(
        parse_selector(":first-of-type")
            .unwrap()
            .complex
            .subject
            .simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::FirstOfType)
    ));
    assert!(matches!(
        parse_selector(":last-of-type")
            .unwrap()
            .complex
            .subject
            .simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::LastOfType)
    ));
    assert!(matches!(
        parse_selector(":only-child")
            .unwrap()
            .complex
            .subject
            .simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::OnlyChild)
    ));
    assert!(matches!(
        parse_selector(":empty")
            .unwrap()
            .complex
            .subject
            .simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::Empty)
    ));
    assert!(matches!(
        parse_selector(":link")
            .unwrap()
            .complex
            .subject
            .simple_selectors[0],
        SimpleSelector::PseudoClass(PseudoClass::Link)
    ));
}

#[test]
fn test_parse_functional_pseudo_class() {
    // :nth-child(2) → NeverMatch (functional pseudo-class, consumed but not evaluated)
    let selector = parse_selector(":nth-child(2)").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_not_pseudo_class() {
    // :not(.foo) → NeverMatch for now
    let selector = parse_selector(":not(.foo)").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

#[test]
fn test_parse_vendor_prefixed_pseudo_element() {
    // ::-webkit-scrollbar → NeverMatch
    let selector = parse_selector("::-webkit-scrollbar").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::NeverMatch
    ));
}

// =============================================================================
// Attribute Selector Parsing Tests
// [§ 6.4 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
// =============================================================================

#[test]
fn test_parse_attribute_exists() {
    // [href] → Attribute(Exists("href"))
    let selector = parse_selector("[href]").unwrap();
    assert_eq!(selector.complex.subject.simple_selectors.len(), 1);
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::Exists(name)) if name == "href"
    ));
}

#[test]
fn test_parse_attribute_equals() {
    // [type=text] → Attribute(Equals("type", "text"))
    let selector = parse_selector("[type=text]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::Equals(name, val))
            if name == "type" && val == "text"
    ));
}

#[test]
fn test_parse_attribute_equals_quoted() {
    // [type="text"] → Attribute(Equals("type", "text"))
    let selector = parse_selector("[type=\"text\"]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::Equals(name, val))
            if name == "type" && val == "text"
    ));
}

#[test]
fn test_parse_attribute_includes() {
    // [class~=foo] → Attribute(Includes("class", "foo"))
    let selector = parse_selector("[class~=foo]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::Includes(name, val))
            if name == "class" && val == "foo"
    ));
}

#[test]
fn test_parse_attribute_dash_match() {
    // [lang|=en] → Attribute(DashMatch("lang", "en"))
    let selector = parse_selector("[lang|=en]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::DashMatch(name, val))
            if name == "lang" && val == "en"
    ));
}

#[test]
fn test_parse_attribute_prefix_match() {
    // [href^=https] → Attribute(PrefixMatch("href", "https"))
    let selector = parse_selector("[href^=https]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::PrefixMatch(name, val))
            if name == "href" && val == "https"
    ));
}

#[test]
fn test_parse_attribute_suffix_match() {
    // [src$=".png"] → Attribute(SuffixMatch("src", ".png"))
    let selector = parse_selector("[src$=\".png\"]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::SuffixMatch(name, val))
            if name == "src" && val == ".png"
    ));
}

#[test]
fn test_parse_attribute_substring_match() {
    // [data-theme*=dark] → Attribute(SubstringMatch("data-theme", "dark"))
    let selector = parse_selector("[data-theme*=dark]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::SubstringMatch(name, val))
            if name == "data-theme" && val == "dark"
    ));
}

#[test]
fn test_parse_attribute_with_whitespace() {
    // [ href = "value" ] → Attribute(Equals("href", "value"))
    let selector = parse_selector("[ href = \"value\" ]").unwrap();
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::Equals(name, val))
            if name == "href" && val == "value"
    ));
}

#[test]
fn test_parse_complex_selector_with_pseudo_and_attr() {
    // div.class:hover [attr=val] → complex selector parses fully
    let selector = parse_selector("div.class:hover [attr=val]").unwrap();
    // Subject: [Attribute(Equals("attr", "val"))]
    assert!(matches!(
        &selector.complex.subject.simple_selectors[0],
        SimpleSelector::Attribute(AttributeSelector::Equals(name, val))
            if name == "attr" && val == "val"
    ));
    // Combinator chain: Descendant with compound [Type("div"), Class("class"), NeverMatch]
    assert_eq!(selector.complex.combinators.len(), 1);
    assert_eq!(selector.complex.combinators[0].0, Combinator::Descendant);
    assert_eq!(selector.complex.combinators[0].1.simple_selectors.len(), 3);
}

// =============================================================================
// Pseudo-class Matching Tests (with DOM tree context)
// =============================================================================

#[test]
fn test_matches_first_child() {
    // Build: <div><p>first</p><p>second</p></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let p1_id = tree.alloc(make_element_type("p", None, &[]));
    let p2_id = tree.alloc(make_element_type("p", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, p1_id);
    tree.append_child(div_id, p2_id);

    let selector = parse_selector(":first-child").unwrap();
    assert!(selector.matches_in_tree(&tree, p1_id));
    assert!(!selector.matches_in_tree(&tree, p2_id));
}

#[test]
fn test_matches_last_child() {
    // Build: <div><p>first</p><p>second</p></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let p1_id = tree.alloc(make_element_type("p", None, &[]));
    let p2_id = tree.alloc(make_element_type("p", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, p1_id);
    tree.append_child(div_id, p2_id);

    let selector = parse_selector(":last-child").unwrap();
    assert!(!selector.matches_in_tree(&tree, p1_id));
    assert!(selector.matches_in_tree(&tree, p2_id));
}

#[test]
fn test_matches_first_of_type() {
    // Build: <div><div>first div</div><p>first p</p><p>second p</p></div>
    let mut tree = DomTree::new();
    let parent_id = tree.alloc(make_element_type("div", None, &[]));
    let inner_div_id = tree.alloc(make_element_type("div", None, &[]));
    let p1_id = tree.alloc(make_element_type("p", None, &[]));
    let p2_id = tree.alloc(make_element_type("p", None, &[]));

    tree.append_child(NodeId::ROOT, parent_id);
    tree.append_child(parent_id, inner_div_id);
    tree.append_child(parent_id, p1_id);
    tree.append_child(parent_id, p2_id);

    // p:first-of-type matches the first <p> even when preceded by <div>
    let selector = parse_selector("p:first-of-type").unwrap();
    assert!(selector.matches_in_tree(&tree, p1_id));
    assert!(!selector.matches_in_tree(&tree, p2_id));
    assert!(!selector.matches_in_tree(&tree, inner_div_id)); // wrong tag
}

#[test]
fn test_matches_root() {
    // Build: Document → html → body
    let mut tree = DomTree::new();
    let html_id = tree.alloc(make_element_type("html", None, &[]));
    let body_id = tree.alloc(make_element_type("body", None, &[]));

    tree.append_child(NodeId::ROOT, html_id);
    tree.append_child(html_id, body_id);

    let selector = parse_selector(":root").unwrap();
    assert!(selector.matches_in_tree(&tree, html_id));
    assert!(!selector.matches_in_tree(&tree, body_id));
}

#[test]
fn test_matches_only_child() {
    // Build: <div><p>only child</p></div>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let p_id = tree.alloc(make_element_type("p", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, p_id);

    let selector = parse_selector(":only-child").unwrap();
    assert!(selector.matches_in_tree(&tree, p_id));

    // Add another child — now p is no longer only-child
    let span_id = tree.alloc(make_element_type("span", None, &[]));
    tree.append_child(div_id, span_id);

    assert!(!selector.matches_in_tree(&tree, p_id));
    assert!(!selector.matches_in_tree(&tree, span_id));
}

#[test]
fn test_matches_empty() {
    // Build: <br/> (no children) and <p>text</p>
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));
    let br_id = tree.alloc(make_element_type("br", None, &[]));
    let p_id = tree.alloc(make_element_type("p", None, &[]));
    let text_id = tree.alloc(NodeType::Text("some text".to_string()));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, br_id);
    tree.append_child(div_id, p_id);
    tree.append_child(p_id, text_id);

    let selector = parse_selector(":empty").unwrap();
    assert!(selector.matches_in_tree(&tree, br_id)); // no children
    assert!(!selector.matches_in_tree(&tree, p_id)); // has text child
}

#[test]
fn test_matches_link() {
    // <a href="..."> matches :link, <a> without href does not
    let mut tree = DomTree::new();
    let div_id = tree.alloc(make_element_type("div", None, &[]));

    let mut a_attrs = HashMap::new();
    let _ = a_attrs.insert("href".to_string(), "https://example.com".to_string());
    let a_with_href = tree.alloc(NodeType::Element(ElementData {
        tag_name: "a".to_string(),
        attrs: a_attrs,
    }));
    let a_without_href = tree.alloc(make_element_type("a", None, &[]));

    tree.append_child(NodeId::ROOT, div_id);
    tree.append_child(div_id, a_with_href);
    tree.append_child(div_id, a_without_href);

    let selector = parse_selector(":link").unwrap();
    assert!(selector.matches_in_tree(&tree, a_with_href));
    assert!(!selector.matches_in_tree(&tree, a_without_href));
}

// =============================================================================
// Attribute Selector Matching Tests
// =============================================================================

fn make_element_with_attrs(tag: &str, attrs: &[(&str, &str)]) -> ElementData {
    let mut attr_map = HashMap::new();
    for (k, v) in attrs {
        let _ = attr_map.insert(k.to_string(), v.to_string());
    }
    ElementData {
        tag_name: tag.to_string(),
        attrs: attr_map,
    }
}

#[test]
fn test_match_attribute_exists() {
    let selector = parse_selector("[class]").unwrap();
    let with_class = make_element_with_attrs("div", &[("class", "foo")]);
    let without_class = make_element_with_attrs("div", &[]);

    assert!(selector.matches(&with_class));
    assert!(!selector.matches(&without_class));
}

#[test]
fn test_match_attribute_equals() {
    let selector = parse_selector("[type=text]").unwrap();
    let matches = make_element_with_attrs("input", &[("type", "text")]);
    let no_match = make_element_with_attrs("input", &[("type", "password")]);

    assert!(selector.matches(&matches));
    assert!(!selector.matches(&no_match));
}

#[test]
fn test_match_attribute_includes() {
    // [class~=bar] — word match in space-separated list
    let selector = parse_selector("[class~=bar]").unwrap();
    let matches = make_element_with_attrs("div", &[("class", "foo bar baz")]);
    let no_match = make_element_with_attrs("div", &[("class", "foobar")]);

    assert!(selector.matches(&matches));
    assert!(!selector.matches(&no_match));
}

#[test]
fn test_match_attribute_dash_match() {
    // [lang|=en] — exact "en" or starts with "en-"
    let selector = parse_selector("[lang|=en]").unwrap();
    let exact = make_element_with_attrs("p", &[("lang", "en")]);
    let prefix = make_element_with_attrs("p", &[("lang", "en-US")]);
    let no_match = make_element_with_attrs("p", &[("lang", "fr")]);

    assert!(selector.matches(&exact));
    assert!(selector.matches(&prefix));
    assert!(!selector.matches(&no_match));
}

#[test]
fn test_match_attribute_prefix() {
    let selector = parse_selector("[href^=https]").unwrap();
    let matches = make_element_with_attrs("a", &[("href", "https://example.com")]);
    let no_match = make_element_with_attrs("a", &[("href", "http://example.com")]);

    assert!(selector.matches(&matches));
    assert!(!selector.matches(&no_match));
}

#[test]
fn test_match_attribute_suffix() {
    let selector = parse_selector("[src$=\".png\"]").unwrap();
    let matches = make_element_with_attrs("img", &[("src", "image.png")]);
    let no_match = make_element_with_attrs("img", &[("src", "image.jpg")]);

    assert!(selector.matches(&matches));
    assert!(!selector.matches(&no_match));
}

#[test]
fn test_match_attribute_substring() {
    let selector = parse_selector("[data-theme*=dark]").unwrap();
    let matches = make_element_with_attrs("div", &[("data-theme", "my-dark-theme")]);
    let no_match = make_element_with_attrs("div", &[("data-theme", "light")]);

    assert!(selector.matches(&matches));
    assert!(!selector.matches(&no_match));
}

#[test]
fn test_never_match_doesnt_match() {
    // .btn:hover → NeverMatch makes the whole compound fail
    let selector = parse_selector(".btn:hover").unwrap();
    let btn = make_element("div", None, &["btn"]);
    assert!(!selector.matches(&btn));
}

// =============================================================================
// Specificity Tests for New Variants
// [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
// =============================================================================

#[test]
fn test_specificity_first_child() {
    // :first-child → pseudo-class = (0,1,0)
    let selector = parse_selector(":first-child").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 1, 0));
}

#[test]
fn test_specificity_attribute() {
    // [href] → attribute selector = (0,1,0)
    let selector = parse_selector("[href]").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 1, 0));
}

#[test]
fn test_specificity_btn_hover() {
    // .btn:hover → Class(0,1,0) + NeverMatch(0,0,0) = (0,1,0)
    let selector = parse_selector(".btn:hover").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 1, 0));
}

#[test]
fn test_specificity_root() {
    // :root → pseudo-class = (0,1,0)
    let selector = parse_selector(":root").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 1, 0));
}

#[test]
fn test_specificity_type_with_attribute() {
    // input[type=text] → Type(0,0,1) + Attr(0,1,0) = (0,1,1)
    let selector = parse_selector("input[type=text]").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 1, 1));
}

#[test]
fn test_specificity_pseudo_element() {
    // ::before → NeverMatch = (0,0,0) (pseudo-element would be C but we use NeverMatch)
    let selector = parse_selector("::before").unwrap();
    assert_eq!(selector.specificity, Specificity(0, 0, 0));
}
