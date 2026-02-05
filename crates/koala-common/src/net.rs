//! HTTP fetch utilities for the Koala browser.
//!
//! Provides simple blocking HTTP GET wrappers used by the document loader,
//! stylesheet fetcher, and image loader.
//!
//! TODO: Implement proper Fetch Standard (<https://fetch.spec.whatwg.org/>)

use std::time::Duration;

/// User-Agent header sent with all requests.
///
/// Mimics a common desktop browser to avoid basic bot detection.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(30);

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
