//! Resource fetch layer for the Koala renderer.
//!
//! Every load of an external resource — the top-level HTML document, external
//! stylesheets, `<script src>`, `<img src>`, `<link rel="icon">` etc. — goes
//! through a single [`RequestSender`] trait so callers can compose alternative
//! implementations without touching the loaders. Concretely:
//!
//! - [`DefaultSender`] dispatches on the URL scheme and runs the real
//!   network / data-URL / filesystem read. This is what production uses.
//! - [`MappedSender`] overlays a URL-to-local-path map on top of an inner
//!   sender so debugging tools can substitute instrumented copies of
//!   third-party JS / CSS without rehosting the page.
//!
//! Installation uses the same thread-local + RAII guard pattern as
//! [`koala_js::dom_handle`] (see `pattern_thread_local_guard.md`): construct
//! a sender, call [`install_sender`] in the scope you want it active, drop
//! the returned [`SenderGuard`] to restore whatever sender was active before.
//!
//! The free functions [`fetch_text`] / [`fetch_bytes`] /
//! [`fetch_bytes_from_data_url`] are thin wrappers that consult the active
//! sender (defaulting to [`DefaultSender`] when none is installed), preserved
//! so existing call sites don't need to know about the trait.
//!
//! TODO: Implement proper Fetch Standard (<https://fetch.spec.whatwg.org/>).
use base64::Engine;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// User-Agent header sent with all requests.
///
/// Mimics a common desktop browser to avoid basic bot detection.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Error type for network fetch and data-URL decode operations.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// Failed to build the HTTP client (e.g. TLS backend unavailable).
    #[error("failed to create HTTP client: {0}")]
    HttpClientInit(#[source] reqwest::Error),

    /// The HTTP request could not be sent.
    #[error("request to '{url}' failed: {source}")]
    RequestFailed {
        /// The URL that was requested.
        url: String,
        /// The underlying transport error.
        #[source]
        source: reqwest::Error,
    },

    /// The server returned a non-success status code.
    #[error("HTTP {status} for '{url}'")]
    HttpStatus {
        /// The URL that was requested.
        url: String,
        /// The HTTP status code.
        status: u16,
    },

    /// The response body could not be read.
    #[error("failed to read response body from '{url}': {source}")]
    ResponseBody {
        /// The URL that was requested.
        url: String,
        /// The underlying I/O or decoding error.
        #[source]
        source: reqwest::Error,
    },

    /// The data URL is malformed (e.g. missing the `,` separator).
    #[error("invalid data URL: {reason}")]
    InvalidDataUrl {
        /// A human-readable explanation of what is wrong.
        reason: String,
    },

    /// Base64 payload in a data URL could not be decoded.
    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    /// A local-file fetch failed. Used both for `file://` URLs and for
    /// plain absolute paths handled by [`DefaultSender`].
    #[error("local read of '{path}' failed: {source}")]
    LocalRead {
        /// The path (URL or filesystem path) that was requested.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// A parsed `data:` URL that can be decoded into raw bytes.
// TODO: Support more media types (e.g. `text/plain`) and image formats (e.g. `image/svg`).
// TODO: Consider using a proper URL parser instead of manual string manipulation.
// TODO: Consider caching decoded data URLs to avoid redundant decoding.
// TODO: Consider implementing `Display` and `Debug` for better error messages and logging.
// TODO: Consider implementing `FromStr` for easier construction from string literals.
// TODO: Consider implementing `Into<Vec<u8>>` for easier integration with other APIs that consume byte data.
// TODO: Consider implementing `PartialEq` and `Eq` for easier testing and deduplication of data URLs.
// TODO: Consider implementing `Clone` and `Copy` if the struct is small enough to be cheaply duplicated.
// TODO: Add helper methods to extract metadata (e.g. media type) from the data URL for more advanced use cases.
pub struct DataURL {
    /// The full raw `data:` URL string (e.g. `data:image/png;base64,...`).
    pub raw_data: String,
}
impl DataURL {
    /// Create a new `DataURL` from a raw data URL string.
    #[must_use]
    pub const fn new(raw_data: String) -> Self {
        Self { raw_data }
    }
    /// Decode the data URL payload into raw bytes.
    ///
    /// Handles both encodings defined by
    /// [RFC 2397](https://datatracker.ietf.org/doc/html/rfc2397):
    /// metadata ending in `;base64` is base64-decoded, anything
    /// else is percent-decoded. The mediatype itself (e.g.
    /// `text/javascript`, `image/png;charset=utf-8`) is ignored
    /// — callers downstream are responsible for interpreting the
    /// payload bytes.
    ///
    /// Percent-decoding is **lenient** to match what browsers do
    /// (and what the WHATWG URL spec recommends for the
    /// `application/x-www-form-urlencoded` parser): a `%`
    /// followed by anything other than two hex digits is passed
    /// through literally rather than treated as an error. This
    /// is the only practical choice — real-world data URLs in
    /// the wild contain stray `%` characters in literal text.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::InvalidDataUrl`] if the URL is
    /// missing the comma separator, or [`FetchError::Base64Decode`]
    /// if a `;base64`-marked payload fails to decode.
    pub fn decode(&self) -> Result<Vec<u8>, FetchError> {
        let data_url = self.raw_data.trim_start_matches("data:");
        let (metadata, data) = match data_url.find(',') {
            Some(i) => (&data_url[..i], &data_url[i + 1..]),
            None => {
                return Err(FetchError::InvalidDataUrl {
                    reason: "missing comma".to_string(),
                })
            }
        };

        if metadata.ends_with(";base64") {
            Ok(base64::engine::general_purpose::STANDARD.decode(data)?)
        } else {
            Ok(percent_decode(data))
        }
    }
}

/// Lenient percent-decode of `s` per RFC 3986 § 2.1. `%XX`
/// triples where `XX` is two hex digits decode to a single byte;
/// every other byte is copied through unchanged, including stray
/// `%` characters whose two-character tail isn't valid hex.
///
/// Returns raw bytes — the caller decides whether to interpret
/// them as UTF-8 text (`String::from_utf8_lossy`) or as opaque
/// binary (image bytes etc.). Note that this is *not* the
/// `application/x-www-form-urlencoded` rule: `+` is left as-is
/// rather than turned into a space, matching RFC 2397's
/// reference to RFC 2396 percent-encoding.
fn percent_decode(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

const fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Abstraction over "go get the bytes at this address."
///
/// Implementations decide whether to hit the network, read a local file,
/// decode a `data:` URL, return a canned response, or delegate to another
/// sender. The loaders in `koala-browser` and `koala-css` only see this
/// trait, so substitution is a matter of installing a different sender
/// — no fetch-call-site changes.
///
/// Implementations must be safe to call from the thread that installed
/// them; they don't need to be `Send`.
pub trait RequestSender {
    /// Fetch the resource at `url` and return its body as raw bytes.
    ///
    /// `url` may be an `http(s)://` URL, a `data:` URL, a `file://` URL,
    /// or a plain filesystem path — the implementation decides which
    /// schemes it handles.
    ///
    /// # Errors
    ///
    /// Returns a [`FetchError`] if the resource cannot be fetched,
    /// decoded, or read.
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError>;
}

/// Production sender. Dispatches on the URL scheme:
///
/// - `data:` → decode in-process via [`DataURL`].
/// - `http://` / `https://` → blocking HTTP GET via `reqwest`, honoring
///   the WPT [`hosts`](crate::hosts) overrides.
/// - anything else → treated as a filesystem path (with an optional
///   `file://` prefix stripped).
///
/// Stateless. Constructing one is free; you don't need to cache the
/// instance.
pub struct DefaultSender;

impl RequestSender for DefaultSender {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        if url.starts_with("data:") {
            return DataURL::new(url.to_string()).decode();
        }
        if url.starts_with("http://") || url.starts_with("https://") {
            return http_fetch(url);
        }
        let path = url.strip_prefix("file://").unwrap_or(url);
        std::fs::read(path).map_err(|e| FetchError::LocalRead {
            path: url.to_string(),
            source: e,
        })
    }
}

/// Sender that consults a URL → local-file map before delegating to an
/// inner sender. Used by debug tooling to substitute instrumented copies
/// of third-party JS / CSS without rehosting the page.
///
/// Lookups are exact-string matches against the URL as the loader sees
/// it — the same URL `<script src>` or `<link href>` resolved to, post
/// base-URL resolution. Loaders that fetch from CDN URLs need the
/// override key to be the resolved CDN URL, not the relative reference.
pub struct MappedSender<I> {
    inner: I,
    overrides: HashMap<String, PathBuf>,
}

impl<I: RequestSender> MappedSender<I> {
    /// Construct a new overlay that delegates everything to `inner`.
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            overrides: HashMap::new(),
        }
    }

    /// Add a URL → local-file mapping. Subsequent fetches of `url`
    /// return the bytes of `path` instead of going through `inner`.
    /// Chainable.
    #[must_use]
    pub fn map(mut self, url: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let _ = self.overrides.insert(url.into(), path.into());
        self
    }
}

