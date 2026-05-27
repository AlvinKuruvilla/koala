//! `location` global — the minimal subset of the
//! [`Location`](https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-location-interface)
//! interface that testharness.js relies on for self-identification:
//! `.href`, `.search`, `.pathname`.
//!
//! [§ 7.7.1 The Location interface](https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-location-interface)
//!
//! The full interface has 13+ accessor pairs, `assign` / `replace`
//! / `reload` methods, origin policy checks, and round-trips
//! through the URL parser. We expose three read-only string
//! properties. Anything testharness.js touches beyond these is
//! deferred until a real test demands it.
//!
//! ### Where the URL comes from
//!
//! koala-js doesn't know what URL the document was loaded from —
//! that's koala-browser's responsibility. The runtime stores a
//! `location_href` String on `JsRuntime` and re-installs the
//! `location` global whenever
//! [`crate::JsRuntime::set_location`] is called. The default
//! before any setter call is `about:blank`, matching how a
//! browser presents the global before navigation completes.
//!
//! ### URL parsing
//!
//! The parser here is intentionally minimal — no `url` crate, no
//! IDNA, no percent decoding of the path. It recognises four
//! shapes:
//!
//! - `scheme://authority/path?query#fragment` (HTTP, HTTPS, file)
//! - `scheme:opaque` (data:, javascript:, mailto:)
//! - `about:blank` and other `about:*` forms
//! - Anything else: treat the whole string as the path
//!
//! Output rules match what major browsers expose:
//! - `pathname` is `/` when the authority has no path. Opaque
//!   `data:` / `mailto:` URLs return an empty pathname.
//! - `search` includes the leading `?`, or is the empty string.
//! - `href` round-trips the input verbatim.

use boa_engine::{
    Context, JsResult, JsString, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};

use super::helpers::{getter, js_string_value};

/// Hidden global slot that holds the current `href` string. The
/// accessor getters read this on every property access so a
/// runtime that flips its location via
/// [`crate::JsRuntime::set_location`] doesn't have to rebuild
/// the whole `location` object.
pub(crate) const HREF_KEY: &str = "__koala_location_href__";

/// Default `href` returned before [`crate::JsRuntime::set_location`]
/// has been called. Matches the spec's "initial about:blank
/// Document" state.
pub(crate) const DEFAULT_HREF: &str = "about:blank";

