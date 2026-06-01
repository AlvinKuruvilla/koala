//! URL resolution utilities.
//!
//! [RFC 3986 § 5.2 Relative Resolution](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2)
//! is the source of truth for the algorithm. Per
//! [HTML § 4.2.3 The base element](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element)
//! the URL Standard delegates to RFC 3986's transformation
//! shape for reference resolution, so anchoring on the older
//! RFC keeps the code small and citable without committing to
//! a full URL Standard parser.

/// [RFC 3986 § 5.2 Relative Resolution](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2)
///
/// Transform a reference `R` (`href`) against a base URI
/// `Base` (`base_url`) and return the target URI `T`.
///
/// # Algorithm
///
/// Follows the case split from
/// [§ 5.2.2 Transform References](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2.2)
/// with the merge subroutine from
/// [§ 5.2.3 Merge Paths](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2.3).
///
/// # Scope
///
/// Three deliberate simplifications relative to the full RFC:
///
/// - [§ 5.2.4 Remove Dot Segments](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2.4)
///   isn't applied — `.` and `..` ride through as literal path
///   segments. Existing TODO; doesn't affect the bare-name
///   relative case this function is shaped around.
/// - Query and fragment of `R` are not split out from the
///   reference string — they ride along inside `href` and land
///   on the merged path verbatim, which works for every koala
///   caller today (`<script src>`, `<link href>`, etc.).
/// - `Base.query` (only consulted by § 5.2.2's "R has empty
///   path and no query" arm, i.e. fragment-only references) is
///   ignored. No koala script-loading site exercises that arm.
///
/// Returns `href` verbatim when no base is provided or when
/// the base can't be parsed — same fallback as the prior
/// implementation.
#[must_use]
pub fn resolve_url(href: &str, base_url: Option<&str>) -> String {
    // [§ 5.2.2] "if defined(R.scheme) then T = R" — an
    // absolute reference is the target verbatim, no merge.
    if has_scheme(href) {
        return href.to_string();
    }

    let Some(base) = base_url.and_then(parse_base) else {
        // No usable base — return the reference as-is.
        return href.to_string();
    };

    // [§ 5.2.2] R has no scheme — its shape selects the branch.
    if let Some(after) = href.strip_prefix("//") {
        // "if defined(R.authority)" — protocol-relative
        // reference. Adopt base's scheme; everything else comes
        // from R.
        format!("{}://{after}", base.scheme)
    } else if href.starts_with('/') {
        // "else if R.path starts-with '/'" — absolute-path
        // reference. Adopt base's scheme + authority; R
        // replaces the path entirely.
        format!("{}://{}{href}", base.scheme, base.authority)
    } else {
        // "else: T.path = merge(Base.path, R.path)". An empty
        // R also lands here and gets merged correctly (R.path
        // == "" produces base's directory unchanged).
        let merged = merge_paths(
            !base.authority.is_empty(),
            base.path,
            href,
        );
        format!("{}://{}{merged}", base.scheme, base.authority)
    }
}

/// Decomposed base URI carrying only the fields
/// [§ 5.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2.2)
/// reads during resolution.
///
/// `query` is intentionally absent: it's only consulted when
/// the reference has no path and no query of its own (i.e. a
/// fragment-only or empty reference inheriting the base's
/// query), which no koala script-loading site exercises today.
/// Add it back when a caller materialises that case.
struct BaseParts<'a> {
    scheme: &'a str,
    /// May be empty — `file:///path` parses as authority="".
    /// The bug case (`https://news.ycombinator.com` with no
    /// trailing slash) has authority="news.ycombinator.com"
    /// and `path=""`, which is the merge-path edge per § 5.2.3.
    authority: &'a str,
    /// Includes the leading `/` when present. Empty when the
    /// base is authority-only, as in `https://example.com`.
    path: &'a str,
}

/// Parse a base URI into the components § 5.2.2 reads.
///
/// Strictly minimal — assumes the base is well-formed
/// `scheme "://" authority path?query?fragment`, which is what
/// `koala-browser` produces for http/https bases today.
/// Returns `None` if there's no `"://"` separator; the caller
/// falls back to "leave the reference as-is".
///
/// Does NOT validate scheme or authority charset — that's the
/// URL Standard's job, and koala doesn't carry a full URL
/// parser. The transformation algorithm just needs the pieces.
fn parse_base(base: &str) -> Option<BaseParts<'_>> {
    let (scheme, rest) = base.split_once("://")?;

    // [§ 3.2] Authority terminates at the first '/' (start of
    // path), '?' (start of query), '#' (start of fragment), or
    // end-of-string.
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];

    // [§ 3.3] Path begins at the authority terminator and runs
    // up to the next '?' or '#'. The leading '/' (if any) is
    // part of the path.
    let after_auth = &rest[auth_end..];
    let path_end = after_auth.find(['?', '#']).unwrap_or(after_auth.len());
    let path = &after_auth[..path_end];

    Some(BaseParts {
        scheme,
        authority,
        path,
    })
}

