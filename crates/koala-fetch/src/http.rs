//! HTTP(S) fetches over the network.

use std::time::Duration;

use crate::{FetchError, Language, Request};

/// User-Agent header sent with all requests.
///
/// Mimics a common desktop browser to avoid basic bot detection.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Shared HTTP body fetch used by [`DefaultSender`](crate::DefaultSender).
/// Sends the `Accept` header for the request's destination and the
/// `Accept-Language` header for `language` (see [`Request`] and
/// [`Language`]).
#[allow(clippy::disallowed_types)] // the one sanctioned HTTP client
pub(crate) fn http_fetch(request: &Request<'_>, language: Language) -> Result<Vec<u8>, FetchError> {
    let url = request.url;
    let client = crate::hosts::apply(reqwest::blocking::Client::builder().timeout(TIMEOUT))
        .build()
        .map_err(FetchError::HttpClientInit)?;

    let response = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", request.destination.accept())
        .header("Accept-Language", language.accept_language())
        .send()
        .map_err(|e| FetchError::RequestFailed {
            url: url.to_string(),
            source: e,
        })?;

    let status = response.status();
    if !status.is_success() {
        // A body that fails to arrive is treated as no body: the status is
        // the failure being reported, and the caller falls back to its own
        // error page exactly as for an empty one.
        let body = response.bytes().map(|b| b.to_vec()).unwrap_or_default();
        return Err(FetchError::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
            body,
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
