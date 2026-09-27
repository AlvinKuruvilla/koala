//! `data:` URLs, decoded in-process.

use base64::Engine;

use crate::FetchError;

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
