//! The seam every fetch goes through, and the sender active on this thread.

use std::cell::RefCell;
use std::path::PathBuf;

use koala_std::collections::HashMap;

use crate::{DefaultSender, FetchError};

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
    ///
    /// NOTE: a fetch made on a thread with no sender installed uses
    /// `DefaultSender` and goes to the live network, even under
    /// `--replay`. Moving any loader onto a worker thread means
    /// installing the caller's sender on that thread too.
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
pub(crate) fn with_active_sender<R>(f: impl FnOnce(&dyn RequestSender) -> R) -> R {
    ACTIVE_SENDER.with_borrow(|slot| match slot {
        Some(sender) => f(&**sender),
        None => f(&DefaultSender),
    })
}
