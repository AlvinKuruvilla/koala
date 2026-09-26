//! Resource fetching for koala: the pieces of the
//! [Fetch Standard](https://fetch.spec.whatwg.org/) koala implements, and the
//! seam every fetch goes through.
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

mod data_url;
mod error;
/// WPT-style hosts-file DNS overrides used when running under wptrunner.
pub mod hosts;
mod http;
mod scheme;
mod sender;

pub use data_url::{DataURL, fetch_bytes_from_data_url};
pub use error::{FetchCause, FetchError};
pub use scheme::DefaultSender;
pub use sender::{MappedSender, RequestSender, SenderGuard, install_sender};

use sender::with_active_sender;

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
