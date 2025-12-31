//! CSS Selector parsing and matching
//!
//! This module implements selector parsing and matching per
//! [Selectors Level 4](https://www.w3.org/TR/selectors-4/).

use crate::lib_dom::ElementData;

/// [§ 5 Elemental selectors](https://www.w3.org/TR/selectors-4/#elemental-selectors)
/// [§ 6 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
///
/// A simple selector is a single condition on an element.
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleSelector {
    /// [§ 5.1 Type selector](https://www.w3.org/TR/selectors-4/#type-selectors)
    /// "A type selector is the name of a document language element type,
    /// and represents an instance of that element type in the document tree."
    Type(String),

    /// [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
    /// "The class selector is given as a full stop (. U+002E) immediately
    /// followed by an identifier."
    Class(String),

    /// [§ 6.7 ID selector](https://www.w3.org/TR/selectors-4/#id-selectors)
    /// "An ID selector is a hash (#, U+0023) immediately followed by the
    /// ID value, which is an identifier."
    Id(String),

    /// [§ 5.2 Universal selector](https://www.w3.org/TR/selectors-4/#universal-selector)
    /// "The universal selector is a single asterisk (*) and represents the
    /// qualified name of any element type."
    Universal,
}

/// [§ 4.2 Compound selectors](https://www.w3.org/TR/selectors-4/#compound)
/// "A compound selector is a sequence of simple selectors that are not
/// separated by a combinator, and represents a set of simultaneous
/// conditions on a single element."
#[derive(Debug, Clone, PartialEq)]
pub struct CompoundSelector {
    pub simple_selectors: Vec<SimpleSelector>,
}

/// [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
/// "A selector's specificity is calculated for a given element as follows:
///  - count the number of ID selectors in the selector (= A)
///  - count the number of class selectors, attributes selectors, and pseudo-classes in the selector (= B)
///  - count the number of type selectors and pseudo-elements in the selector (= C)
/// Specificities are compared by comparing the three components in order."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl Specificity {
    /// Create a new specificity with (A, B, C) components.
    pub fn new(a: u32, b: u32, c: u32) -> Self {
        Specificity(a, b, c)
    }
}

/// A parsed CSS selector ready for matching.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSelector {
    pub compound: CompoundSelector,
    pub specificity: Specificity,
}

impl ParsedSelector {
    /// [§ 4.1 Selector Matching](https://www.w3.org/TR/selectors-4/#match-a-selector-against-an-element)
    /// "A selector is said to match an element when..."
    ///
    /// For a compound selector, all simple selectors must match.
    pub fn matches(&self, element: &ElementData) -> bool {
        self.compound
            .simple_selectors
            .iter()
            .all(|simple| simple.matches(element))
    }
}

impl SimpleSelector {
    /// Check if this simple selector matches the given element.
    pub fn matches(&self, element: &ElementData) -> bool {
        match self {
            // [§ 5.1 Type selector](https://www.w3.org/TR/selectors-4/#type-selectors)
            // "A type selector written in the style sheet as an identifier represents
            // an element in the document tree with the same qualified name as the identifier."
            SimpleSelector::Type(name) => element.tag_name.eq_ignore_ascii_case(name),

            // [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
            // "For documents that use the class attribute (which most do), authors
            // can use the 'period' (.) notation as an alternative."
            SimpleSelector::Class(class_name) => element.classes().contains(class_name.as_str()),

            // [§ 6.7 ID selector](https://www.w3.org/TR/selectors-4/#id-selectors)
            // "An ID selector represents an element instance that has an identifier
            // that matches the identifier in the ID selector."
            SimpleSelector::Id(id) => element.id().map_or(false, |el_id| el_id == id),

            // [§ 5.2 Universal selector](https://www.w3.org/TR/selectors-4/#universal-selector)
            // "The universal selector...represents the qualified name of any element type."
            SimpleSelector::Universal => true,
        }
    }
}

/// Check if a character can start an identifier.
/// [CSS Syntax Level 3 § 4.3.10](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
fn is_ident_start_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

/// Check if a character can continue an identifier.
/// [CSS Syntax Level 3 § 4.3.9](https://www.w3.org/TR/css-syntax-3/#ident-code-point)
fn is_ident_char(c: char) -> bool {
    is_ident_start_char(c) || c.is_ascii_digit() || c == '-'
}

