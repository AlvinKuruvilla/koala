//! CSS Cascading and Style Computation
//!
//! This module implements style computation per
//! [CSS Cascading and Inheritance Level 4](https://www.w3.org/TR/css-cascade-4/).

use std::collections::HashMap;

use crate::parser::{Rule, StyleRule, Stylesheet};
use crate::selector::{ParsedSelector, Specificity, parse_selector};
use crate::style::ComputedStyle;
use koala_common::warning::warn_once;
use koala_dom::{DomTree, NodeId, NodeType};

/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// "Each style rule has a cascade origin, which determines where it enters
/// the cascade."
///
/// "The cascading process sorts declarations according to the following
/// criteria, in descending order of priority:
/// Origin and Importance > Context > Element-Attached Styles >
/// Specificity > Order of Appearance"
///
/// `UserAgent` (0) < `Author` (1): author rules always override UA rules
/// regardless of specificity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CascadeOrigin {
    /// [§ 6.1](https://www.w3.org/TR/css-cascade-4/#cascade-origin-ua)
    /// "The user agent's default styles."
    UserAgent = 0,
    /// [§ 6.1](https://www.w3.org/TR/css-cascade-4/#cascade-origin-author)
    /// "The author specifies style sheets for a source document."
    Author = 1,
}

/// [§ 6 Cascading](https://www.w3.org/TR/css-cascade-4/#cascading)
///
/// A matched rule with its origin and specificity for cascade ordering.
struct MatchedRule<'a> {
    origin: CascadeOrigin,
    specificity: Specificity,
    rule: &'a StyleRule,
}

/// A pre-parsed rule: one (selector, rule) pair tagged with its origin.
///
/// Each comma-separated selector in a rule produces a separate `ParsedRule`.
/// For example, `h1, h2, h3 { font-weight: bold; }` produces three entries.
struct ParsedRule<'a> {
    origin: CascadeOrigin,
    selector: ParsedSelector,
    rule: &'a StyleRule,
}

