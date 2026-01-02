//! Integration tests for named character reference lookup.

use koala_html::tokenizer::named_character_references::{any_entity_has_prefix, lookup_entity};

#[test]
fn test_lookup_common_entities() {
    assert_eq!(lookup_entity("amp;"), Some("&"));
    assert_eq!(lookup_entity("lt;"), Some("<"));
    assert_eq!(lookup_entity("gt;"), Some(">"));
    assert_eq!(lookup_entity("quot;"), Some("\""));
    assert_eq!(lookup_entity("nbsp;"), Some("\u{00A0}"));
}

#[test]
fn test_lookup_legacy_entities() {
    // Legacy entities without semicolon
    assert_eq!(lookup_entity("amp"), Some("&"));
    assert_eq!(lookup_entity("lt"), Some("<"));
    assert_eq!(lookup_entity("gt"), Some(">"));
}

#[test]
fn test_lookup_unknown_entity() {
    assert_eq!(lookup_entity("notarealentity;"), None);
    assert_eq!(lookup_entity(""), None);
}

#[test]
fn test_prefix_matching() {
    assert!(any_entity_has_prefix("a")); // amp, apos, alpha, etc.
    assert!(any_entity_has_prefix("am")); // amp
    assert!(any_entity_has_prefix("amp")); // amp, amp;
    assert!(any_entity_has_prefix("amp;")); // amp;
    assert!(!any_entity_has_prefix("ampx")); // nothing
    assert!(!any_entity_has_prefix("xyz")); // nothing
}