/// Parse a raw selector string into a ParsedSelector.
///
/// [§ 4 Selector syntax](https://www.w3.org/TR/selectors-4/#syntax)
///
/// MVP supports: type selectors, .class, #id, and compound selectors.
/// Returns None for unsupported selectors (combinators, pseudo-classes, etc.).
pub fn parse_selector(raw: &str) -> Option<ParsedSelector> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut simple_selectors = Vec::new();
    let mut specificity = Specificity::default();

    let mut chars = trimmed.chars().peekable();
    let mut current = String::new();

    while let Some(c) = chars.next() {
        match c {
            // [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
            // "The class selector is given as a full stop (. U+002E)
            // immediately followed by an identifier."
            '.' => {
                // Flush any pending type selector
                if !current.is_empty() {
                    simple_selectors.push(SimpleSelector::Type(current.clone()));
                    specificity.2 += 1;
                    current.clear();
                }
                // Collect class name
                while chars.peek().map_or(false, |&c| is_ident_char(c)) {
                    current.push(chars.next().unwrap());
                }
                if !current.is_empty() {
                    simple_selectors.push(SimpleSelector::Class(current.clone()));
                    // [§ 17 Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
                    // "count the number of class selectors...in the selector (= B)"
                    specificity.1 += 1;
                    current.clear();
                }
            }

            // [§ 6.7 ID selector](https://www.w3.org/TR/selectors-4/#id-selectors)
            // "An ID selector is a hash (#, U+0023) immediately followed by the ID value"
            '#' => {
                // Flush any pending type selector
                if !current.is_empty() {
                    simple_selectors.push(SimpleSelector::Type(current.clone()));
                    specificity.2 += 1;
                    current.clear();
                }
                // Collect ID
                while chars.peek().map_or(false, |&c| is_ident_char(c)) {
                    current.push(chars.next().unwrap());
                }
                if !current.is_empty() {
                    simple_selectors.push(SimpleSelector::Id(current.clone()));
                    // [§ 17 Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
                    // "count the number of ID selectors in the selector (= A)"
                    specificity.0 += 1;
                    current.clear();
                }
            }

            // [§ 5.2 Universal selector](https://www.w3.org/TR/selectors-4/#universal-selector)
            '*' => {
                // Flush any pending type selector (shouldn't happen in valid CSS)
                if !current.is_empty() {
                    simple_selectors.push(SimpleSelector::Type(current.clone()));
                    specificity.2 += 1;
                    current.clear();
                }
                simple_selectors.push(SimpleSelector::Universal);
                // Universal selector has (0,0,0) specificity - no addition needed
            }

            // [§ 4.1 Selector syntax](https://www.w3.org/TR/selectors-4/#syntax)
            // Combinators - not supported in MVP
            ' ' | '>' | '+' | '~' => {
                // Skip leading/trailing whitespace
                if current.is_empty() && simple_selectors.is_empty() {
                    continue;
                }
                // If we have content, this is a combinator - not supported
                if !current.is_empty() || !simple_selectors.is_empty() {
                    // Check if it's just trailing whitespace
                    let remaining: String = chars.collect();
                    if remaining.trim().is_empty() {
                        // Just trailing whitespace, flush and return
                        if !current.is_empty() {
                            simple_selectors.push(SimpleSelector::Type(current.clone()));
                            specificity.2 += 1;
                            current.clear();
                        }
                        break;
                    }
                    // Actual combinator - not supported in MVP
                    return None;
                }
            }

            // Identifier characters - part of type selector
            // [CSS Syntax § 4.3.9-10](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
            // First character must be an ident-start char or '-', continuation can be ident chars
            _ if current.is_empty() && (is_ident_start_char(c) || c == '-') => {
                current.push(c);
            }
            _ if !current.is_empty() && is_ident_char(c) => {
                current.push(c);
            }

            // Unknown character - skip this selector
            _ => {
                return None;
            }
        }
    }

    // Flush remaining type selector
    if !current.is_empty() {
        // [§ 17 Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
        // "count the number of type selectors...in the selector (= C)"
        simple_selectors.push(SimpleSelector::Type(current));
        specificity.2 += 1;
    }

    if simple_selectors.is_empty() {
        return None;
    }

    Some(ParsedSelector {
        compound: CompoundSelector { simple_selectors },
        specificity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_element(tag: &str, id: Option<&str>, classes: &[&str]) -> ElementData {
        let mut attrs = HashMap::new();
        if let Some(id_val) = id {
            attrs.insert("id".to_string(), id_val.to_string());
        }
        if !classes.is_empty() {
            attrs.insert("class".to_string(), classes.join(" "));
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
        assert_eq!(selector.compound.simple_selectors.len(), 1);
        assert!(matches!(
            &selector.compound.simple_selectors[0],
            SimpleSelector::Type(name) if name == "body"
        ));
    }

    #[test]
    fn test_parse_class_selector() {
        let selector = parse_selector(".highlight").unwrap();
        assert_eq!(selector.specificity, Specificity(0, 1, 0));
        assert!(matches!(
            &selector.compound.simple_selectors[0],
            SimpleSelector::Class(name) if name == "highlight"
        ));
    }

    #[test]
    fn test_parse_id_selector() {
        let selector = parse_selector("#main-content").unwrap();
        assert_eq!(selector.specificity, Specificity(1, 0, 0));
        assert!(matches!(
            &selector.compound.simple_selectors[0],
            SimpleSelector::Id(name) if name == "main-content"
        ));
    }

    #[test]
    fn test_parse_universal_selector() {
        let selector = parse_selector("*").unwrap();
        assert_eq!(selector.specificity, Specificity(0, 0, 0));
        assert!(matches!(
            &selector.compound.simple_selectors[0],
            SimpleSelector::Universal
        ));
    }

    #[test]
    fn test_parse_compound_selector() {
        // div.highlight#main
        let selector = parse_selector("div.highlight#main").unwrap();
        assert_eq!(selector.specificity, Specificity(1, 1, 1));
        assert_eq!(selector.compound.simple_selectors.len(), 3);
    }

    #[test]
    fn test_parse_descendant_combinator_not_supported() {
        // Descendant combinator not supported in MVP
        assert!(parse_selector("div p").is_none());
    }

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

    #[test]
    fn test_specificity_ordering() {
        // ID > class > type
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
}
