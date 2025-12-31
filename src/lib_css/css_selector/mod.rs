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

/// [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)
///
/// "A combinator is punctuation that represents a particular kind of
/// relationship between the selectors on either side."
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combinator {
    /// [§ 16.1 Descendant combinator](https://www.w3.org/TR/selectors-4/#descendant-combinators)
    /// "A descendant combinator is whitespace that separates two compound selectors.
    /// A selector of the form 'A B' represents an element B that is an arbitrary
    /// descendant of some ancestor element A."
    Descendant,

    /// [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
    /// "A child combinator is a greater-than sign (>) that separates two compound
    /// selectors. A selector of the form 'A > B' represents an element B that is
    /// a direct child of element A."
    Child,

    /// [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
    /// "A next-sibling combinator is a plus sign (+) that separates two compound
    /// selectors. A selector of the form 'A + B' represents an element B that
    /// immediately follows element A, where A and B share the same parent."
    NextSibling,

    /// [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
    /// "A subsequent-sibling combinator is a tilde (~) that separates two compound
    /// selectors. A selector of the form 'A ~ B' represents an element B that
    /// follows element A (not necessarily immediately), where A and B share the
    /// same parent."
    SubsequentSibling,
}

/// [§ 4.3 Complex selectors](https://www.w3.org/TR/selectors-4/#complex)
///
/// "A complex selector is a chain of one or more compound selectors separated
/// by combinators. It represents a set of simultaneous conditions on a set of
/// elements in the particular relationships described by its combinators."
///
/// Example: `div.container > ul.nav li a.active`
/// This would be parsed as:
/// ```text
/// [div.container] --(Child)--> [ul.nav] --(Descendant)--> [li] --(Descendant)--> [a.active]
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSelector {
    /// The rightmost compound selector (the subject of the selector).
    /// [§ 4.3](https://www.w3.org/TR/selectors-4/#complex)
    /// "The elements represented by a complex selector are the elements matched
    /// by the last compound selector in the complex selector."
    pub subject: CompoundSelector,

    /// Chain of (combinator, compound_selector) pairs going left from the subject.
    /// Empty if this is a simple compound selector with no combinators.
    ///
    /// For `A > B C`, this would be:
    /// - subject: C
    /// - combinators: [(Descendant, B), (Child, A)]
    ///
    /// The order is right-to-left because matching is done from the subject upward.
    pub combinators: Vec<(Combinator, CompoundSelector)>,
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
    pub complex: ComplexSelector,
    pub specificity: Specificity,
}

impl ParsedSelector {
    /// Check if this is a simple selector (no combinators).
    pub fn is_simple(&self) -> bool {
        self.complex.combinators.is_empty()
    }

    /// [§ 4.1 Selector Matching](https://www.w3.org/TR/selectors-4/#match-a-selector-against-an-element)
    /// "A selector is said to match an element when..."
    ///
    /// For simple selectors (no combinators), all simple selectors in the
    /// compound must match the element directly.
    ///
    /// For complex selectors (with combinators), we need DOM tree traversal
    /// to check ancestor/sibling relationships.
    pub fn matches(&self, element: &ElementData) -> bool {
        // First, the subject (rightmost compound) must match the element
        let subject_matches = self
            .complex
            .subject
            .simple_selectors
            .iter()
            .all(|simple| simple.matches(element));

        if !subject_matches {
            return false;
        }

        // If there are no combinators, we're done
        if self.complex.combinators.is_empty() {
            return true;
        }

        // Complex selectors require DOM traversal
        self.matches_with_context(element)
    }

