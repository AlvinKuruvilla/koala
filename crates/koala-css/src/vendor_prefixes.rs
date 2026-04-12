//! Vendor-prefix policy for koala-css.
//!
//! Vendor-prefixed CSS properties come in three flavours, and each
//! needs a different response from the engine:
//!
//! 1. **Legacy prefixes for standard properties.** Things like
//!    `-webkit-border-radius`, `-webkit-transform`,
//!    `-webkit-box-shadow`. These have a standard counterpart;
//!    authors should be using the unprefixed form. We *warn* on
//!    these so the author knows to fix it. A future improvement is
//!    to detect the standard form and suggest it in the warning
//!    ("did you mean `border-radius`?"), or accept the prefixed
//!    form as an alias of the standard one. Not done yet.
//!
//! 2. **Vendor-only extensions that were never standardised.**
//!    WebKit / Mozilla hooks like `-webkit-font-smoothing` or
//!    `-moz-osx-font-smoothing` that the CSS Working Group
//!    explicitly declined to adopt. There is no upstream spec to
//!    implement. We *silently ignore* these, because the whole
//!    point of vendor prefixes is "engines that don't support me
//!    should skip me quietly" — per
//!    [CSS Syntax § 4.1.1](https://www.w3.org/TR/css-syntax-3/#consume-declaration),
//!    "if any property is unknown, the entire declaration must
//!    be ignored." A silent declaration is the spec-correct
//!    behaviour.
//!
//! 3. **In-flight experimental features.** Rare today but
//!    historically things like `-ms-grid` before `grid` landed.
//!    Not currently relevant to koala-css — flag if one surfaces.
//!
//! This module owns the category-2 list: the vendor extensions we
//! have decided, with rationale, to explicitly drop on the floor.
//! When a new vendor-only extension surfaces, add an entry here
//! with a one-sentence explanation. Everything else that starts
//! with `-` falls through to the standard "unknown property"
//! warning path, so author typos for category-1 properties stay
//! visible.
//!
//! The style system dispatches here from
//! [`computed.rs`](crate::style::computed)'s property-unknown
//! arm — see `is_silent_vendor_property`.

/// A single vendor-prefixed property that koala-css has decided
/// to silently ignore, plus the reason it was silenced. The
/// `rationale` is purely documentation — it exists so a future
/// reader can understand the intent without grep-hunting through
/// commit messages.
pub struct SilentVendorProperty {
    /// The exact property name as it appears in the author's CSS,
    /// including the leading `-` and the vendor identifier.
    pub name: &'static str,
    /// One-sentence explanation of why the engine drops this
    /// declaration without warning.
    pub rationale: &'static str,
}

/// The complete list of category-2 vendor extensions koala-css
/// silently ignores. Extending this list is an explicit policy
/// change: add the entry, write the rationale, and update
/// `tests::silenced_properties_are_listed` in this file.
pub const SILENT_VENDOR_PROPERTIES: &[SilentVendorProperty] = &[
    SilentVendorProperty {
        name: "-webkit-font-smoothing",
        rationale:
            "Text anti-aliasing hint exclusive to WebKit on macOS. \
             No CSS spec — the CSSWG has declined to standardise \
             font smoothing controls because rasterization strategy \
             is an engine concern, not an author concern. Our \
             fontdue-based rasterizer produces grayscale anti-aliased \
             glyphs by default, which is what `antialiased` asks for, \
             so silently ignoring the declaration preserves the \
             author's intent.",
    },
    SilentVendorProperty {
        name: "-moz-osx-font-smoothing",
        rationale:
            "Firefox counterpart of `-webkit-font-smoothing` targeting \
             macOS only. Same rationale: no spec, and our default \
             rasterization matches what `grayscale` requests.",
    },
    SilentVendorProperty {
        name: "-webkit-tap-highlight-color",
        rationale:
            "iOS Safari only — suppresses the translucent overlay \
             iOS draws on tapped elements. No desktop equivalent, \
             no spec, no behaviour to match on a pointer device.",
    },
];

/// Returns `true` when `property` is a vendor-prefixed property
/// the engine has explicitly decided to silently ignore. Called
/// from the style system's unknown-property arm so category-2
/// vendor extensions drop out without emitting a warning, while
/// everything else (including category-1 legacy prefixes) stays
/// on the warning path.
#[must_use]
pub fn is_silent_vendor_property(property: &str) -> bool {
    SILENT_VENDOR_PROPERTIES
        .iter()
        .any(|entry| entry.name == property)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry in `SILENT_VENDOR_PROPERTIES` is reachable via
    /// `is_silent_vendor_property`. Guards against typos between
    /// the const list and the lookup function.
    #[test]
    fn silenced_properties_are_all_recognised() {
        for entry in SILENT_VENDOR_PROPERTIES {
            assert!(
                is_silent_vendor_property(entry.name),
                "{} should be silenced but lookup returned false",
                entry.name,
            );
        }
    }

    /// Category-1 vendor prefixes — legacy forms of standard
    /// properties — must NOT be silenced. These should warn so
    /// the author sees they wrote `-webkit-border-radius` instead
    /// of `border-radius`. If one of these accidentally gets added
    /// to the silent list, this test fails.
    #[test]
    fn legacy_prefixes_for_standard_properties_still_warn() {
        for property in [
            "-webkit-border-radius",
            "-webkit-transform",
            "-webkit-box-shadow",
            "-moz-border-radius",
            "-ms-grid",
        ] {
            assert!(
                !is_silent_vendor_property(property),
                "{property} has a standard counterpart and must stay on \
                 the warning path so author typos remain visible",
            );
        }
    }

    /// Non-prefixed properties are never silent through this
    /// module. The function is specifically for vendor prefixes;
    /// anything else would indicate a policy leak.
    #[test]
    fn standard_properties_are_not_silent() {
        for property in ["color", "font-size", "border-radius", "display"] {
            assert!(
                !is_silent_vendor_property(property),
                "{property} is a standard property, not a vendor \
                 extension — it should not be in the silent list",
            );
        }
    }

    /// `font-smoothing` without the `-webkit-` prefix is not a
    /// real CSS property either, but it's not our problem to
    /// silence — if an author writes it, that's a typo we should
    /// flag, not hide.
    #[test]
    fn unprefixed_font_smoothing_is_not_silent() {
        assert!(!is_silent_vendor_property("font-smoothing"));
    }
}
