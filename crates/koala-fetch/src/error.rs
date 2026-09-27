//! Why a fetch failed.

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
        /// The response body: usually the server's own error page, which a
        /// document load shows in place of the browser's (Chromium's
        /// `HttpErrorNavigationThrottle`, Firefox's `nsURILoader` do the
        /// same). Empty when the server sent none or it could not be read.
        body: Vec<u8>,
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

    /// The URL's scheme is not one koala can load, often a typo such as
    /// `htpps://`.
    #[error(
        "'{url}' uses the '{scheme}' scheme, which koala cannot load \
         (it loads http, https, file, and data URLs)"
    )]
    UnsupportedScheme {
        /// The URL that was requested.
        url: String,
        /// The scheme as written.
        scheme: String,
    },

    /// A `file:` URL that names no file on this machine: malformed, or
    /// with a host, which is what a protocol-relative reference
    /// (`//cdn.example/x.js`) resolves to in a page loaded from disk.
    #[error("'{url}' does not name a file on this machine")]
    InvalidFileUrl {
        /// The URL that was requested.
        url: String,
    },

    /// A [`ReplaySender`](crate::archive::ReplaySender) was asked for a URL
    /// its archive does not hold. Replay never falls back to the network,
    /// so this means the current build requests something the recorded
    /// build did not.
    #[error(
        "'{url}' is not in the replay archive; this build fetches a resource the \
         recording did not (re-record the page if that is expected)"
    )]
    NotInArchive {
        /// The URL that was requested.
        url: String,
    },

    /// The fetch failed when the archive was recorded, and replay
    /// reproduces that failure.
    #[error("'{url}' failed when the archive was recorded: {message}")]
    RecordedFailure {
        /// The URL that was requested.
        url: String,
        /// The error message captured at record time.
        message: String,
    },
}

/// Why a fetch failed, in the terms a user can act on. Browsers show a
/// different error page for each (Chromium's net error codes, Firefox's
/// `aboutNetError` pages); [`FetchError::cause`] maps an error onto them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchCause {
    /// DNS found no address for the host.
    NameNotResolved,
    /// The host answered and refused the connection.
    ConnectionRefused,
    /// The host did not answer in time.
    TimedOut,
    /// Any other failure to reach the host (TLS, reset, unreachable).
    ConnectionFailed,
    /// The server answered with this error status.
    HttpStatus(u16),
    /// A local file does not exist.
    FileNotFound,
    /// The URL's scheme is not one koala loads; carries the scheme.
    UnsupportedScheme(String),
    /// Anything else: malformed input, a replay miss, a failed read.
    Other,
}

impl FetchError {
    /// Classify this error for display.
    ///
    /// Refusals and timeouts are recognized from typed errors in the
    /// cause chain. DNS failures are not typed anywhere reqwest exposes:
    /// hyper-util reports them as a `ConnectError` whose message is
    /// `"dns error"`, so that text is matched; `tests/fetch_cause.rs` pins
    /// it, so a dependency upgrade that changes the wording fails a test
    /// instead of silently turning "host not found" into a generic
    /// connection failure.
    #[must_use]
    pub fn cause(&self) -> FetchCause {
        match self {
            Self::RequestFailed { source, .. } => {
                if source.is_timeout() {
                    return FetchCause::TimedOut;
                }
                let mut next: Option<&(dyn std::error::Error + 'static)> = Some(source);
                while let Some(error) = next {
                    if let Some(io) = error.downcast_ref::<std::io::Error>() {
                        match io.kind() {
                            std::io::ErrorKind::ConnectionRefused => {
                                return FetchCause::ConnectionRefused;
                            }
                            std::io::ErrorKind::TimedOut => return FetchCause::TimedOut,
                            _ => {}
                        }
                    }
                    if error.to_string() == "dns error" {
                        return FetchCause::NameNotResolved;
                    }
                    next = error.source();
                }
                FetchCause::ConnectionFailed
            }
            Self::HttpStatus { status, .. } => FetchCause::HttpStatus(*status),
            Self::LocalRead { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                FetchCause::FileNotFound
            }
            Self::UnsupportedScheme { scheme, .. } => FetchCause::UnsupportedScheme(scheme.clone()),
            Self::HttpClientInit(_)
            | Self::ResponseBody { .. }
            | Self::InvalidDataUrl { .. }
            | Self::Base64Decode(_)
            | Self::LocalRead { .. }
            | Self::InvalidFileUrl { .. }
            | Self::NotInArchive { .. }
            | Self::RecordedFailure { .. } => FetchCause::Other,
        }
    }
}
