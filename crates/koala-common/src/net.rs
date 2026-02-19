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

    /// The data URL uses an encoding other than base64.
    #[error("unsupported data URL encoding: {metadata}")]
    UnsupportedDataUrlEncoding {
        /// The metadata portion of the data URL (before the comma).
        metadata: String,
    },
}

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
    /// Returns [`FetchError::InvalidDataUrl`] if the URL is missing the
    /// comma separator, [`FetchError::Base64Decode`] if the base64 payload
    /// is invalid, or [`FetchError::UnsupportedDataUrlEncoding`] if the
    /// data URL uses an encoding other than base64.
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
            // Percent-decode the data
            Err(FetchError::UnsupportedDataUrlEncoding {
                metadata: metadata.to_string(),
            })
        }
    }
}

/// Fetch a URL and return its body as text.
///
/// # Errors
///
/// Returns a [`FetchError`] if the HTTP client cannot be created, the request
/// fails, the response has a non-success status, or the body cannot be decoded.
pub fn fetch_text(url: &str) -> Result<String, FetchError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
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

    response.text().map_err(|e| FetchError::ResponseBody {
        url: url.to_string(),
        source: e,
    })
}

/// Fetch a URL and return its body as raw bytes.
///
/// # Errors
///
/// Returns a [`FetchError`] if the HTTP client cannot be created, the request
/// fails, the response has a non-success status, or the body cannot be read.
pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, FetchError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
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
/// Decode a `data:` URL and return its payload as raw bytes.
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
    let data_url = DataURL::new(url.to_string());
    data_url.decode()
}
