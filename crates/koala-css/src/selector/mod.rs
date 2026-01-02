//! CSS Selector parsing and matching
//!
//! This module implements selector parsing and matching per
//! [Selectors Level 4](https://www.w3.org/TR/selectors-4/).

use koala_dom::{DomTree, ElementData, NodeId};

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
    /// The list of simple selectors that make up this compound selector.
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
    /// The complex selector (compound selectors with combinators).
    pub complex: ComplexSelector,
    /// The specificity of this selector.
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
    /// NOTE: This method only works for simple selectors. For complex selectors
    /// with combinators, use `matches_in_tree` which has access to DOM context.
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

        // Complex selectors require DOM traversal - for now, return false
        // since we don't have tree context. Callers should use matches_in_tree.
        false
    }

    /// [§ 4.1 Selector Matching](https://www.w3.org/TR/selectors-4/#match-a-selector-against-an-element)
    /// "A selector is said to match an element when..."
    ///
    /// Match a selector against an element with full DOM tree context.
    /// This enables matching complex selectors with combinators.
    ///
    /// # Arguments
    /// * `tree` - The DOM tree containing the element
    /// * `node_id` - The NodeId of the element to match
    ///
    /// # Returns
    /// `true` if the selector matches the element
    pub fn matches_in_tree(&self, tree: &DomTree, node_id: NodeId) -> bool {
        // Get the element data for this node
        let Some(element) = tree.as_element(node_id) else {
            return false;
        };

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
        self.matches_combinators(tree, node_id)
    }

    /// [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)
    ///
    /// Match a complex selector by traversing the DOM tree according to
    /// combinator relationships.
    ///
    /// The combinator chain is stored in right-to-left order (from subject outward),
    /// so we walk the chain and for each combinator, find a matching element in
    /// the appropriate relationship (ancestor, parent, previous sibling, etc.).
    ///
    /// # Arguments
    /// * `tree` - The DOM tree
    /// * `subject_id` - The NodeId of the subject element (already matched)
    ///
    /// # Returns
    /// `true` if all combinator relationships are satisfied
    fn matches_combinators(&self, tree: &DomTree, subject_id: NodeId) -> bool {
        let mut current_id = subject_id;

        // Walk the combinator chain (right-to-left, from subject outward)
        for (combinator, compound) in &self.complex.combinators {
            match combinator {
                // [§ 16.1 Descendant combinator](https://www.w3.org/TR/selectors-4/#descendant-combinators)
                // "A selector of the form 'A B' represents an element B that is an
                // arbitrary descendant of some ancestor element A."
                Combinator::Descendant => {
                    // Find any ancestor that matches the compound selector
                    let matched_ancestor = tree.ancestors(current_id).find(|&ancestor_id| {
                        if let Some(ancestor_elem) = tree.as_element(ancestor_id) {
                            compound_matches(compound, ancestor_elem)
                        } else {
                            false
                        }
                    });

                    match matched_ancestor {
                        Some(ancestor_id) => current_id = ancestor_id,
                        None => return false,
                    }
                }

                // [§ 16.2 Child combinator](https://www.w3.org/TR/selectors-4/#child-combinators)
                // "A selector of the form 'A > B' represents an element B that is a
                // direct child of element A."
                Combinator::Child => {
                    // The immediate parent must match
                    let Some(parent_id) = tree.parent(current_id) else {
                        return false;
                    };

                    let Some(parent_elem) = tree.as_element(parent_id) else {
                        return false;
                    };

                    if !compound_matches(compound, parent_elem) {
                        return false;
                    }

                    current_id = parent_id;
                }

                // [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
                // "A next-sibling combinator is a plus sign (+) that separates two compound
                // selectors. A selector of the form 'A + B' represents an element B that
                // immediately follows element A, where A and B share the same parent."
                Combinator::NextSibling => {
                    // Find the immediately preceding element sibling
                    let prev_element = find_previous_element_sibling(tree, current_id);

                    let Some(prev_id) = prev_element else {
                        return false;
                    };

                    let Some(prev_elem) = tree.as_element(prev_id) else {
                        return false;
                    };

                    if !compound_matches(compound, prev_elem) {
                        return false;
                    }

                    current_id = prev_id;
                }

                // [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
                // "A subsequent-sibling combinator is a tilde (~) that separates two compound
                // selectors. A selector of the form 'A ~ B' represents an element B that
                // follows element A (not necessarily immediately), where A and B share the
                // same parent."
                Combinator::SubsequentSibling => {
                    // Find any preceding element sibling that matches
                    let matched_sibling =
                        find_matching_preceding_sibling(tree, current_id, compound);

                    match matched_sibling {
                        Some(sibling_id) => current_id = sibling_id,
                        None => return false,
                    }
                }
            }
        }

        // All combinators matched
        true
    }
}