/// Register the `location` global. Called from
/// [`super::register_globals`] once at runtime construction; the
/// underlying `href` slot is later mutated by
/// [`crate::JsRuntime::set_location`] without re-registering.
pub(super) fn register_location(context: &mut Context) {
    context
        .register_global_property(
            js_string!(HREF_KEY),
            JsString::from(DEFAULT_HREF),
            Attribute::all(),
        )
        .expect("__koala_location_href__ should not already exist");

    let href_getter = getter(context, href_get);
    let search_getter = getter(context, search_get);
    let pathname_getter = getter(context, pathname_get);
    let to_string_fn = NativeFunction::from_copy_closure(to_string_native);

    let accessor_attrs = Attribute::CONFIGURABLE | Attribute::ENUMERABLE;

    let location = ObjectInitializer::new(context)
        .accessor(
            js_string!("href"),
            Some(href_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("search"),
            Some(search_getter),
            None,
            accessor_attrs,
        )
        .accessor(
            js_string!("pathname"),
            Some(pathname_getter),
            None,
            accessor_attrs,
        )
        .function(to_string_fn, js_string!("toString"), 0)
        .build();

    context
        .register_global_property(
            js_string!("location"),
            location,
            Attribute::all(),
        )
        .expect("`location` global should not already exist");
}

/// `location.href` — the current full URL string. Read from the
/// hidden `__koala_location_href__` slot on every access.
fn href_get(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    Ok(js_string_value(&read_href(context)?))
}

/// `location.search` — the query string portion of the URL,
/// including the leading `?`. Empty when there is no query.
fn search_get(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let href = read_href(context)?;
    Ok(js_string_value(parse_search(&href)))
}

/// `location.pathname` — the path component of the URL. Falls
/// back to `/` for hierarchical URLs without an explicit path,
/// and to the empty string for opaque schemes like `data:` and
/// `about:`.
fn pathname_get(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let href = read_href(context)?;
    Ok(js_string_value(parse_pathname(&href)))
}

/// `location.toString()` returns `href` per spec — implemented as
/// a method rather than relying on Symbol.toPrimitive coercion so
/// the existing accessor-only ObjectInitializer pattern carries.
fn to_string_native(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    Ok(js_string_value(&read_href(context)?))
}

/// Read the hidden href slot off the global object.
fn read_href(context: &mut Context) -> JsResult<String> {
    let global = context.global_object();
    let value = global.get(js_string!(HREF_KEY), context)?;
    Ok(value.to_string(context)?.to_std_string_escaped())
}

/// Split a URL string into its `?query` portion, returning the
/// portion INCLUDING the leading `?`, or `""` when no query is
/// present. The fragment (`#…`) is stripped before searching so
/// `https://example.com/foo?a=1#frag` yields `?a=1`.
///
/// Pure function exposed at module scope so the unit tests below
/// can exercise the URL-parsing edge cases without going through
/// Boa.
pub(crate) fn parse_search(href: &str) -> &str {
    let before_fragment = href.split_once('#').map_or(href, |(left, _)| left);
    match before_fragment.find('?') {
        Some(i) => &before_fragment[i..],
        None => "",
    }
}

/// Extract the `pathname` portion of `href`. The rules differ by
/// scheme shape and are spelled out in the module-level comment.
pub(crate) fn parse_pathname(href: &str) -> &str {
    // Strip fragment and query first — neither contributes to the
    // pathname.
    let before_fragment = href.split_once('#').map_or(href, |(left, _)| left);
    let before_query = before_fragment
        .split_once('?')
        .map_or(before_fragment, |(left, _)| left);

    // Try the `scheme://authority/path` shape. The authority
    // portion ends at the first `/` after the `//`, and what
    // follows is the pathname.
    if let Some(after_scheme) = scheme_slash_slash_remainder(before_query) {
        return match after_scheme.find('/') {
            Some(i) => &after_scheme[i..],
            None => "/",
        };
    }

    // `scheme:opaque` (data:, mailto:, javascript:, about:) — no
    // hierarchical path, so the spec exposes an empty pathname.
    if has_scheme_prefix(before_query) {
        return "";
    }

    // Nothing recognisable — treat the whole string as the path.
    // Real browsers would refuse this URL entirely; we'd rather
    // return something sensible than panic during a WPT run.
    before_query
}

/// Returns the portion of `href` after `scheme://` when the URL
/// has that shape, or `None` otherwise. The scheme part is not
/// validated for character set beyond "ascii alphanumeric plus
/// `+ - .`" — sufficient for the schemes WPT actually serves.
fn scheme_slash_slash_remainder(href: &str) -> Option<&str> {
    let (scheme, rest) = href.split_once("://")?;
    if scheme.is_empty() || !scheme.bytes().all(is_scheme_byte) {
        return None;
    }
    Some(rest)
}

/// Returns true when `href` starts with `scheme:` for any valid
/// scheme prefix (no `://` required). Used to detect opaque-style
/// URLs like `data:text/plain,foo` or `about:blank`.
fn has_scheme_prefix(href: &str) -> bool {
    let Some((scheme, _)) = href.split_once(':') else {
        return false;
    };
    !scheme.is_empty() && scheme.bytes().all(is_scheme_byte)
}

const fn is_scheme_byte(b: u8) -> bool {
    matches!(b,
        b'a'..=b'z'
        | b'A'..=b'Z'
        | b'0'..=b'9'
        | b'+'
        | b'-'
        | b'.'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathname_for_http_url_with_path() {
        assert_eq!(parse_pathname("https://example.com/foo/bar"), "/foo/bar");
    }

    #[test]
    fn pathname_for_http_url_with_authority_only_returns_slash() {
        assert_eq!(parse_pathname("https://example.com"), "/");
        assert_eq!(parse_pathname("https://example.com/"), "/");
    }

    #[test]
    fn pathname_strips_query_and_fragment() {
        assert_eq!(
            parse_pathname("https://example.com/path?q=1#frag"),
            "/path",
        );
        assert_eq!(parse_pathname("https://example.com/path#frag"), "/path");
    }

    #[test]
    fn pathname_for_file_url() {
        assert_eq!(parse_pathname("file:///tmp/index.html"), "/tmp/index.html");
    }

    #[test]
    fn pathname_for_opaque_schemes_is_empty() {
        assert_eq!(parse_pathname("data:text/plain,hi"), "");
        assert_eq!(parse_pathname("about:blank"), "");
        assert_eq!(parse_pathname("mailto:user@example.com"), "");
    }

    #[test]
    fn search_extracts_query_with_leading_question_mark() {
        assert_eq!(parse_search("https://example.com/path?a=1&b=2"), "?a=1&b=2");
    }

    #[test]
    fn search_is_empty_when_no_query_present() {
        assert_eq!(parse_search("https://example.com/path"), "");
        assert_eq!(parse_search("about:blank"), "");
    }

    #[test]
    fn search_strips_fragment_before_extracting_query() {
        // Fragments come AFTER queries in the URL — but if the
        // raw text has `#...?...` we still must not return the
        // `?` from inside the fragment.
        assert_eq!(parse_search("https://example.com/p#frag?notquery"), "");
        // Real-world shape: query precedes fragment.
        assert_eq!(parse_search("https://example.com/p?q=1#f"), "?q=1");
    }
}