impl<I: RequestSender> RequestSender for MappedSender<I> {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        if let Some(path) = self.overrides.get(url) {
            return std::fs::read(path).map_err(|e| FetchError::LocalRead {
                path: path.to_string_lossy().into_owned(),
                source: e,
            });
        }
        self.inner.fetch(url)
    }
}

thread_local! {
    /// Thread-local active sender. `None` falls back to [`DefaultSender`].
    /// Set via [`install_sender`], cleared when the returned guard drops.
    static ACTIVE_SENDER: RefCell<Option<Box<dyn RequestSender>>> = const { RefCell::new(None) };
}

/// Install `sender` as the active sender for this thread. The previous
/// sender is restored when the returned [`SenderGuard`] is dropped.
///
/// Guards nest: installing while one is already active stashes the
/// previous sender and restores it on drop, so callers can locally
/// override without disturbing whatever the outer scope had set up.
#[must_use = "the guard restores the previous sender on drop"]
pub fn install_sender(sender: Box<dyn RequestSender>) -> SenderGuard {
    let previous = ACTIVE_SENDER.with_borrow_mut(|slot| slot.replace(sender));
    SenderGuard { previous }
}

/// RAII guard returned by [`install_sender`]. Restores the previous
/// active sender on drop.
pub struct SenderGuard {
    previous: Option<Box<dyn RequestSender>>,
}

