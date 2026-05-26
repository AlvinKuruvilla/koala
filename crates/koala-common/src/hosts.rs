//! WPT-style hosts-file DNS overrides for reqwest.
//!
//! WPT serves tests under `web-platform.test` (and ~60 subdomain
//! variants) by convention. Normally browsers rely on a hosts-file
//! mapping configured at the OS level (`/etc/hosts` on Unix). To
//! avoid requiring users to modify that shared system file, koala
//! supports `koala-cli --hosts-file=PATH`. The contents are
//! installed into a process-wide override table that the reqwest
//! client builder consults via [`apply`] before every fetch.
//!
//! The expected file format matches the output of
//! `wpt make-hosts-file`: lines of `<IP>\t<hostname>` with `#`
//! comments and blank lines ignored. Multiple hostnames may follow
//! the same IP, which mirrors the conventional `/etc/hosts`
//! format.
//!
//! reqwest 0.12 documents that "explicitly specified port in the
//! URL will override any port in the resolved `SocketAddr`s ...
//! port `0` will be replaced by the conventional port for the
//! given scheme" — so this module stores every override with
//! port `0` and lets reqwest pick the right destination port at
//! connect time.

use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::OnceLock;

/// Errors produced while loading a hosts file or installing its
/// contents as the process-wide override set.
#[derive(Debug, thiserror::Error)]
pub enum HostsError {
    /// The hosts file could not be read.
    #[error("could not read hosts file '{path}': {source}")]
    Read {
        /// Path that was attempted.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A line was present but contained no IP token.
    #[error("invalid hosts entry on line {line}: '{content}'")]
    BadLine {
        /// 1-based line number in the source file.
        line: usize,
        /// The raw line content.
        content: String,
    },

    /// The IP address on a line could not be parsed.
    #[error("invalid IP address '{addr}' on line {line}")]
    BadAddress {
        /// 1-based line number in the source file.
        line: usize,
        /// The unparseable address text.
        addr: String,
    },

    /// [`set_from_file`] was called more than once. The process-wide
    /// override table is one-shot.
    #[error("hosts overrides have already been installed")]
    AlreadySet,
}

static OVERRIDES: OnceLock<Vec<(String, SocketAddr)>> = OnceLock::new();

/// Parse hosts-file text into `(hostname, SocketAddr)` overrides.
fn parse(input: &str) -> Result<Vec<(String, SocketAddr)>, HostsError> {
    let mut entries = Vec::new();
    for (idx, raw) in input.lines().enumerate() {
        let line_num = idx + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let addr_str = parts.next().ok_or_else(|| HostsError::BadLine {
            line: line_num,
            content: raw.to_string(),
        })?;
        let ip: IpAddr = addr_str.parse().map_err(|_| HostsError::BadAddress {
            line: line_num,
            addr: addr_str.to_string(),
        })?;

        let mut had_host = false;
        for host in parts {
            had_host = true;
            entries.push((host.to_string(), SocketAddr::new(ip, 0)));
        }
        // A line with an IP but no hostname is suspicious but not
        // strictly invalid in the /etc/hosts grammar. Treat it as
        // a parse error so wpt-style files always round-trip cleanly.
        if !had_host {
            return Err(HostsError::BadLine {
                line: line_num,
                content: raw.to_string(),
            });
        }
    }
    Ok(entries)
}

/// Load a hosts file from disk and install it as the process-wide
/// override set. Idempotency is intentional: callers that pass
/// the same file twice will hit [`HostsError::AlreadySet`].
///
/// # Errors
///
/// Returns a [`HostsError`] if the file cannot be read, a line is
/// malformed, or this function has already been called in this
/// process.
pub fn set_from_file(path: &Path) -> Result<(), HostsError> {
    let contents = fs::read_to_string(path).map_err(|source| HostsError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let entries = parse(&contents)?;
    OVERRIDES.set(entries).map_err(|_| HostsError::AlreadySet)
}

/// Apply the installed overrides (if any) to `builder`. When no
/// hosts file has been loaded, returns the builder unchanged.
pub fn apply(
    builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    let Some(entries) = OVERRIDES.get() else {
        return builder;
    };
    let mut b = builder;
    for (host, addr) in entries {
        b = b.resolve(host, *addr);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wpt_style_entries() {
        let input = "\
# Start web-platform-tests hosts
127.0.0.1\tnot-web-platform.test
127.0.0.1\tweb-platform.test
127.0.0.1\twww.web-platform.test
# End web-platform-tests hosts
";
        let entries = parse(input).expect("wpt-style entries should parse");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].0, "not-web-platform.test");
        assert_eq!(entries[0].1.port(), 0, "port must be 0 so reqwest uses URL port");
        assert_eq!(entries[0].1.ip().to_string(), "127.0.0.1");
        assert_eq!(entries[2].0, "www.web-platform.test");
    }

    #[test]
    fn allows_multiple_hosts_per_ip() {
        let input = "127.0.0.1 web-platform.test www.web-platform.test alt.web-platform.test\n";
        let entries = parse(input).expect("multi-host line should parse");
        assert_eq!(entries.len(), 3);
        assert!(entries
            .iter()
            .any(|(h, _)| h == "alt.web-platform.test"));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let input = "\n  # comment\n\n127.0.0.1 example\n# trailing\n\n";
        let entries = parse(input).expect("comments and blanks should be skipped");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "example");
    }

    #[test]
    fn rejects_invalid_ip() {
        let input = "not-an-ip web-platform.test\n";
        let err = parse(input).expect_err("bad ip should be rejected");
        assert!(matches!(err, HostsError::BadAddress { line: 1, .. }));
    }

    #[test]
    fn rejects_line_with_only_ip() {
        let input = "127.0.0.1\n";
        let err = parse(input).expect_err("ip-without-host should fail");
        assert!(matches!(err, HostsError::BadLine { line: 1, .. }));
    }
}