/// Parse all rules from a stylesheet, expanding comma-separated selectors.
///
/// [§ 5.1 Selector Lists](https://www.w3.org/TR/selectors-4/#grouping)
///
/// "A comma-separated list of selectors represents the union of all
/// elements selected by each individual selector in the list."
///
/// Each valid selector in a rule produces a separate `ParsedRule` entry
/// so that matching checks every selector independently.
fn parse_stylesheet_rules<'a>(
    stylesheet: &'a Stylesheet,
    origin: CascadeOrigin,
    out: &mut Vec<ParsedRule<'a>>,
) {
    for rule in &stylesheet.rules {
        match rule {
            Rule::Style(style_rule) => {
                let mut any_parsed = false;

                // Expand ALL valid selectors, not just the first one.
                // This is critical for rules like `h1, h2, h3 { ... }`.
                for sel in &style_rule.selectors {
                    if let Some(parsed) = parse_selector(&sel.text) {
                        out.push(ParsedRule {
                            origin,
                            selector: parsed,
                            rule: style_rule,
                        });
                        any_parsed = true;
                    }
                }

                // Warn if all selectors in this rule failed to parse
                if !any_parsed && !style_rule.selectors.is_empty() {
                    let selector_text = style_rule
                        .selectors
                        .iter()
                        .map(|s| s.text.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    warn_once(
                        "CSS",
                        &format!("failed to parse selector '{selector_text}'"),
                    );
                }
            }
            Rule::At(_) => {} // Skip at-rules for MVP
        }
    }
}

/// [§ 6 Cascading](https://www.w3.org/TR/css-cascade-4/#cascading)
///
/// "The cascade takes an unordered list of declared values for a given property
/// on a given element, sorts them by their declaration's precedence..."
///
/// Compute styles for the entire DOM tree given UA and author stylesheets.
/// Returns a map from `NodeId` to computed style.
///
/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// UA rules are always overridden by author rules (origin beats specificity).
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn compute_styles(
    tree: &DomTree,
    ua_stylesheet: &Stylesheet,
    author_stylesheet: &Stylesheet,
) -> HashMap<NodeId, ComputedStyle> {
    let mut styles = HashMap::new();

    // Parse all selectors upfront, tagged with their origin.
    let mut parsed_rules = Vec::new();
    parse_stylesheet_rules(ua_stylesheet, CascadeOrigin::UserAgent, &mut parsed_rules);
    parse_stylesheet_rules(author_stylesheet, CascadeOrigin::Author, &mut parsed_rules);

    // Start with default inherited style (none)
    let initial_style = ComputedStyle::default();
    compute_node_styles(
        tree,
        tree.root(),
        &parsed_rules,
        &initial_style,
        &mut styles,
    );

    styles
}

/// [§ 6 Cascading](https://www.w3.org/TR/css-cascade-4/#cascading)
///
/// Recursively compute styles for a node and its children.
/// Applies cascade sorting and specificity rules per the spec.
fn compute_node_styles(
    tree: &DomTree,
    id: NodeId,
    rules: &[ParsedRule],
    inherited: &ComputedStyle,
    styles: &mut HashMap<NodeId, ComputedStyle>,
) {
    let Some(node) = tree.get(id) else { return };

    match &node.node_type {
        NodeType::Element(_element_data) => {
            // [§ 7 Inheritance](https://www.w3.org/TR/css-cascade-4/#inheriting)
            // Start with inherited styles
            let mut computed = inherit_styles(inherited);

            // [§ 6.4 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
            // Find all matching rules using tree-aware matching for combinator support
            let mut matched: Vec<MatchedRule> = rules
                .iter()
                .filter(|pr| pr.selector.matches_in_tree(tree, id))
                .map(|pr| MatchedRule {
                    origin: pr.origin,
                    specificity: pr.selector.specificity,
                    rule: pr.rule,
                })
                .collect();

            // [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
            //
            // "The cascading process sorts declarations according to the following
            // criteria, in descending order of priority:
            // Origin and Importance > ... > Specificity > Order of Appearance"
            //
            // Sort by (origin, specificity) — UA rules sort before author rules,
            // so author rules always override UA rules regardless of specificity.
            // Within the same origin, higher specificity wins.
            matched.sort_by(|a, b| {
                a.origin
                    .cmp(&b.origin)
                    .then_with(|| a.specificity.cmp(&b.specificity))
            });

            // Apply declarations in order (lowest priority first, highest last wins)
            for m in matched {
                for decl in &m.rule.declarations {
                    computed.apply_declaration(decl);
                }
            }

            // Store the computed style
            let _ = styles.insert(id, computed.clone());

            // Recurse to children with this element's computed style as inherited
            for &child_id in tree.children(id) {
                compute_node_styles(tree, child_id, rules, &computed, styles);
            }
        }
        NodeType::Document => {
            // Document doesn't have styles itself, but pass through to children
            for &child_id in tree.children(id) {
                compute_node_styles(tree, child_id, rules, inherited, styles);
            }
        }
        // Text and comment nodes don't have styles applied directly.
        // They inherit from their parent element when rendered.
        NodeType::Text(_) | NodeType::Comment(_) => {}
    }
}

/// [§ 7.1 Inherited Properties](https://www.w3.org/TR/css-cascade-4/#inherited-property)
/// "Some properties are inherited from an ancestor element to its descendants."
///
/// Create a new style inheriting appropriate properties from the parent.
fn inherit_styles(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        // Inherited properties
        // [§ 3.1 color](https://www.w3.org/TR/css-color-4/#the-color-property)
        // "Inherited: yes"
        color: parent.color.clone(),

        // [§ 3.1 font-family](https://www.w3.org/TR/css-fonts-4/#font-family-prop)
        // "Inherited: yes"
        font_family: parent.font_family.clone(),

        // [§ 3.5 font-size](https://www.w3.org/TR/css-fonts-4/#font-size-prop)
        // "Inherited: yes"
        font_size: parent.font_size,

        // [§ 3.2 font-weight](https://www.w3.org/TR/css-fonts-4/#font-weight-prop)
        // "Inherited: yes"
        font_weight: parent.font_weight,

        // [§ 3.3 font-style](https://www.w3.org/TR/css-fonts-4/#font-style-prop)
        // "Inherited: yes"
        font_style: parent.font_style.clone(),

        // [§ 4.2 line-height](https://www.w3.org/TR/css-inline-3/#line-height-property)
        // "Inherited: yes"
        line_height: parent.line_height,

        // [§ 2 writing-mode](https://www.w3.org/TR/css-writing-modes-4/#block-flow)
        // "Inherited: yes"
        writing_mode: parent.writing_mode,

        // [§ 16.2 text-align](https://www.w3.org/TR/CSS2/text.html#alignment-prop)
        // "Inherited: yes"
        text_align: parent.text_align.clone(),

        // Non-inherited properties start as None
        //
        // [§ 2 display](https://www.w3.org/TR/css-display-3/#the-display-properties)
        // "Inherited: no"
        display: None,
        display_none: false,

        // [§ 3.2 background-color](https://www.w3.org/TR/css-backgrounds-3/#background-color)
        // "Inherited: no"
        background_color: None,

        // [§ 6 Box Model](https://www.w3.org/TR/css-box-4/)
        // "Inherited: no"
        margin_top: None,
        margin_right: None,
        margin_bottom: None,
        margin_left: None,
        // [§ 4.2 Flow-Relative Margins](https://drafts.csswg.org/css-logical-1/#margin-properties)
        // "Inherited: no"
        margin_block_start: None,
        margin_block_end: None,
        padding_top: None,
        padding_right: None,
        padding_bottom: None,
        padding_left: None,

        // Borders are not inherited
        border_top: None,
        border_right: None,
        border_bottom: None,
        border_left: None,

        // [§ 10.2 width](https://www.w3.org/TR/CSS2/visudet.html#the-width-property)
        // [§ 10.5 height](https://www.w3.org/TR/CSS2/visudet.html#the-height-property)
        // "Inherited: no"
        width: None,
        height: None,

        // [§ 10.4 min-width / max-width](https://www.w3.org/TR/CSS2/visudet.html#min-max-widths)
        // [§ 10.7 min-height / max-height](https://www.w3.org/TR/CSS2/visudet.html#min-max-heights)
        // "Inherited: no"
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,

        // [§ 5.1 flex-direction](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
        // [§ 8.2 justify-content](https://www.w3.org/TR/css-flexbox-1/#justify-content-property)
        // [§ 7 Flexibility](https://www.w3.org/TR/css-flexbox-1/#flexibility)
        // "Inherited: no"
        flex_direction: None,
        justify_content: None,
        flex_grow: None,
        flex_shrink: None,
        flex_basis: None,

        // [§ 9.5 float](https://www.w3.org/TR/CSS2/visuren.html#floats)
        // "Inherited: no"
        float: None,
        // [§ 9.5.2 clear](https://www.w3.org/TR/CSS2/visuren.html#flow-control)
        // "Inherited: no"
        clear: None,

        // [§ 9.3.1 position](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
        // "Inherited: no"
        position: None,
        // [§ 9.3.2 Box offsets](https://www.w3.org/TR/CSS2/visuren.html#position-props)
        // "Inherited: no"
        top: None,
        right: None,
        bottom: None,
        left: None,

        // [§ 4.4 box-sizing](https://www.w3.org/TR/css-box-4/#box-sizing)
        // "Inherited: no"
        box_sizing_border_box: None,

        // Source order tracking for cascade resolution (not inherited, per-element)
        margin_top_source_order: None,
        margin_right_source_order: None,
        margin_bottom_source_order: None,
        margin_left_source_order: None,
    }
}
