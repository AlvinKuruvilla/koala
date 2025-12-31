//! Named character reference lookup table.
//!
//! [§ 13.2.5.73 Named character reference state](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)
//!
//! This module provides lookup functions for HTML named character references.
//! The full spec defines 2,231 entities; we implement the most common ones here.

use std::collections::HashMap;
use std::sync::LazyLock;

/// The named character reference table.
/// Maps entity names (without the leading '&') to their replacement strings.
///
/// NOTE: Some entities map to multiple characters (e.g., "fjlig" -> "fj").
/// The spec requires entities to be matched WITH the trailing semicolon when present,
/// but some legacy entities work without it (e.g., "&amp" matches "&amp;").
static NAMED_ENTITIES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Most common entities (required for basic HTML)
    m.insert("amp;", "&");
    m.insert("amp", "&");  // Legacy (no semicolon)
    m.insert("lt;", "<");
    m.insert("lt", "<");   // Legacy
    m.insert("gt;", ">");
    m.insert("gt", ">");   // Legacy
    m.insert("quot;", "\"");
    m.insert("quot", "\""); // Legacy
    m.insert("apos;", "'");
    m.insert("nbsp;", "\u{00A0}");

    // Common punctuation and symbols
    m.insert("copy;", "\u{00A9}");   // ©
    m.insert("reg;", "\u{00AE}");    // ®
    m.insert("trade;", "\u{2122}");  // ™
    m.insert("mdash;", "\u{2014}");  // —
    m.insert("ndash;", "\u{2013}");  // –
    m.insert("hellip;", "\u{2026}"); // …
    m.insert("bull;", "\u{2022}");   // •
    m.insert("middot;", "\u{00B7}"); // ·
    m.insert("lsquo;", "\u{2018}");  // '
    m.insert("rsquo;", "\u{2019}");  // '
    m.insert("ldquo;", "\u{201C}");  // "
    m.insert("rdquo;", "\u{201D}");  // "
    m.insert("laquo;", "\u{00AB}");  // «
    m.insert("raquo;", "\u{00BB}");  // »

    // Currency
    m.insert("cent;", "\u{00A2}");   // ¢
    m.insert("pound;", "\u{00A3}");  // £
    m.insert("euro;", "\u{20AC}");   // €
    m.insert("yen;", "\u{00A5}");    // ¥

    // Math symbols
    m.insert("times;", "\u{00D7}");  // ×
    m.insert("divide;", "\u{00F7}"); // ÷
    m.insert("plusmn;", "\u{00B1}"); // ±
    m.insert("ne;", "\u{2260}");     // ≠
    m.insert("le;", "\u{2264}");     // ≤
    m.insert("ge;", "\u{2265}");     // ≥
    m.insert("deg;", "\u{00B0}");    // °
    m.insert("frac12;", "\u{00BD}"); // ½
    m.insert("frac14;", "\u{00BC}"); // ¼
    m.insert("frac34;", "\u{00BE}"); // ¾

    // Arrows
    m.insert("larr;", "\u{2190}");   // ←
    m.insert("rarr;", "\u{2192}");   // →
    m.insert("uarr;", "\u{2191}");   // ↑
    m.insert("darr;", "\u{2193}");   // ↓

    // Greek letters (commonly used)
    m.insert("alpha;", "\u{03B1}");
    m.insert("beta;", "\u{03B2}");
    m.insert("gamma;", "\u{03B3}");
    m.insert("delta;", "\u{03B4}");
    m.insert("pi;", "\u{03C0}");
    m.insert("sigma;", "\u{03C3}");
    m.insert("omega;", "\u{03C9}");

    // Accented characters (common)
    m.insert("Agrave;", "\u{00C0}");
    m.insert("Aacute;", "\u{00C1}");
    m.insert("Acirc;", "\u{00C2}");
    m.insert("Atilde;", "\u{00C3}");
    m.insert("Auml;", "\u{00C4}");
    m.insert("agrave;", "\u{00E0}");
    m.insert("aacute;", "\u{00E1}");
    m.insert("acirc;", "\u{00E2}");
    m.insert("atilde;", "\u{00E3}");
    m.insert("auml;", "\u{00E4}");
    m.insert("Egrave;", "\u{00C8}");
    m.insert("Eacute;", "\u{00C9}");
    m.insert("egrave;", "\u{00E8}");
    m.insert("eacute;", "\u{00E9}");
    m.insert("Igrave;", "\u{00CC}");
    m.insert("Iacute;", "\u{00CD}");
    m.insert("igrave;", "\u{00EC}");
    m.insert("iacute;", "\u{00ED}");
    m.insert("Ograve;", "\u{00D2}");
    m.insert("Oacute;", "\u{00D3}");
    m.insert("ograve;", "\u{00F2}");
    m.insert("oacute;", "\u{00F3}");
    m.insert("Ugrave;", "\u{00D9}");
    m.insert("Uacute;", "\u{00DA}");
    m.insert("ugrave;", "\u{00F9}");
    m.insert("uacute;", "\u{00FA}");
    m.insert("ntilde;", "\u{00F1}");
    m.insert("Ntilde;", "\u{00D1}");
    m.insert("ccedil;", "\u{00E7}");
    m.insert("Ccedil;", "\u{00C7}");

    m
});

/// Look up a named character reference.
///
/// Returns the replacement string if found.
/// The `name` should NOT include the leading '&'.
///
/// # Example
/// ```ignore
/// lookup_entity("amp;")  // Returns Some("&")
/// lookup_entity("amp")   // Returns Some("&") - legacy support
/// lookup_entity("xyz;")  // Returns None
/// ```
pub fn lookup_entity(name: &str) -> Option<&'static str> {
    NAMED_ENTITIES.get(name).copied()
}

/// Check if any entity name starts with the given prefix.
///
/// This is used to determine whether we should keep consuming characters
/// while looking for the longest match.
///
/// # Example
/// ```ignore
/// any_entity_has_prefix("am")   // true (amp, amp;)
/// any_entity_has_prefix("xyz")  // false
/// ```
pub fn any_entity_has_prefix(prefix: &str) -> bool {
    NAMED_ENTITIES.keys().any(|name| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(any_entity_has_prefix("a"));      // amp, apos, alpha, etc.
        assert!(any_entity_has_prefix("am"));     // amp
        assert!(any_entity_has_prefix("amp"));    // amp, amp;
        assert!(any_entity_has_prefix("amp;"));   // amp;
        assert!(!any_entity_has_prefix("ampx"));  // nothing
        assert!(!any_entity_has_prefix("xyz"));   // nothing
    }
}
