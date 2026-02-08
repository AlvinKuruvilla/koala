//! CSS Selector parsing and matching
//!
//! This module implements selector parsing and matching per
//! [Selectors Level 4](https://www.w3.org/TR/selectors-4/).

use koala_dom::{DomTree, ElementData, NodeId, NodeType};

/// [§ 5 Elemental selectors](https://www.w3.org/TR/selectors-4/#elemental-selectors)
/// [§ 6 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
///
/// A simple selector is a single condition on an element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleSelector {
    /// [§ 5.1 Type selector](https://www.w3.org/TR/selectors-4/#type-selectors)
    /// "A type selector is the name of a document language element type,
    /// and represents an instance of that element type in the document tree."
    ///
    /// Examples: `div`, `p`, `span`, `body`, `h1`
    Type(String),

    /// [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
    /// "The class selector is given as a full stop (. U+002E) immediately
    /// followed by an identifier."
    ///
    /// Examples: `.highlight`, `.btn`, `.nav-item`
    Class(String),

    /// [§ 6.7 ID selector](https://www.w3.org/TR/selectors-4/#id-selectors)
    /// "An ID selector is a hash (#, U+0023) immediately followed by the
    /// ID value, which is an identifier."
    ///
    /// Examples: `#main`, `#header`, `#nav-bar`
    Id(String),

    /// [§ 5.2 Universal selector](https://www.w3.org/TR/selectors-4/#universal-selector)
    /// "The universal selector is a single asterisk (*) and represents the
    /// qualified name of any element type."
    ///
    /// Example: `*`
    Universal,

    /// Pseudo-class or pseudo-element that always fails to match.
    /// Used for interactive states (`:hover`, `:focus`, `:active`, `:visited`, etc.)
    /// and pseudo-elements (`::before`, `::after`, etc.) that are irrelevant
    /// to static rendering but whose presence should not cause the entire
    /// rule to be dropped.
    ///
    /// Examples: `:hover`, `:focus`, `:active`, `:visited`, `::before`, `::after`,
    /// `::placeholder`, `:nth-child(2)`, `:not(.foo)`
    NeverMatch,

    /// [§ 4 Pseudo-classes](https://www.w3.org/TR/selectors-4/#pseudo-classes)
    /// Structural pseudo-class that requires DOM tree context to match.
    ///
    /// Examples: `:root`, `:first-child`, `:last-child`, `:empty`, `:only-child`,
    /// `:first-of-type`, `:last-of-type`, `:link`, `:enabled`, `:disabled`
    PseudoClass(PseudoClass),

    /// [§ 6.4 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
    /// Attribute selector matching based on element attributes.
    ///
    /// Examples: `[href]`, `[type=text]`, `[class~=active]`, `[lang|=en]`,
    /// `[href^=https]`, `[src$=".png"]`, `[data-theme*=dark]`
    Attribute(AttributeSelector),
}