    /// [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)
    ///
    /// Match a complex selector by traversing the DOM tree according to
    /// combinator relationships.
    ///
    /// # Arguments
    /// * `element` - The element to match (already matched the subject)
    ///
    /// # Returns
    /// `true` if all combinator relationships are satisfied
    ///
    /// # Note
    /// This requires access to the DOM tree (parent/sibling pointers) which
    /// is not available from ElementData alone. The current implementation
    /// will panic until DOM traversal is implemented.
    fn matches_with_context(&self, _element: &ElementData) -> bool {
        // [§ 16.1 Descendant combinator](https://www.w3.org/TR/selectors-4/#descendant-combinators)
        // "A selector of the form 'A B' represents an element B that is an
        // arbitrary descendant of some ancestor element A."
        //
        // [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
        // "A selector of the form 'A > B' represents an element B that is a
        // direct child of element A."
        //
        // [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
        // "A selector of the form 'A + B' represents an element B that
        // immediately follows element A, where A and B share the same parent."
        //
        // [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
        // "A selector of the form 'A ~ B' represents an element B that follows
        // element A (not necessarily immediately), where A and B share the
        // same parent."
        //
        // To implement these, we need:
        // 1. Parent pointer to walk up the tree (for Descendant, Child)
        // 2. Previous sibling pointer (for NextSibling, SubsequentSibling)
        // 3. Access to parent's children list (for sibling combinators)
        todo!(
            "Complex selector matching requires DOM tree traversal. \
             Need to implement parent/sibling access in lib_dom. \
             Combinators: {:?}",
            self.complex.combinators
        )
    }
}

impl ComplexSelector {
    /// [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
    ///
    /// Calculate specificity for the entire complex selector by summing
    /// specificity of all compound selectors in the chain.
    pub fn calculate_specificity(&self) -> Specificity {
        let mut spec = calculate_compound_specificity(&self.subject);

        for (_, compound) in &self.combinators {
            let compound_spec = calculate_compound_specificity(compound);
            spec.0 += compound_spec.0;
            spec.1 += compound_spec.1;
            spec.2 += compound_spec.2;
        }

        spec
    }
}