impl Drop for SenderGuard {
    fn drop(&mut self) {
        ACTIVE_SENDER.with_borrow_mut(|slot| *slot = self.previous.take());
    }
}

/// Run `f` with a reference to the currently-active sender — the one
/// installed by [`install_sender`] on this thread, falling back to
/// [`DefaultSender`] if none is installed.
fn with_active_sender<R>(f: impl FnOnce(&dyn RequestSender) -> R) -> R {
    ACTIVE_SENDER.with_borrow(|slot| match slot {
        Some(sender) => f(&**sender),
        None => f(&DefaultSender),
    })
}

/// Shared HTTP body fetch used by [`DefaultSender`]. Separated so the
/// trait impl reads as a three-arm scheme dispatch.
fn http_fetch(url: &str) -> Result<Vec<u8>, FetchError> {
    let client = crate::hosts::apply(reqwest::blocking::Client::builder().timeout(TIMEOUT))
        .build()
        .map_err(FetchError::HttpClientInit)?;

    let response = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| FetchError::RequestFailed {
            url: url.to_string(),
            source: e,
        })?;

    if !response.status().is_success() {
        return Err(FetchError::HttpStatus {
            url: url.to_string(),
            status: response.status().as_u16(),
        });
    }

    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|e| FetchError::ResponseBody {
            url: url.to_string(),
            source: e,
        })
}

/// Fetch the resource at `url` and return its body as text. Delegates
/// to the active [`RequestSender`]; bytes are decoded with
/// [`String::from_utf8_lossy`].
///
/// # Errors
///
/// Returns a [`FetchError`] if the underlying fetch fails.
pub fn fetch_text(url: &str) -> Result<String, FetchError> {
    let bytes = fetch_bytes(url)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Fetch the resource at `url` and return its body as raw bytes.
/// Delegates to the active [`RequestSender`].
///
/// # Errors
///
/// Returns a [`FetchError`] if the underlying fetch fails.
pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, FetchError> {
    with_active_sender(|s| s.fetch(url))
}

/// Decode a `data:` URL directly, bypassing the active sender. Kept as
/// a public function for callers that want to short-circuit the scheme
/// dispatch when they know the input is a data URL.
///
/// # Errors
///
/// Returns a [`FetchError`] if the data URL cannot be decoded.
///
/// # Panics
///
/// Panics if `url` does not start with `data:`.
pub fn fetch_bytes_from_data_url(url: &str) -> Result<Vec<u8>, FetchError> {
    assert!(url.starts_with("data:"));
    DataURL::new(url.to_string()).decode()
}

#[cfg(test)]
mod data_url_tests {
    use super::*;

    fn decode(url: &str) -> Result<Vec<u8>, FetchError> {
        DataURL::new(url.to_string()).decode()
    }

    fn decode_str(url: &str) -> String {
        String::from_utf8(decode(url).expect("decode should succeed")).unwrap()
    }

    #[test]
    fn base64_data_url_still_decodes() {
        // Body: "hello"
        assert_eq!(
            decode_str("data:text/plain;base64,aGVsbG8="),
            "hello",
        );
    }

    #[test]
    fn plain_text_data_url_passes_through_unescaped() {
        // No metadata, no encoding marker — the comma-separated
        // tail is returned verbatim.
        assert_eq!(decode_str("data:,abc"), "abc");
    }

    #[test]
    fn percent_decoded_payload_with_javascript_mediatype() {
        // The common WPT shape: `data:text/javascript,` with a
        // percent-encoded body. `%20` → space, `%27` → `'`.
        assert_eq!(
            decode_str("data:text/javascript,globalThis.x%20=%20'hi'"),
            "globalThis.x = 'hi'",
        );
    }

    #[test]
    fn percent_decode_handles_mixed_case_hex() {
        // Both uppercase and lowercase hex digits are valid.
        assert_eq!(decode_str("data:,%2f%2F"), "//");
    }

    #[test]
    fn lone_percent_at_end_is_kept_literally() {
        // A `%` without two trailing hex digits is passed through
        // unchanged — the lenient WHATWG-style policy. Anything
        // stricter would error on real-world inputs that contain
        // literal `%` signs.
        assert_eq!(decode_str("data:,50%"), "50%");
    }

    #[test]
    fn percent_followed_by_non_hex_is_kept_literally() {
        // `%XY` where one of X or Y isn't a hex digit also
        // passes through — same lenient policy.
        assert_eq!(decode_str("data:,a%Zb"), "a%Zb");
    }

    #[test]
    fn missing_comma_is_an_error() {
        let err = decode("data:text/plain").unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("missing comma"),
            "expected 'missing comma' in error, got: {message}",
        );
    }
}