/// Structural pseudo-classes per [§ 4 Pseudo-classes](https://www.w3.org/TR/selectors-4/#pseudo-classes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoClass {
    /// [§ 4.4 :root](https://www.w3.org/TR/selectors-4/#the-root-pseudo)
    /// "The :root pseudo-class represents an element that is the root of the document."
    ///
    /// Example: `:root { --main-color: blue; }` — matches the `<html>` element
    Root,

    /// [§ 4.12 :first-child](https://www.w3.org/TR/selectors-4/#the-first-child-pseudo)
    /// "The :first-child pseudo-class represents an element that is first among its
    /// inclusive siblings."
    ///
    /// Example: `li:first-child` — matches `<li>A</li>` in `<ul><li>A</li><li>B</li></ul>`
    FirstChild,

    /// [§ 4.12 :last-child](https://www.w3.org/TR/selectors-4/#the-last-child-pseudo)
    /// "The :last-child pseudo-class represents an element that is last among its
    /// inclusive siblings."
    ///
    /// Example: `li:last-child` — matches `<li>B</li>` in `<ul><li>A</li><li>B</li></ul>`
    LastChild,

    /// [§ 4.11 :first-of-type](https://www.w3.org/TR/selectors-4/#the-first-of-type-pseudo)
    /// "The :first-of-type pseudo-class represents an element that is the first sibling
    /// of its type."
    ///
    /// Example: `p:first-of-type` — matches the first `<p>` even if preceded by a `<div>`
    FirstOfType,

    /// [§ 4.11 :last-of-type](https://www.w3.org/TR/selectors-4/#the-last-of-type-pseudo)
    /// "The :last-of-type pseudo-class represents an element that is the last sibling
    /// of its type."
    ///
    /// Example: `p:last-of-type` — matches the last `<p>` among its siblings
    LastOfType,

    /// [§ 4.12 :only-child](https://www.w3.org/TR/selectors-4/#the-only-child-pseudo)
    /// "The :only-child pseudo-class represents an element that has no siblings."
    ///
    /// Example: `p:only-child` — matches `<p>` in `<div><p>alone</p></div>`
    OnlyChild,

    /// [§ 4.5 :empty](https://www.w3.org/TR/selectors-4/#the-empty-pseudo)
    /// "The :empty pseudo-class represents an element that has no children at all."
    ///
    /// Example: `div:empty` — matches `<div></div>` but not `<div>text</div>`
    Empty,

    /// [§ 4.6 :link](https://www.w3.org/TR/selectors-4/#the-link-pseudo)
    /// "The :link pseudo-class applies to links that have not yet been visited."
    /// In static rendering, all links are treated as unvisited.
    ///
    /// Example: `a:link` — matches `<a href="...">` but not `<a>` without href
    Link,

    /// :disabled — form element with disabled attribute
    ///
    /// Example: `input:disabled` — matches `<input disabled>`
    Disabled,

    /// :enabled — form element without disabled attribute
    ///
    /// Example: `input:enabled` — matches `<input>` (no disabled attribute)
    Enabled,
}

/// Attribute selectors per [§ 6.4](https://www.w3.org/TR/selectors-4/#attribute-selectors)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeSelector {
    /// [§ 6.4] [attr] — "Represents an element with the att attribute"
    ///
    /// Example: `[href]` — matches any element that has an `href` attribute
    Exists(String),

    /// [§ 6.4] [attr=value] — "Represents an element with the att attribute whose value
    /// is exactly 'val'."
    ///
    /// Example: `[type="text"]` — matches `<input type="text">` but not `<input type="password">`
    Equals(String, String),

    /// [§ 6.4] [attr~=value] — "Represents an element with the att attribute whose value
    /// is a whitespace-separated list of words, one of which is exactly 'val'."
    ///
    /// Example: `[class~="active"]` — matches `<div class="btn active">` (word "active" present)
    Includes(String, String),

    /// [§ 6.4] [attr|=value] — "Represents an element with the att attribute, its value
    /// either being exactly 'val' or beginning with 'val' immediately followed by '-'."
    ///
    /// Example: `[lang|="en"]` — matches `<p lang="en">` and `<p lang="en-US">`
    DashMatch(String, String),

    /// [§ 6.4] [attr^=value] — "Represents an element with the att attribute whose value
    /// begins with the prefix 'val'."
    ///
    /// Example: `[href^="https"]` — matches `<a href="https://example.com">`
    PrefixMatch(String, String),

    /// [§ 6.4] [attr$=value] — "Represents an element with the att attribute whose value
    /// ends with the suffix 'val'."
    ///
    /// Example: `[src$=".png"]` — matches `<img src="photo.png">`
    SuffixMatch(String, String),

    /// [§ 6.4] [attr*=value] — "Represents an element with the att attribute whose value
    /// contains at least one instance of the substring 'val'."
    ///
    /// Example: `[data-theme*="dark"]` — matches `<div data-theme="my-dark-mode">`
    SubstringMatch(String, String),
}

