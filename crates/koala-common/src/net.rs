//! HTTP fetch utilities for the Koala renderer.
//!
//! Provides simple blocking HTTP GET wrappers used by the document loader,
//! stylesheet fetcher, and image loader.
//!
//! TODO: Implement proper Fetch Standard (<https://fetch.spec.whatwg.org/>)
use base64::Engine;
use std::time::Duration;

/// User-Agent header sent with all requests.
///
/// Mimics a common desktop browser to avoid basic bot detection.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(30);

/// A parsed `data:` URL that can be decoded into raw bytes.
// TODO: Support more media types (e.g. `text/plain`) and image formats (e.g. `image/svg`).
// TODO: Support more metadata options (e.g. charset) and encodings (e.g. percent-encoding).
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
    /// Currently supports base64-encoded data URLs only.
    ///
    /// # Errors
    ///
    /// Returns an error string if base64 decoding fails.
    ///
    /// # Panics
    ///
    /// Panics if the data URL uses an encoding other than base64.
    pub fn decode(&self) -> Result<Vec<u8>, String> {
        let data_url = self.raw_data.trim_start_matches("data:");
        let (metadata, data) = match data_url.find(',') {
            Some(i) => (&data_url[..i], &data_url[i + 1..]),
            None => return Err("Invalid data URL: missing comma".to_string()),
        };

        if metadata.ends_with(";base64") {
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("Base64 decode error: {e}"))
        } else {
            // Percent-decode the data
            panic!("Unrecognized data URL encoding: {metadata}");
        }
    }
}

/// Fetch a URL and return its body as text.
///
/// # Errors
///
/// Returns an error string if the HTTP client cannot be created, the request
/// fails, the response has a non-success status, or the body cannot be decoded.
pub fn fetch_text(url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    response
        .text()
        .map_err(|e| format!("Failed to read response body: {e}"))
}

/// Fetch a URL and return its body as raw bytes.
///
/// # Errors
///
/// Returns an error string if the HTTP client cannot be created, the request
/// fails, the response has a non-success status, or the body cannot be read.
pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read response body: {e}"))
}
/// Decode a `data:` URL and return its payload as raw bytes.
///
/// # Errors
///
/// Returns an error string if the data URL cannot be decoded.
///
/// # Panics
///
/// Panics if `url` does not start with `data:`.
pub fn fetch_bytes_from_data_url(url: &str) -> Result<Vec<u8>, String> {
    assert!(url.starts_with("data:"));
    let data_url = DataURL::new(url.to_string());
    data_url.decode()
}
