//! A recorded set of fetch responses, and the senders that write and serve
//! it.
//!
//! Measuring a live page mixes network variance into every sample, and the
//! page itself changes between runs. An [`Archive`] freezes what a load
//! fetched: [`RecordingSender`] wraps a real sender and keeps every response
//! it returns, and [`ReplaySender`] serves those responses back without
//! touching the network. Two builds replaying one archive therefore see
//! byte-identical inputs.
//!
//! Recording goes through koala's own fetch path ([`RequestSender`]), so the
//! archive holds exactly the URLs koala requested, not what another browser
//! would have.
//!
//! # Format
//!
//! One JSON file. Entries are keyed by URL as the loader requested it, after
//! base-URL resolution:
//!
//! ```text
//! {
//!   "format": "koala-fetch-archive",
//!   "version": 2,
//!   "entries": {
//!     "https://example.com/":         { "body": "<base64>", "sha256": "<hex>" },
//!     "https://example.com/missing":  { "body": "<base64>", "sha256": "<hex>", "status": 404 },
//!     "https://example.com/gone.css": { "error": "request to '...' failed: ..." }
//!   }
//! }
//! ```
//!
//! - **`body`**: the response bytes, base64-encoded because scripts and
//!   images are not valid UTF-8 in general.
//! - **`sha256`**: of the decoded body. [`Archive::load`] checks it, so a
//!   truncated or hand-edited archive is rejected rather than replayed.
//!   It also lets tools outside koala compare archives without decoding.
//! - **`status`**: present when the server answered with an error status;
//!   `body` is then the server's error page. Replay returns the same status
//!   and body, so a page that loads a server's 404 page still does.
//! - **`error`**: the fetch failed while recording. Replay fails the same
//!   fetch with the same message, so a page with a broken stylesheet stays
//!   broken instead of silently gaining or losing a resource.
//!
//! `data:` URLs are never recorded; they carry their own bytes and are
//! decoded directly by both senders.
//!
//! # Replay misses
//!
//! A URL absent from the archive is an error ([`FetchError::NotInArchive`]),
//! never a fallback to the network. A miss means the build under test
//! requests something the recorded build did not, which is itself a result
//! the caller should see.
//!
//! # Trade-offs
//!
//! - **Size**: base64 inflates bodies by a third, and the whole archive is
//!   held in memory. Pages are a few MB, and one file is easier to move and
//!   inspect than a directory of blobs.
//! - **Bodies only**: headers, status codes, and redirects are not kept,
//!   because [`RequestSender`] returns bytes and no loader reads headers
//!   yet. When one does (content-type sniffing, caching), the format needs a
//!   new version.
//! - **First response wins**: a URL fetched twice while recording keeps its
//!   first body. Replay is deterministic either way; keeping the first
//!   matches what the first load of the page saw.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use koala_fetch::{DataURL, FetchError, RequestSender};

/// Value of the `format` field; anything else is not an archive.
const FORMAT: &str = "koala-fetch-archive";

/// Current format version. Bump when the file layout changes.
///
/// - 1: `body` and `error` entries.
/// - 2: adds `status` to body entries, for HTTP error responses. Version 1
///   files are still read: they have no status entries to misread.
const VERSION: u32 = 2;

/// The oldest version `Archive::load` accepts.
const OLDEST_VERSION: u32 = 1;

/// Errors reading or writing an [`Archive`] file.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// The archive file could not be read or written.
    #[error("cannot access archive '{path}': {source}")]
    Io {
        /// The archive path.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The file is not valid JSON, or not shaped like an archive.
    #[error("'{path}' is not a valid fetch archive: {source}")]
    Parse {
        /// The archive path.
        path: String,
        /// The JSON error.
        #[source]
        source: serde_json::Error,
    },

    /// The `format` or `version` field does not match this build.
    #[error(
        "'{path}' has format '{format}' version {version}; this build reads \
         '{FORMAT}' versions {OLDEST_VERSION} to {VERSION} (re-record the page)"
    )]
    Unsupported {
        /// The archive path.
        path: String,
        /// The `format` field found in the file.
        format: String,
        /// The `version` field found in the file.
        version: u32,
    },

    /// An entry's body failed to decode or does not match its `sha256`.
    #[error("'{path}' is corrupt at entry '{url}': {reason} (re-record the page)")]
    Corrupt {
        /// The archive path.
        path: String,
        /// The entry's URL.
        url: String,
        /// What failed to check out.
        reason: String,
    },
}

/// A recorded set of fetch responses, keyed by URL. See the
/// [module documentation](self) for the file format.
#[derive(Debug, Default)]
pub struct Archive {
    // BTreeMap so `save` writes entries in a stable order and two
    // recordings of the same page diff cleanly.
    entries: BTreeMap<String, Entry>,
}