/// [§ 4.2 Compound selectors](https://www.w3.org/TR/selectors-4/#compound)
///
/// "A compound selector is a sequence of simple selectors that are not
/// separated by a combinator, and represents a set of simultaneous
/// conditions on a single element."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundSelector {
    /// The list of simple selectors that make up this compound selector.
    pub simple_selectors: Vec<SimpleSelector>,
}

/// [§ 16 Combinators](https://www.w3.org/TR/selectors-4/#combinators)
///
/// "A combinator is punctuation that represents a particular kind of
/// relationship between the selectors on either side."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexSelector {
    /// The rightmost compound selector (the subject of the selector).
    /// [§ 4.3](https://www.w3.org/TR/selectors-4/#complex)
    /// "The elements represented by a complex selector are the elements matched
    /// by the last compound selector in the complex selector."
    pub subject: CompoundSelector,

    /// Chain of (combinator, `compound_selector`) pairs going left from the subject.
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
///
/// Specificities are compared by comparing the three components in order."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl Specificity {
    /// Create a new specificity with (A, B, C) components.
    #[must_use]
    pub const fn new(a: u32, b: u32, c: u32) -> Self {
        Self(a, b, c)
    }
}

/// A parsed CSS selector ready for matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSelector {
    /// The complex selector (compound selectors with combinators).
    pub complex: ComplexSelector,
    /// The specificity of this selector.
    pub specificity: Specificity,
}