/// Check if a compound selector matches an element.
fn compound_matches(compound: &CompoundSelector, element: &ElementData) -> bool {
    compound
        .simple_selectors
        .iter()
        .all(|simple| simple.matches(element))
}

/// [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
///
/// Find the immediately preceding element sibling (skipping text/comment nodes).
/// Per spec, the next-sibling combinator considers element nodes only.
fn find_previous_element_sibling(tree: &DomTree, node_id: NodeId) -> Option<NodeId> {
    // Walk backwards through preceding siblings until we find an element
    for sibling_id in tree.preceding_siblings(node_id) {
        if tree.as_element(sibling_id).is_some() {
            return Some(sibling_id);
        }
    }
    None
}

/// [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
///
/// Find any preceding element sibling that matches the compound selector.
fn find_matching_preceding_sibling(
    tree: &DomTree,
    node_id: NodeId,
    compound: &CompoundSelector,
) -> Option<NodeId> {
    for sibling_id in tree.preceding_siblings(node_id) {
        if let Some(sibling_elem) = tree.as_element(sibling_id) {
            if compound_matches(compound, sibling_elem) {
                return Some(sibling_id);
            }
        }
    }
    None
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
/// [§ 4.3.10 ident-start code point](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
fn is_ident_start_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

/// Check if a character can continue an identifier.
/// [§ 4.3.9 ident code point](https://www.w3.org/TR/css-syntax-3/#ident-code-point)
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
                if current_ident.is_empty() && current_compound.is_empty() && compounds.is_empty() {
                    continue;
                }

                // Consume all contiguous whitespace
                while chars.peek().map_or(false, |&ch| ch.is_ascii_whitespace()) {
                    let _ = chars.next();
                }

                // Check what follows the whitespace
                match chars.peek() {
                    // End of selector - just trailing whitespace
                    None => {
                        let _ = flush_compound(
                            &mut current_ident,
                            &mut current_compound,
                            &mut compounds,
                        );
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
                    let _ = chars.next();
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
                    let _ = chars.next();
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
                    let _ = chars.next();
                }
                combinators_between.push(Combinator::SubsequentSibling);
            }

            // Identifier characters - part of type selector
            // [§ 4.3.9-10 ident code points](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
            // "An ident-start code point is a letter, a non-ASCII code point, or U+005F LOW LINE (_)."
            // First character must be an ident-start char or '-', continuation can be ident chars.
            _ if current_ident.is_empty() && (is_ident_start_char(c) || c == '-') => {
                current_ident.push(c);
            }
            // [§ 4.3.9 ident code point](https://www.w3.org/TR/css-syntax-3/#ident-code-point)
            // "An ident code point is an ident-start code point, a digit, or U+002D HYPHEN-MINUS (-)."
            _ if !current_ident.is_empty() && is_ident_char(c) => {
                current_ident.push(c);
            }

            // [§ 4 Pseudo-classes](https://www.w3.org/TR/selectors-4/#pseudo-classes)
            // TODO: Implement pseudo-class selectors (:hover, :active, :focus, :link, :visited, etc.)
            //
            // [§ 11 Pseudo-elements](https://www.w3.org/TR/selectors-4/#pseudo-elements)
            // TODO: Implement pseudo-element selectors (::before, ::after, etc.)
            ':' => {
                // Pseudo-classes and pseudo-elements are not yet implemented
                return None;
            }

            // [§ 6.4 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
            // TODO: Implement attribute selectors ([attr], [attr=value], etc.)
            '[' => {
                return None;
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
    let _ = flush_compound(&mut current_ident, &mut current_compound, &mut compounds);

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
