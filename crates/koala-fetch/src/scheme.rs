//! Dispatching a fetch on its URL's scheme.

use std::path::PathBuf;

use crate::http::http_fetch;
use crate::{DataURL, FetchError, RequestSender};

/// Production sender. Dispatches on the URL scheme:
///
/// - `data:` → decode in-process via [`DataURL`].
/// - `http://` / `https://` → blocking HTTP GET via `reqwest`, honoring
///   the WPT [`hosts`](crate::hosts) overrides.
/// - `file:` → read the file the URL names on this machine.
/// - no scheme → a filesystem path.
/// - any other scheme → [`FetchError::UnsupportedScheme`].
///
/// Schemes match ASCII case-insensitively, as the URL Standard requires:
/// `HTTPS://a.test/` is fetched like `https://a.test/`.
///
/// Stateless. Constructing one is free; you don't need to cache the
/// instance.
pub struct DefaultSender;

impl RequestSender for DefaultSender {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let written = koala_common::url::scheme(url);
        let path = match written.map(str::to_ascii_lowercase).as_deref() {
            None => PathBuf::from(url),
            // `DataURL` expects the lower-case prefix; `data:` is five bytes.
            Some("data") => return DataURL::new(format!("data:{}", &url[5..])).decode(),
            Some("http" | "https") => return http_fetch(url),
            Some("file") => ::url::Url::parse(url)
                .ok()
                .and_then(|parsed| parsed.to_file_path().ok())
                .ok_or_else(|| FetchError::InvalidFileUrl {
                    url: url.to_string(),
                })?,
            // On Windows, `C:\page.html` parses as scheme `c`; a single
            // letter is a drive, not a scheme.
            Some(drive) if cfg!(windows) && drive.len() == 1 => PathBuf::from(url),
            Some(_) => {
                return Err(FetchError::UnsupportedScheme {
                    url: url.to_string(),
                    scheme: written.unwrap_or_default().to_string(),
                });
            }
        };
        std::fs::read(&path).map_err(|e| FetchError::LocalRead {
            path: url.to_string(),
            source: e,
        })
    }
}