impl ParsedSelector {
    /// Check if this is a simple selector (no combinators).
    #[must_use]
    pub const fn is_simple(&self) -> bool {
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
    #[must_use]
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
    /// * `node_id` - The `NodeId` of the element to match
    ///
    /// # Returns
    /// `true` if the selector matches the element
    #[must_use]
    pub fn matches_in_tree(&self, tree: &DomTree, node_id: NodeId) -> bool {
        // First, the subject (rightmost compound) must match the element
        if !compound_matches_in_tree(&self.complex.subject, tree, node_id) {
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
    /// * `subject_id` - The `NodeId` of the subject element (already matched)
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
                    let matched_ancestor = tree
                        .ancestors(current_id)
                        .find(|&ancestor_id| compound_matches_in_tree(compound, tree, ancestor_id));

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

                    if !compound_matches_in_tree(compound, tree, parent_id) {
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
                    let Some(prev_id) = find_previous_element_sibling(tree, current_id) else {
                        return false;
                    };

                    if !compound_matches_in_tree(compound, tree, prev_id) {
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

/// Check if a compound selector matches an element, with optional tree context
/// for structural pseudo-class matching.
fn compound_matches_in_tree(compound: &CompoundSelector, tree: &DomTree, node_id: NodeId) -> bool {
    let Some(element) = tree.as_element(node_id) else {
        return false;
    };
    compound.simple_selectors.iter().all(|simple| match simple {
        SimpleSelector::PseudoClass(pc) => pseudo_class_matches(pc, tree, node_id, element),
        _ => simple.matches(element),
    })
}

/// [§ 4 Pseudo-classes](https://www.w3.org/TR/selectors-4/#pseudo-classes)
///
/// Match a structural pseudo-class against an element with full DOM tree context.
fn pseudo_class_matches(
    pc: &PseudoClass,
    tree: &DomTree,
    node_id: NodeId,
    element: &ElementData,
) -> bool {
    match pc {
        // [§ 4.4 :root](https://www.w3.org/TR/selectors-4/#the-root-pseudo)
        // "The :root pseudo-class represents an element that is the root of the document.
        // In HTML, this is the <html> element."
        PseudoClass::Root => tree.document_element() == Some(node_id),

        // [§ 4.12 :first-child](https://www.w3.org/TR/selectors-4/#the-first-child-pseudo)
        // "The :first-child pseudo-class represents an element that is first among its
        // inclusive siblings."
        PseudoClass::FirstChild => tree.parent(node_id).is_some_and(|parent| {
            tree.children(parent)
                .iter()
                .find(|&&c| tree.as_element(c).is_some())
                == Some(&node_id)
        }),

        // [§ 4.12 :last-child](https://www.w3.org/TR/selectors-4/#the-last-child-pseudo)
        // "The :last-child pseudo-class represents an element that is last among its
        // inclusive siblings."
        PseudoClass::LastChild => tree.parent(node_id).is_some_and(|parent| {
            tree.children(parent)
                .iter()
                .rev()
                .find(|&&c| tree.as_element(c).is_some())
                == Some(&node_id)
        }),

        // [§ 4.11 :first-of-type](https://www.w3.org/TR/selectors-4/#the-first-of-type-pseudo)
        // "The :first-of-type pseudo-class represents an element that is the first sibling
        // of its type."
        PseudoClass::FirstOfType => tree.parent(node_id).is_some_and(|parent| {
            tree.children(parent).iter().find(|&&c| {
                tree.as_element(c)
                    .is_some_and(|e| e.tag_name.eq_ignore_ascii_case(&element.tag_name))
            }) == Some(&node_id)
        }),

        // [§ 4.11 :last-of-type](https://www.w3.org/TR/selectors-4/#the-last-of-type-pseudo)
        // "The :last-of-type pseudo-class represents an element that is the last sibling
        // of its type."
        PseudoClass::LastOfType => tree.parent(node_id).is_some_and(|parent| {
            tree.children(parent).iter().rev().find(|&&c| {
                tree.as_element(c)
                    .is_some_and(|e| e.tag_name.eq_ignore_ascii_case(&element.tag_name))
            }) == Some(&node_id)
        }),

        // [§ 4.12 :only-child](https://www.w3.org/TR/selectors-4/#the-only-child-pseudo)
        // "The :only-child pseudo-class represents an element that has no siblings."
        PseudoClass::OnlyChild => tree.parent(node_id).is_some_and(|parent| {
            tree.children(parent)
                .iter()
                .filter(|&&c| tree.as_element(c).is_some())
                .count()
                == 1
        }),

        // [§ 4.5 :empty](https://www.w3.org/TR/selectors-4/#the-empty-pseudo)
        // "The :empty pseudo-class represents an element that has no children at all.
        // In terms of the document tree, only element nodes and content nodes...
        // must be considered."
        PseudoClass::Empty => {
            tree.children(node_id)
                .iter()
                .all(|&c| match tree.get(c).map(|n| &n.node_type) {
                    Some(NodeType::Text(t)) => t.trim().is_empty(),
                    Some(NodeType::Comment(_)) => true,
                    _ => false,
                })
        }

        // [§ 4.6 :link](https://www.w3.org/TR/selectors-4/#the-link-pseudo)
        // "The :link pseudo-class applies to links that have not yet been visited."
        // In static rendering, all links are unvisited, so :link matches any
        // <a> or <area> element with an href attribute.
        PseudoClass::Link => {
            (element.tag_name.eq_ignore_ascii_case("a")
                || element.tag_name.eq_ignore_ascii_case("area"))
                && element.attrs.contains_key("href")
        }

        // :disabled — element has the disabled attribute
        PseudoClass::Disabled => element.attrs.contains_key("disabled"),

        // :enabled — element does not have the disabled attribute
        PseudoClass::Enabled => !element.attrs.contains_key("disabled"),
    }
}

/// [§ 16.3 Next-sibling combinator](https://www.w3.org/TR/selectors-4/#adjacent-sibling-combinators)
///
/// Find the immediately preceding element sibling (skipping text/comment nodes).
/// Per spec, the next-sibling combinator considers element nodes only.
fn find_previous_element_sibling(tree: &DomTree, node_id: NodeId) -> Option<NodeId> {
    // Walk backwards through preceding siblings until we find an element
    tree.preceding_siblings(node_id)
        .find(|&sibling_id| tree.as_element(sibling_id).is_some())
}

/// [§ 16.4 Subsequent-sibling combinator](https://www.w3.org/TR/selectors-4/#general-sibling-combinators)
///
/// Find any preceding element sibling that matches the compound selector.
fn find_matching_preceding_sibling(
    tree: &DomTree,
    node_id: NodeId,
    compound: &CompoundSelector,
) -> Option<NodeId> {
    tree.preceding_siblings(node_id).find(|&sibling_id| {
        tree.as_element(sibling_id).is_some()
            && compound_matches_in_tree(compound, tree, sibling_id)
    })
}

impl ComplexSelector {
    /// [§ 17 Calculating Specificity](https://www.w3.org/TR/selectors-4/#specificity-rules)
    ///
    /// Calculate specificity for the entire complex selector by summing
    /// specificity of all compound selectors in the chain.
    #[must_use]
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
            SimpleSelector::Class(_)
            | SimpleSelector::PseudoClass(_)
            | SimpleSelector::Attribute(_) => spec.1 += 1,

            // "count the number of type selectors and pseudo-elements
            // in the selector (= C)"
            SimpleSelector::Type(_) => spec.2 += 1,

            // "ignore the universal selector"
            // NeverMatch represents interactive pseudo-classes/pseudo-elements that
            // never match — they contribute 0 to specificity since the entire compound
            // will fail to match anyway.
            SimpleSelector::Universal | SimpleSelector::NeverMatch => {}
        }
    }

    spec
}

impl SimpleSelector {
    /// Check if this simple selector matches the given element.
    #[must_use]
    pub fn matches(&self, element: &ElementData) -> bool {
        match self {
            // [§ 5.1 Type selector](https://www.w3.org/TR/selectors-4/#type-selectors)
            // "A type selector written in the style sheet as an identifier represents
            // an element in the document tree with the same qualified name as the identifier."
            Self::Type(name) => element.tag_name.eq_ignore_ascii_case(name),

            // [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
            // "For documents that use the class attribute (which most do), authors
            // can use the 'period' (.) notation as an alternative."
            Self::Class(class_name) => element.classes().contains(class_name.as_str()),

            // [§ 6.7 ID selector](https://www.w3.org/TR/selectors-4/#id-selectors)
            // "An ID selector represents an element instance that has an identifier
            // that matches the identifier in the ID selector."
            Self::Id(id) => element.id().is_some_and(|el_id| el_id == id),

            // [§ 5.2 Universal selector](https://www.w3.org/TR/selectors-4/#universal-selector)
            // "The universal selector...represents the qualified name of any element type."
            Self::Universal => true,

            // Interactive pseudo-classes/pseudo-elements never match in static rendering.
            // Structural pseudo-classes need tree context to match; without it,
            // conservatively return false.
            Self::NeverMatch | Self::PseudoClass(_) => false,

            // [§ 6.4 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
            Self::Attribute(attr_sel) => match attr_sel {
                // [attr] — has attribute
                AttributeSelector::Exists(name) => element.attrs.contains_key(name.as_str()),
                // [attr=value] — exact match
                AttributeSelector::Equals(name, val) => {
                    element.attrs.get(name.as_str()).is_some_and(|v| v == val)
                }
                // [attr~=value] — space-separated word match
                AttributeSelector::Includes(name, val) => element
                    .attrs
                    .get(name.as_str())
                    .is_some_and(|v| v.split_ascii_whitespace().any(|w| w == val)),
                // [attr|=value] — exact or prefix with hyphen
                AttributeSelector::DashMatch(name, val) => element
                    .attrs
                    .get(name.as_str())
                    .is_some_and(|v| v == val || v.starts_with(&format!("{val}-"))),
                // [attr^=value] — starts with
                AttributeSelector::PrefixMatch(name, val) => element
                    .attrs
                    .get(name.as_str())
                    .is_some_and(|v| v.starts_with(val.as_str())),
                // [attr$=value] — ends with
                AttributeSelector::SuffixMatch(name, val) => element
                    .attrs
                    .get(name.as_str())
                    .is_some_and(|v| v.ends_with(val.as_str())),
                // [attr*=value] — substring
                AttributeSelector::SubstringMatch(name, val) => element
                    .attrs
                    .get(name.as_str())
                    .is_some_and(|v| v.contains(val.as_str())),
            },
        }
    }
}

/// Check if a character can start an identifier.
/// [§ 4.3.10 ident-start code point](https://www.w3.org/TR/css-syntax-3/#ident-start-code-point)
const fn is_ident_start_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

/// Check if a character can continue an identifier.
/// [§ 4.3.9 ident code point](https://www.w3.org/TR/css-syntax-3/#ident-code-point)
const fn is_ident_char(c: char) -> bool {
    is_ident_start_char(c) || c.is_ascii_digit() || c == '-'
}

/// Parse an attribute value inside `[attr=value]`.
/// Handles both quoted (`"val"`, `'val'`) and unquoted ident values.
fn parse_attr_value(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    // Skip whitespace before value
    while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
        let _ = chars.next();
    }

    match chars.peek() {
        Some(&q @ ('"' | '\'')) => {
            let _ = chars.next(); // consume opening quote
            let mut val = String::new();
            for ch in chars.by_ref() {
                if ch == q {
                    return Some(val);
                }
                val.push(ch);
            }
            None // unterminated string
        }
        Some(_) => {
            // Unquoted ident value
            let mut val = String::new();
            while chars
                .peek()
                .is_some_and(|&ch| is_ident_char(ch) || ch == '.')
            {
                val.push(chars.next().unwrap());
            }
            if val.is_empty() { None } else { Some(val) }
        }
        None => None,
    }
}

/// Parse a raw selector string into a `ParsedSelector`.
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
///
/// # Panics
///
/// Panics if the internal compound/combinator bookkeeping is inconsistent
/// (this should not happen with valid input).
#[must_use]
pub fn parse_selector(raw: &str) -> Option<ParsedSelector> {
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

    while let Some(c) = chars.next() {
        match c {
            // [§ 6.6 Class selector](https://www.w3.org/TR/selectors-4/#class-html)
            // "The class selector is given as a full stop (. U+002E)
            // immediately followed by an identifier."
            '.' => {
                flush_ident(&mut current_ident, &mut current_compound);
                // Collect class name
                while chars.peek().is_some_and(|&ch| is_ident_char(ch)) {
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
                while chars.peek().is_some_and(|&ch| is_ident_char(ch)) {
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
                while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
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
                    Some('>' | '+' | '~') => {
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
                while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
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
                while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
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
                while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
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
            // [§ 11 Pseudo-elements](https://www.w3.org/TR/selectors-4/#pseudo-elements)
            ':' => {
                flush_ident(&mut current_ident, &mut current_compound);

                // Check for pseudo-element (::) vs pseudo-class (:)
                let is_pseudo_element = chars.peek() == Some(&':');
                if is_pseudo_element {
                    let _ = chars.next(); // consume second ':'
                }

                // Collect the pseudo name
                let mut pseudo_name = String::new();
                while chars.peek().is_some_and(|&ch| is_ident_char(ch)) {
                    pseudo_name.push(chars.next().unwrap());
                }

                if pseudo_name.is_empty() {
                    return None;
                }

                // If followed by '(', consume balanced parentheses
                // (for :nth-child(...), :not(...), etc.)
                if chars.peek() == Some(&'(') {
                    let _ = chars.next(); // consume '('
                    let mut depth = 1u32;
                    for ch in chars.by_ref() {
                        match ch {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if depth != 0 {
                        return None; // unbalanced parentheses
                    }
                }

                let pseudo_lower = pseudo_name.to_ascii_lowercase();

                if is_pseudo_element {
                    // All pseudo-elements → NeverMatch (we don't render ::before, ::after, etc.)
                    current_compound.push(SimpleSelector::NeverMatch);
                } else {
                    // Dispatch pseudo-class by name
                    match pseudo_lower.as_str() {
                        // Structural pseudo-classes that affect static rendering
                        "root" => {
                            current_compound.push(SimpleSelector::PseudoClass(PseudoClass::Root))
                        }
                        "first-child" => current_compound
                            .push(SimpleSelector::PseudoClass(PseudoClass::FirstChild)),
                        "last-child" => current_compound
                            .push(SimpleSelector::PseudoClass(PseudoClass::LastChild)),
                        "first-of-type" => current_compound
                            .push(SimpleSelector::PseudoClass(PseudoClass::FirstOfType)),
                        "last-of-type" => current_compound
                            .push(SimpleSelector::PseudoClass(PseudoClass::LastOfType)),
                        "only-child" => current_compound
                            .push(SimpleSelector::PseudoClass(PseudoClass::OnlyChild)),
                        "empty" => {
                            current_compound.push(SimpleSelector::PseudoClass(PseudoClass::Empty))
                        }
                        "link" => {
                            current_compound.push(SimpleSelector::PseudoClass(PseudoClass::Link))
                        }
                        "disabled" => current_compound
                            .push(SimpleSelector::PseudoClass(PseudoClass::Disabled)),
                        "enabled" => {
                            current_compound.push(SimpleSelector::PseudoClass(PseudoClass::Enabled))
                        }

                        // Everything else: interactive states, legacy pseudo-elements
                        // (:before, :after), functional pseudo-classes (:nth-child, :not,
                        // :is, :where, :has), and unknown → NeverMatch (graceful degradation)
                        _ => {
                            current_compound.push(SimpleSelector::NeverMatch);
                        }
                    }
                }
            }

            // [§ 6.4 Attribute selectors](https://www.w3.org/TR/selectors-4/#attribute-selectors)
            '[' => {
                flush_ident(&mut current_ident, &mut current_compound);

                // Skip whitespace inside brackets
                while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
                    let _ = chars.next();
                }

                // Collect attribute name
                let mut attr_name = String::new();
                while chars.peek().is_some_and(|&ch| is_ident_char(ch)) {
                    attr_name.push(chars.next().unwrap());
                }

                if attr_name.is_empty() {
                    return None;
                }

                // Skip whitespace after attribute name
                while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
                    let _ = chars.next();
                }

                // Check what follows: ']', '=', '~=', '|=', '^=', '$=', '*='
                match chars.peek() {
                    Some(']') => {
                        let _ = chars.next();
                        current_compound.push(SimpleSelector::Attribute(
                            AttributeSelector::Exists(attr_name),
                        ));
                    }
                    Some('=') => {
                        let _ = chars.next();
                        let val = parse_attr_value(&mut chars)?;
                        // Skip whitespace before ']'
                        while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
                            let _ = chars.next();
                        }
                        if chars.next() != Some(']') {
                            return None;
                        }
                        current_compound.push(SimpleSelector::Attribute(
                            AttributeSelector::Equals(attr_name, val),
                        ));
                    }
                    Some(&op @ ('~' | '|' | '^' | '$' | '*')) => {
                        let _ = chars.next(); // consume operator char
                        if chars.next() != Some('=') {
                            return None;
                        }
                        let val = parse_attr_value(&mut chars)?;
                        // Skip whitespace before ']'
                        while chars.peek().is_some_and(|&ch| ch.is_ascii_whitespace()) {
                            let _ = chars.next();
                        }
                        if chars.next() != Some(']') {
                            return None;
                        }
                        let attr_sel = match op {
                            '~' => AttributeSelector::Includes(attr_name, val),
                            '|' => AttributeSelector::DashMatch(attr_name, val),
                            '^' => AttributeSelector::PrefixMatch(attr_name, val),
                            '$' => AttributeSelector::SuffixMatch(attr_name, val),
                            '*' => AttributeSelector::SubstringMatch(attr_name, val),
                            _ => unreachable!(),
                        };
                        current_compound.push(SimpleSelector::Attribute(attr_sel));
                    }
                    _ => return None,
                }
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
