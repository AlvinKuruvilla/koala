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
    HashMap::from([
        // Most common entities (required for basic HTML)
        ("amp;", "&"),
        ("amp", "&"), // Legacy (no semicolon)
        ("lt;", "<"),
        ("lt", "<"), // Legacy
        ("gt;", ">"),
        ("gt", ">"), // Legacy
        ("quot;", "\""),
        ("quot", "\""), // Legacy
        ("apos;", "'"),
        ("nbsp;", "\u{00A0}"),
        // Common punctuation and symbols
        ("copy;", "\u{00A9}"),   // ©
        ("reg;", "\u{00AE}"),    // ®
        ("trade;", "\u{2122}"),  // ™
        ("mdash;", "\u{2014}"),  // —
        ("ndash;", "\u{2013}"),  // –
        ("hellip;", "\u{2026}"), // …
        ("bull;", "\u{2022}"),   // •
        ("middot;", "\u{00B7}"), // ·
        ("lsquo;", "\u{2018}"),  // '
        ("rsquo;", "\u{2019}"),  // '
        ("ldquo;", "\u{201C}"),  // "
        ("rdquo;", "\u{201D}"),  // "
        ("laquo;", "\u{00AB}"),  // «
        ("raquo;", "\u{00BB}"),  // »
        // Currency
        ("cent;", "\u{00A2}"),  // ¢
        ("pound;", "\u{00A3}"), // £
        ("euro;", "\u{20AC}"),  // €
        ("yen;", "\u{00A5}"),   // ¥
        // Math symbols
        ("times;", "\u{00D7}"),  // ×
        ("divide;", "\u{00F7}"), // ÷
        ("plusmn;", "\u{00B1}"), // ±
        ("ne;", "\u{2260}"),     // ≠
        ("le;", "\u{2264}"),     // ≤
        ("ge;", "\u{2265}"),     // ≥
        ("deg;", "\u{00B0}"),    // °
        ("frac12;", "\u{00BD}"), // ½
        ("frac14;", "\u{00BC}"), // ¼
        ("frac34;", "\u{00BE}"), // ¾
        // Arrows
        ("larr;", "\u{2190}"), // ←
        ("rarr;", "\u{2192}"), // →
        ("uarr;", "\u{2191}"), // ↑
        ("darr;", "\u{2193}"), // ↓
        // Greek letters (commonly used)
        ("alpha;", "\u{03B1}"),
        ("beta;", "\u{03B2}"),
        ("gamma;", "\u{03B3}"),
        ("delta;", "\u{03B4}"),
        ("pi;", "\u{03C0}"),
        ("sigma;", "\u{03C3}"),
        ("omega;", "\u{03C9}"),
        // Accented characters (common)
        ("Agrave;", "\u{00C0}"),
        ("Aacute;", "\u{00C1}"),
        ("Acirc;", "\u{00C2}"),
        ("Atilde;", "\u{00C3}"),
        ("Auml;", "\u{00C4}"),
        ("agrave;", "\u{00E0}"),
        ("aacute;", "\u{00E1}"),
        ("acirc;", "\u{00E2}"),
        ("atilde;", "\u{00E3}"),
        ("auml;", "\u{00E4}"),
        ("Egrave;", "\u{00C8}"),
        ("Eacute;", "\u{00C9}"),
        ("egrave;", "\u{00E8}"),
        ("eacute;", "\u{00E9}"),
        ("Igrave;", "\u{00CC}"),
        ("Iacute;", "\u{00CD}"),
        ("igrave;", "\u{00EC}"),
        ("iacute;", "\u{00ED}"),
        ("Ograve;", "\u{00D2}"),
        ("Oacute;", "\u{00D3}"),
        ("ograve;", "\u{00F2}"),
        ("oacute;", "\u{00F3}"),
        ("Ugrave;", "\u{00D9}"),
        ("Uacute;", "\u{00DA}"),
        ("ugrave;", "\u{00F9}"),
        ("uacute;", "\u{00FA}"),
        ("ntilde;", "\u{00F1}"),
        ("Ntilde;", "\u{00D1}"),
        ("ccedil;", "\u{00E7}"),
        ("Ccedil;", "\u{00C7}"),
    ])
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