/// One recorded fetch outcome.
#[derive(Debug, Clone)]
enum Entry {
    /// The fetch succeeded with this body.
    Body(Vec<u8>),
    /// The server answered with an error status and this body.
    HttpError { status: u16, body: Vec<u8> },
    /// The fetch failed with this message.
    Failed(String),
}

/// On-disk shape of an archive. Kept separate from [`Archive`] so the
/// in-memory form holds decoded bytes and replay never re-decodes base64.
#[derive(Serialize, Deserialize)]
struct WireArchive {
    format: String,
    version: u32,
    entries: BTreeMap<String, WireEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WireEntry {
    Body {
        body: String,
        sha256: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
    },
    Failed { error: String },
}

impl Archive {
    /// An empty archive.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of recorded URLs, failures included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The recorded failures, as `(url, description)` in URL order: fetches
    /// that failed, and responses with an HTTP error status.
    pub fn failures(&self) -> impl Iterator<Item = (&str, String)> {
        self.entries.iter().filter_map(|(url, entry)| match entry {
            Entry::Failed(message) => Some((url.as_str(), message.clone())),
            Entry::HttpError { status, .. } => Some((url.as_str(), format!("HTTP {status}"))),
            Entry::Body(_) => None,
        })
    }

    /// Read an archive from `path`, checking every body against its
    /// recorded SHA-256.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError`] if the file cannot be read, is not an
    /// archive of this format and version, or has a body that fails to
    /// decode or verify.
    pub fn load(path: &Path) -> Result<Self, ArchiveError> {
        let display = path.display().to_string();
        let text = std::fs::read_to_string(path).map_err(|source| ArchiveError::Io {
            path: display.clone(),
            source,
        })?;
        let wire: WireArchive = serde_json::from_str(&text).map_err(|source| ArchiveError::Parse {
            path: display.clone(),
            source,
        })?;
        if wire.format != FORMAT || !(OLDEST_VERSION..=VERSION).contains(&wire.version) {
            return Err(ArchiveError::Unsupported {
                path: display,
                format: wire.format,
                version: wire.version,
            });
        }

        let mut entries = BTreeMap::new();
        for (url, wire_entry) in wire.entries {
            let entry = match wire_entry {
                WireEntry::Body {
                    body,
                    sha256,
                    status,
                } => {
                    let bytes = BASE64.decode(body).map_err(|e| ArchiveError::Corrupt {
                        path: display.clone(),
                        url: url.clone(),
                        reason: format!("body is not valid base64: {e}"),
                    })?;
                    let actual = sha256_hex(&bytes);
                    if actual != sha256 {
                        return Err(ArchiveError::Corrupt {
                            path: display,
                            url,
                            reason: format!("body hashes to {actual}, recorded {sha256}"),
                        });
                    }
                    match status {
                        None => Entry::Body(bytes),
                        Some(status) => Entry::HttpError {
                            status,
                            body: bytes,
                        },
                    }
                }
                WireEntry::Failed { error } => Entry::Failed(error),
            };
            let _ = entries.insert(url, entry);
        }
        Ok(Self { entries })
    }

    /// Write the archive to `path` as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::Io`] if the file cannot be written.
    ///
    /// # Panics
    ///
    /// Panics if serialization fails, which cannot happen: every field is a
    /// string, integer, or map with string keys.
    pub fn save(&self, path: &Path) -> Result<(), ArchiveError> {
        let wire = WireArchive {
            format: FORMAT.to_string(),
            version: VERSION,
            entries: self
                .entries
                .iter()
                .map(|(url, entry)| {
                    let wire_entry = match entry {
                        Entry::Body(bytes) => WireEntry::Body {
                            body: BASE64.encode(bytes),
                            sha256: sha256_hex(bytes),
                            status: None,
                        },
                        Entry::HttpError { status, body } => WireEntry::Body {
                            body: BASE64.encode(body),
                            sha256: sha256_hex(body),
                            status: Some(*status),
                        },
                        Entry::Failed(message) => WireEntry::Failed {
                            error: message.clone(),
                        },
                    };
                    (url.clone(), wire_entry)
                })
                .collect(),
        };
        let text = serde_json::to_string_pretty(&wire)
            .expect("archive fields are strings, integers, and string-keyed maps");
        std::fs::write(path, text).map_err(|source| ArchiveError::Io {
            path: path.display().to_string(),
            source,
        })
    }
}

/// Sender that forwards to `inner` and records every outcome into an
/// [`Archive`].
///
/// Clones share one archive, so install a clone and keep the original to
/// read the recording back (see `record` in `main.rs`).
pub struct RecordingSender<I> {
    inner: Rc<I>,
    archive: Rc<RefCell<Archive>>,
}

impl<I> Clone for RecordingSender<I> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            archive: Rc::clone(&self.archive),
        }
    }
}

