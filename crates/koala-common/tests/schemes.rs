//! Which URLs `DefaultSender` fetches, by scheme.

use koala_common::net::{DefaultSender, FetchError, RequestSender};
use koala_common::url::{file_url_from_path, scheme};

/// A mistyped scheme is reported as a scheme problem, not as a missing
/// file: koala used to read `htpps://www.google.com` as a relative path
/// and report "No such file or directory".
#[test]
fn unknown_scheme_is_named_in_the_error() {
    let err = DefaultSender
        .fetch("htpps://www.google.com")
        .expect_err("htpps is not a scheme koala loads");
    assert!(
        matches!(&err, FetchError::UnsupportedScheme { scheme, .. } if scheme == "htpps"),
        "got {err}"
    );
    assert!(err.to_string().contains("'htpps' scheme"), "got {err}");
}

/// Schemes are ASCII case-insensitive.
#[test]
fn upper_case_schemes_dispatch_like_lower_case() {
    assert_eq!(DefaultSender.fetch("DATA:,hi").expect("valid data URL"), b"hi");

    let path = std::env::temp_dir().join(format!("koala-schemes-{}.txt", std::process::id()));
    std::fs::write(&path, "body").expect("temp dir is writable");
    let url = file_url_from_path(&path).expect("file exists");
    let upper = format!("FILE{}", &url["file".len()..]);
    let body = DefaultSender.fetch(&upper);
    std::fs::remove_file(&path).expect("file was just written");
    assert_eq!(body.expect("FILE: reads like file:"), b"body");
}

/// A string with no scheme is still a filesystem path.
#[test]
fn no_scheme_is_a_path() {
    let err = DefaultSender
        .fetch("definitely/not/here.html")
        .expect_err("no such file");
    assert!(matches!(err, FetchError::LocalRead { .. }), "got {err}");
}

#[test]
fn scheme_is_returned_as_written() {
    assert_eq!(scheme("HTTPS://a.test/"), Some("HTTPS"));
    assert_eq!(scheme("htpps://www.google.com"), Some("htpps"));
    assert_eq!(scheme("relative/path.html"), None);
    assert_eq!(scheme("/abs/path.html"), None);
    // A colon after a character schemes cannot contain is not a scheme.
    assert_eq!(scheme("foo/bar:baz"), None);
}