/// Calculate specificity for a single compound selector.
fn calculate_compound_specificity(compound: &CompoundSelector) -> Specificity {
    let mut spec = Specificity::default();

    for simple in &compound.simple_selectors {
        match simple {
            // [§ 17](https://www.w3.org/TR/selectors-4/#specificity-rules)
            // "count the number of ID selectors in the selector (= A)"
            SimpleSelector::Id(_) => spec.0 += 1,

            // "count the number of class selectors, attributes selectors,
            // and pseudo-classes in the selector (= B)"
            SimpleSelector::Class(_) => spec.1 += 1,

            // "count the number of type selectors and pseudo-elements
            // in the selector (= C)"
            SimpleSelector::Type(_) => spec.2 += 1,

            // "ignore the universal selector"
            SimpleSelector::Universal => {}
        }
    }

    spec
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
/// "The grammar of Selectors is defined in terms of CSS syntax."
///
/// Supports:
/// - Type selectors: `div`, `p`, `span`
/// - Class selectors: `.class`
/// - ID selectors: `#id`
/// - Universal selector: `*`
/// - Compound selectors: `div.class#id`
/// - Complex selectors with combinators: `div p`, `ul > li`, `h1 + p`, `h1 ~ p`
///
/// Returns None for unsupported selectors (pseudo-classes, attribute selectors, etc.).
pub fn parse_selector(raw: &str) -> Option<ParsedSelector> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // [§ 4.3 Complex selectors](https://www.w3.org/TR/selectors-4/#complex)
    // "A complex selector is a chain of one or more compound selectors
    // separated by combinators."
    //
    // We parse left-to-right, collecting compound selectors and the
    // combinators between them. At the end, we reverse so the rightmost
    // (subject) is easily accessible.

    let mut compounds: Vec<CompoundSelector> = Vec::new();
    let mut combinators_between: Vec<Combinator> = Vec::new();

    let mut chars = trimmed.chars().peekable();
    let mut current_compound = Vec::new();
    let mut current_ident = String::new();

    /// Flush the current identifier as a type selector into the compound.
    fn flush_ident(ident: &mut String, compound: &mut Vec<SimpleSelector>) {
        if !ident.is_empty() {
            compound.push(SimpleSelector::Type(ident.clone()));
            ident.clear();
        }
    }

    /// Flush current compound selector into the compounds list.
    /// Returns true if a non-empty compound was flushed.
    fn flush_compound(
        ident: &mut String,
        compound: &mut Vec<SimpleSelector>,
        compounds: &mut Vec<CompoundSelector>,
    ) -> bool {
        flush_ident(ident, compound);
        if compound.is_empty() {
            return false;
        }
        compounds.push(CompoundSelector {
            simple_selectors: std::mem::take(compound),
        });
        true
    }

    while let Some(c) = chars.next() {
        match c {
            // [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
            // "The class selector is given as a full stop (. U+002E)
            // immediately followed by an identifier."
            '.' => {
                flush_ident(&mut current_ident, &mut current_compound);
                // Collect class name
                while chars.peek().map_or(false, |&ch| is_ident_char(ch)) {
                    current_ident.push(chars.next().unwrap());
                }
                if !current_ident.is_empty() {
                    current_compound.push(SimpleSelector::Class(current_ident.clone()));
                    current_ident.clear();
                }
            }

            // [§ 6.7 ID selector](https://www.w3.org/TR/selectors-4/#id-selectors)
            // "An ID selector is a hash (#, U+0023) immediately followed by the ID value"
            '#' => {
                flush_ident(&mut current_ident, &mut current_compound);
                // Collect ID
                while chars.peek().map_or(false, |&ch| is_ident_char(ch)) {
                    current_ident.push(chars.next().unwrap());
                }
                if !current_ident.is_empty() {
                    current_compound.push(SimpleSelector::Id(current_ident.clone()));
                    current_ident.clear();
                }
            }

            // [§ 5.2 Universal selector](https://www.w3.org/TR/selectors-4/#universal-selector)
            // "The universal selector is a single asterisk (*) and represents the
            // qualified name of any element type."
            '*' => {
                flush_ident(&mut current_ident, &mut current_compound);
                current_compound.push(SimpleSelector::Universal);
                // Universal selector has (0,0,0) specificity - no addition needed
            }

            // [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)
            // "A combinator is punctuation that represents a particular kind of
            // relationship between the selectors on either side."
            //
            // Whitespace characters may be the descendant combinator, but we need
            // to check for explicit combinators (>, +, ~) that might follow.
            ' ' | '\t' | '\n' | '\r' => {
                // Skip leading whitespace
                if current_ident.is_empty() && current_compound.is_empty() && compounds.is_empty()
                {
                    continue;
                }

                // Consume all contiguous whitespace
                while chars.peek().map_or(false, |&ch| ch.is_ascii_whitespace()) {
                    chars.next();
                }

                // Check what follows the whitespace
                match chars.peek() {
                    // End of selector - just trailing whitespace
                    None => {
                        flush_compound(&mut current_ident, &mut current_compound, &mut compounds);
                    }

                    // [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
                    // [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
                    // [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
                    // Explicit combinator follows - just flush the identifier into the compound,
                    // but don't flush the compound itself. The combinator case will handle that.
                    Some('>') | Some('+') | Some('~') => {
                        flush_ident(&mut current_ident, &mut current_compound);
                    }

                    // [§ 16.1 Descendant combinator](https://www.w3.org/TR/selectors-4/#descendant-combinators)
                    // "A descendant combinator is whitespace that separates two compound selectors.
                    // A selector of the form 'A B' represents an element B that is an arbitrary
                    // descendant of some ancestor element A."
                    Some(_) => {
                        if !flush_compound(
                            &mut current_ident,
                            &mut current_compound,
                            &mut compounds,
                        ) {
                            continue; // No left-hand compound yet, skip
                        }
                        combinators_between.push(Combinator::Descendant);
                    }
                }
            }

            // [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
            // "A child combinator is a greater-than sign (>) that separates two compound
            // selectors. A selector of the form 'A > B' represents an element B that is
            // a direct child of element A."
            '>' => {
                if !flush_compound(&mut current_ident, &mut current_compound, &mut compounds) {
                    return None; // Invalid: > without left-hand side
                }
                // Skip whitespace after combinator
                while chars.peek().map_or(false, |&ch| ch.is_ascii_whitespace()) {
                    chars.next();
                }
                combinators_between.push(Combinator::Child);
            }

            // [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
            // "A next-sibling combinator is a plus sign (+) that separates two compound
            // selectors. A selector of the form 'A + B' represents an element B that
            // immediately follows element A, where A and B share the same parent."
            '+' => {
                if !flush_compound(&mut current_ident, &mut current_compound, &mut compounds) {
                    return None; // Invalid: + without left-hand side
                }
                // Skip whitespace after combinator
                while chars.peek().map_or(false, |&ch| ch.is_ascii_whitespace()) {
                    chars.next();
                }
                combinators_between.push(Combinator::NextSibling);
            }

            // [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
            // "A subsequent-sibling combinator is a tilde (~) that separates two compound
            // selectors. A selector of the form 'A ~ B' represents an element B that
            // follows element A (not necessarily immediately), where A and B share the
            // same parent."
            '~' => {
                if !flush_compound(&mut current_ident, &mut current_compound, &mut compounds) {
                    return None; // Invalid: ~ without left-hand side
                }
                // Skip whitespace after combinator
                while chars.peek().map_or(false, |&ch| ch.is_ascii_whitespace()) {
                    chars.next();
                }
                combinators_between.push(Combinator::SubsequentSibling);
            }

            // Identifier characters - part of type selector
            // [CSS Syntax § 4.3.9-10](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
            // "An ident-start code point is a letter, a non-ASCII code point, or U+005F LOW LINE (_)."
            // First character must be an ident-start char or '-', continuation can be ident chars.
            _ if current_ident.is_empty() && (is_ident_start_char(c) || c == '-') => {
                current_ident.push(c);
            }
            // [CSS Syntax § 4.3.9](https://www.w3.org/TR/css-syntax-3/#ident-code-point)
            // "An ident code point is an ident-start code point, a digit, or U+002D HYPHEN-MINUS (-)."
            _ if !current_ident.is_empty() && is_ident_char(c) => {
                current_ident.push(c);
            }

            // Unknown character - unsupported selector syntax
            _ => {
                return None;
            }
        }
    }

    // Flush final compound selector
    // [§ 17 Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
    // "count the number of type selectors...in the selector (= C)"
    flush_compound(&mut current_ident, &mut current_compound, &mut compounds);

    if compounds.is_empty() {
        return None;
    }

    // Validate: we should have exactly one more compound than combinators
    // For "A B C" we have 3 compounds and 2 combinators
    if compounds.len() != combinators_between.len() + 1 {
        return None;
    }

    // [§ 4.3 Complex selectors](https://www.w3.org/TR/selectors-4/#complex)
    // "The elements represented by a complex selector are the elements matched
    // by the last compound selector in the complex selector."
    //
    // Build the ComplexSelector with the rightmost compound as subject
    let subject = compounds.pop().unwrap();

    // Build the combinator chain in right-to-left order for matching
    // For "A > B C" we have compounds: [A, B, C] and combinators: [Child, Descendant]
    // After popping subject (C), compounds: [A, B], combinators: [Child, Descendant]
    // We want: [(Descendant, B), (Child, A)] so matching walks up from subject
    let mut combinator_chain = Vec::new();
    for (compound, combinator) in compounds
        .into_iter()
        .zip(combinators_between.into_iter())
        .rev()
    {
        combinator_chain.push((combinator, compound));
    }

    let complex = ComplexSelector {
        subject,
        combinators: combinator_chain,
    };

    // [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
    // Calculate specificity by summing all simple selectors in the complex selector
    let specificity = complex.calculate_specificity();

    Some(ParsedSelector {
        complex,
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

    // =========================================================================
    // Combinator Parsing Tests
    // [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)
    // =========================================================================

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
        assert_eq!(selector.complex.combinators[0].0, Combinator::SubsequentSibling);

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

    // =========================================================================
    // Simple Selector Matching Tests
    // =========================================================================

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

    // =========================================================================
    // Specificity Tests
    // [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
    // =========================================================================

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
}