/// [§ 5.2.3 Merge Paths](https://datatracker.ietf.org/doc/html/rfc3986#section-5.2.3).
///
/// > "if defined(Base.authority) and empty(Base.path) then
/// >    return a string consisting of the reference's path
/// >    component appended to '/', as in:
/// >        result = '/' + R.path
/// > else
/// >    return a string consisting of the reference's path
/// >    component appended to all but the last segment of the
/// >    base URI's path (i.e., excluding any characters after
/// >    the right-most '/' in the base URI path, or excluding
/// >    the entire base URI path if it does not contain any
/// >    '/' characters)."
fn merge_paths(base_has_authority: bool, base_path: &str, ref_path: &str) -> String {
    if base_has_authority && base_path.is_empty() {
        // First clause — authority-only base. The implicit
        // path is `/`, so the result is `/` + R.path. This is
        // the arm the HN bug (`https://news.ycombinator.com` +
        // `hn.js?…`) lands on; the previous implementation
        // landed it on the second clause and treated the host
        // as a "filename" to be replaced.
        format!("/{ref_path}")
    } else {
        // Second clause — base has a path. "All but the last
        // segment" is everything up to and including the
        // right-most '/'. If Base.path has no '/' at all, the
        // result is just R.path (last clause: "excluding the
        // entire base URI path"). Shouldn't happen in practice
        // for HTTP bases (their paths always start with `/`)
        // but the branch is spec-required.
        match base_path.rsplit_once('/') {
            Some((dir, _file)) => format!("{dir}/{ref_path}"),
            None => ref_path.to_string(),
        }
    }
}

/// "R has a scheme" detection per
/// [RFC 3986 § 3.1](https://datatracker.ietf.org/doc/html/rfc3986#section-3.1).
///
/// > "Scheme names consist of a sequence of characters
/// >  beginning with a letter and followed by any combination
/// >  of letters, digits, plus ('+'), period ('.'), or hyphen
/// >  ('-')."
///
/// Returns `true` iff the reference starts with a valid
/// scheme followed by `:`. Tighter than the prior
/// `starts_with("http://" | …)` check — accepts `mailto:`,
/// `javascript:`, `blob:`, etc. without enumerating them, and
/// correctly rejects strings like `foo/bar:baz` (the `:` is
/// inside the path, not after a scheme).
fn has_scheme(href: &str) -> bool {
    let mut chars = href.char_indices();
    let Some((_, first)) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    for (_, c) in chars {
        match c {
            ':' => return true,
            'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '.' => {}
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::resolve_url;

    // Sanity arms — pre-existing behavior we don't want to
    // regress while fixing the bare-name-relative bug.

    #[test]
    fn absolute_http_passes_through() {
        assert_eq!(
            resolve_url("http://example.com/x.js", Some("https://other.test/")),
            "http://example.com/x.js",
        );
    }

    #[test]
    fn absolute_https_passes_through() {
        assert_eq!(
            resolve_url("https://example.com/x.js", Some("https://other.test/")),
            "https://example.com/x.js",
        );
    }

    #[test]
    fn data_url_passes_through() {
        assert_eq!(
            resolve_url("data:text/plain,hello", Some("https://example.com/")),
            "data:text/plain,hello",
        );
    }

    #[test]
    fn protocol_relative_picks_up_base_scheme() {
        assert_eq!(
            resolve_url("//cdn.example.com/x.js", Some("https://example.com/")),
            "https://cdn.example.com/x.js",
        );
    }

    #[test]
    fn absolute_path_replaces_path_under_base_origin() {
        assert_eq!(
            resolve_url("/scripts/x.js", Some("https://example.com/foo/bar.html")),
            "https://example.com/scripts/x.js",
        );
    }

    #[test]
    fn relative_path_joins_against_base_directory() {
        // Base with a file path: replace the file with the relative ref.
        assert_eq!(
            resolve_url("x.js", Some("https://example.com/foo/bar.html")),
            "https://example.com/foo/x.js",
        );
    }

    #[test]
    fn relative_path_joins_against_base_with_trailing_slash() {
        // Base whose path ends in `/` — the empty trailing segment is
        // replaced; trailing slash + file form the joined URL.
        assert_eq!(
            resolve_url("x.js", Some("https://example.com/foo/")),
            "https://example.com/foo/x.js",
        );
    }

    // The bare-name-relative bug.
    //
    // Surfaced loading news.ycombinator.com: HTML has
    //   <script src="hn.js?SMNcJPuowwn2FRyKwpFD">
    // and the base URL is `https://news.ycombinator.com` (no
    // trailing slash, as the user typed it). The current
    // implementation does
    //   `base.rsplit_once('/')`
    // which finds the `/` inside `//` between scheme and host
    // because there's no later one, treating the host
    // `news.ycombinator.com` as a "filename" to be replaced.
    // Result: `https://hn.js?SMNcJPuowwn2FRyKwpFD` — `hn.js`
    // becomes the host.
    //
    // Per RFC 3986 § 5.2.3 (merge): when the base URL has no
    // path component, the relative reference should be merged
    // by prepending `/` — i.e. the implicit base path is `/`.

    #[test]
    fn bare_name_relative_against_authority_only_base() {
        // The HN bug exactly.
        assert_eq!(
            resolve_url(
                "hn.js?SMNcJPuowwn2FRyKwpFD",
                Some("https://news.ycombinator.com"),
            ),
            "https://news.ycombinator.com/hn.js?SMNcJPuowwn2FRyKwpFD",
        );
    }

    #[test]
    fn bare_name_relative_against_authority_with_trailing_slash() {
        // The "right" form of the same base — base ends in `/`
        // after the host. Should produce the same result as the
        assert_eq!(
            resolve_url(
                "hn.js?SMNcJPuowwn2FRyKwpFD",
                Some("https://news.ycombinator.com/"),
            ),
            "https://news.ycombinator.com/hn.js?SMNcJPuowwn2FRyKwpFD",
        );
    }

    #[test]
    fn bare_name_relative_against_http_authority_only_base() {
        // Same bug, http scheme — make sure the fix isn't
        // accidentally https-only.
        assert_eq!(
            resolve_url("foo.js", Some("http://example.com")),
            "http://example.com/foo.js",
        );
    }
}