impl<I: RequestSender> RecordingSender<I> {
    /// Record everything fetched through `inner` into a new, empty archive.
    #[must_use]
    pub fn new(inner: I) -> Self {
        Self {
            inner: Rc::new(inner),
            archive: Rc::new(RefCell::new(Archive::new())),
        }
    }

    /// The recording so far.
    ///
    /// # Panics
    ///
    /// Panics if called from inside a fetch on the same recorder, which the
    /// single-threaded loaders never do.
    #[must_use]
    pub fn archive(&self) -> std::cell::Ref<'_, Archive> {
        self.archive.borrow()
    }
}

impl<I: RequestSender> RequestSender for RecordingSender<I> {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        if url.starts_with("data:") {
            return DataURL::new(url.to_string()).decode();
        }
        let result = self.inner.fetch(url);
        let entry = match &result {
            Ok(bytes) => Entry::Body(bytes.clone()),
            Err(FetchError::HttpStatus { status, body, .. }) => Entry::HttpError {
                status: *status,
                body: body.clone(),
            },
            Err(e) => Entry::Failed(e.to_string()),
        };
        let _ = self
            .archive
            .borrow_mut()
            .entries
            .entry(url.to_string())
            .or_insert(entry);
        result
    }
}

/// Sender that serves fetches from an [`Archive`] and never touches the
/// network.
///
/// It also logs what it served, so [`ReplaySender::input_digest`] can
/// identify the exact inputs a load consumed. Clones share the log; install
/// a clone and keep the original.
#[derive(Clone)]
pub struct ReplaySender {
    archive: Rc<Archive>,
    // URL → what was served for it. BTreeMap so the digest is independent
    // of fetch order.
    served: Rc<RefCell<BTreeMap<String, Served>>>,
}

/// What one replayed URL produced, as far as the digest is concerned.
// Read only by `input_digest`, which only `--bench` reports.
#[cfg_attr(not(feature = "bench"), allow(dead_code))]
#[derive(Debug, Clone)]
enum Served {
    Body(String),
    HttpError(u16, String),
    Failed(String),
    Missing,
}

impl ReplaySender {
    /// Serve fetches from `archive`.
    #[must_use]
    pub fn new(archive: Archive) -> Self {
        Self {
            archive: Rc::new(archive),
            served: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    /// SHA-256 (hex) over every URL served so far and what it produced:
    /// the body's hash, the recorded failure, or a miss.
    ///
    /// Two loads with equal digests consumed identical inputs. Only served
    /// URLs count, so an archive with extra unused entries does not change
    /// the digest, and a build that newly misses a URL does.
    ///
    /// # Panics
    ///
    /// Panics if called from inside a fetch on the same sender, which the
    /// single-threaded loaders never do.
    #[must_use]
    #[cfg_attr(not(feature = "bench"), allow(dead_code))]
    pub fn input_digest(&self) -> String {
        let mut hasher = Sha256::new();
        for (url, served) in self.served.borrow().iter() {
            hasher.update(url.as_bytes());
            hasher.update(b"\0");
            match served {
                Served::Body(sha256) => {
                    hasher.update(b"body:");
                    hasher.update(sha256.as_bytes());
                }
                Served::HttpError(status, sha256) => {
                    hasher.update(format!("http {status}:").as_bytes());
                    hasher.update(sha256.as_bytes());
                }
                Served::Failed(message) => {
                    hasher.update(b"error:");
                    hasher.update(message.as_bytes());
                }
                Served::Missing => hasher.update(b"missing"),
            }
            hasher.update(b"\n");
        }
        hex::encode(hasher.finalize())
    }
}

impl RequestSender for ReplaySender {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        if url.starts_with("data:") {
            return DataURL::new(url.to_string()).decode();
        }
        let (served, result) = match self.archive.entries.get(url) {
            Some(Entry::Body(bytes)) => (Served::Body(sha256_hex(bytes)), Ok(bytes.clone())),
            Some(Entry::HttpError { status, body }) => (
                Served::HttpError(*status, sha256_hex(body)),
                Err(FetchError::HttpStatus {
                    url: url.to_string(),
                    status: *status,
                    body: body.clone(),
                }),
            ),
            Some(Entry::Failed(message)) => (
                Served::Failed(message.clone()),
                Err(FetchError::RecordedFailure {
                    url: url.to_string(),
                    message: message.clone(),
                }),
            ),
            None => (
                Served::Missing,
                Err(FetchError::NotInArchive {
                    url: url.to_string(),
                }),
            ),
        };
        let _ = self.served.borrow_mut().insert(url.to_string(), served);
        result
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
