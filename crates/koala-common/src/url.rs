//! URL resolution utilities.
//!
//! [§ 4.2.3 The base element](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element)
//! [URL Standard](https://url.spec.whatwg.org/)

/// [§ 4.2.3 The base element](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element)
/// [URL Standard](https://url.spec.whatwg.org/)
///
/// Resolve a potentially relative URL against a base URL.
///
/// # Algorithm
///
/// [§ 2.5 URLs](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#resolving-urls)
///
/// STEP 1: "If url is an absolute URL, return url."
///
/// STEP 2: "Otherwise, resolve url relative to base."
///
/// NOTE: This is a simplified implementation. Full URL resolution requires
/// implementing the URL Standard's URL parsing algorithm.
#[must_use]
pub fn resolve_url(href: &str, base_url: Option<&str>) -> String {
    // STEP 1: Check if href is already absolute.
    //
    // [URL Standard § 4.3](https://url.spec.whatwg.org/#url-parsing)
    // "An absolute-URL string is a URL-scheme string, followed by U+003A (:),
    // followed by a scheme-specific part."
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("data:")
        || href.starts_with("file:")
    {
        return href.to_string();
    }

    // STEP 2: Resolve relative URL against base.
    //
    // TODO(url-resolution): Implement proper URL resolution per URL Standard.
    // The full algorithm handles:
    // - Protocol-relative URLs (//example.com/path)
    // - Absolute paths (/path/to/file)
    // - Relative paths (../path, ./path, path)
    // - Query strings and fragments
    //
    // For now, do simple path joining for common cases.
    let Some(base) = base_url else {
        return href.to_string();
    };

    if href.starts_with("//") {
        // Protocol-relative URL - prepend scheme from base
        //
        // TODO(url-resolution): Extract scheme from base properly
        if base.starts_with("https:") {
            format!("https:{href}")
        } else {
            format!("http:{href}")
        }
    } else if href.starts_with('/') {
        // Absolute path - join with origin
        //
        // TODO(url-resolution): Extract origin from base_url properly
        // For now, find the third slash (after scheme://) and take everything before it
        base.find("://").map_or_else(
            || href.to_string(),
            |scheme_end| {
                let after_scheme = &base[scheme_end + 3..];
                after_scheme.find('/').map_or_else(
                    // No path in base, just append
                    || format!("{base}{href}"),
                    |path_start| {
                        let origin = &base[..scheme_end + 3 + path_start];
                        format!("{origin}{href}")
                    },
                )
            },
        )
    } else {
        // Relative path - join with base directory
        //
        // TODO(url-resolution): Handle . and .. path segments properly
        let base_dir = base.rsplit_once('/').map_or(base, |(dir, _)| dir);
        format!("{base_dir}/{href}")
    }
}
