// The page shown in place of a document that could not be loaded or
// drawn.
//
// It follows the pages Chromium and Firefox show (Chromium's
// `components/neterror` and `localized_error.cc`, Firefox's
// `aboutNetError`): a heading and one-sentence summary specific to the
// cause, a short list of things to try, and an error code. A "Reload"
// button is left out because koala-ui cannot follow clicks inside a page
// yet; reloading is listed as a suggestion instead, for the toolbar's
// reload. Below the code, the engine's own message, for whoever is
// debugging koala.
//
// The template lives in `res/error.html` so it can be edited as HTML.
// Every value is HTML-escaped before it goes in, and placeholders are
// replaced in a single pass, so neither markup nor a `{{...}}` in a URL
// or message can reach the page as anything but text.

use koala_browser::LoadError;
use koala_browser::fetch::FetchCause;

const TEMPLATE: &str = include_str!("../res/error.html");

/// Why the page could not be shown.
pub enum Failure<'a> {
    /// The document could not be fetched.
    Load(&'a LoadError),
    /// The engine panicked or produced nothing to draw; carries its message.
    Engine(&'a str),
}

/// A complete HTML document explaining `failure` for `url`, ready for
/// `koala_browser::parse_html_string`.
pub fn render(url: &str, failure: &Failure<'_>) -> String {
    let host = host_of(url);
    let page = describe(url, &host, failure);
    let detail = match failure {
        Failure::Load(error) => error.to_string(),
        Failure::Engine(message) => (*message).to_string(),
    };
    let suggestions = if page.suggestions.is_empty() {
        String::new()
    } else {
        let items: String = page
            .suggestions
            .iter()
            .map(|s| format!("<li>{}</li>", escape(s)))
            .collect();
        format!("<p>Try:</p><ul>{items}</ul>")
    };
    fill(TEMPLATE, |name| match name {
        "title" => Some(escape(&page.title)),
        "heading" => Some(escape(page.heading)),
        "summary" => Some(page.summary.clone()),
        "suggestions" => Some(suggestions.clone()),
        "code" => Some(escape(&page.code)),
        "detail" => Some(escape(&detail)),
        _ => None,
    })
}

/// The text of one error page. `summary` is HTML, built from escaped
/// parts.
struct Page {
    title: String,
    heading: &'static str,
    summary: String,
    suggestions: &'static [&'static str],
    code: String,
}

const CHECK_CONNECTION: &str = "Checking the connection";
const CHECK_NETWORK: &str = "Checking the proxy, firewall, and DNS configuration";
const RELOAD: &str = "Reloading the page";

fn describe(url: &str, host: &str, failure: &Failure<'_>) -> Page {
    let unreachable = |summary: String, suggestions, code: &str| Page {
        title: host.to_string(),
        heading: "This site can't be reached",
        summary,
        suggestions,
        code: code.to_string(),
    };
    let strong_host = format!("<strong>{}</strong>", escape(host));
    let Failure::Load(LoadError::Fetch(error)) = failure else {
        return Page {
            title: host.to_string(),
            heading: "Something went wrong while displaying this page",
            summary: "koala loaded the page but could not draw it. This is a bug in koala."
                .to_string(),
            suggestions: &[RELOAD],
            code: "ERR_ENGINE".to_string(),
        };
    };
    match error.cause() {
        FetchCause::NameNotResolved => unreachable(
            format!("{strong_host}'s server IP address could not be found."),
            &["Checking the address for a typo", CHECK_CONNECTION, RELOAD],
            "ERR_NAME_NOT_RESOLVED",
        ),
        FetchCause::ConnectionRefused => unreachable(
            format!("{strong_host} refused to connect."),
            &[CHECK_CONNECTION, CHECK_NETWORK, RELOAD],
            "ERR_CONNECTION_REFUSED",
        ),
        FetchCause::TimedOut => unreachable(
            format!("{strong_host} took too long to respond."),
            &[CHECK_CONNECTION, CHECK_NETWORK, RELOAD],
            "ERR_TIMED_OUT",
        ),
        FetchCause::ConnectionFailed => unreachable(
            format!("koala could not connect to {strong_host}."),
            &[CHECK_CONNECTION, CHECK_NETWORK, RELOAD],
            "ERR_CONNECTION_FAILED",
        ),
        // Browsers show the server's own error page and fall back to this
        // only when the response body is empty. koala does not keep the
        // body yet, so it always falls back.
        FetchCause::HttpStatus(status) => Page {
            title: host.to_string(),
            heading: if status == 404 {
                "This page can't be found"
            } else {
                "This page isn't working"
            },
            summary: if status == 404 {
                format!(
                    "No webpage was found for the web address: <strong>{}</strong>",
                    escape(url)
                )
            } else {
                format!("{strong_host} is currently unable to handle this request.")
            },
            suggestions: &[RELOAD],
            code: format!("HTTP ERROR {status}"),
        },
        FetchCause::FileNotFound => Page {
            title: url.to_string(),
            heading: "Your file couldn't be accessed",
            summary: "It may have been moved, edited, or deleted.".to_string(),
            suggestions: &[],
            code: "ERR_FILE_NOT_FOUND".to_string(),
        },
        FetchCause::UnsupportedScheme(scheme) => Page {
            title: url.to_string(),
            heading: "The address wasn't understood",
            summary: format!(
                "koala can't open <strong>{}:</strong> addresses.",
                escape(&scheme)
            ),
            suggestions: &["Checking the address for a typo"],
            code: "ERR_UNKNOWN_URL_SCHEME".to_string(),
        },
        FetchCause::Other => unreachable(
            format!("The page at {strong_host} could not be loaded."),
            &[RELOAD],
            "ERR_FAILED",
        ),
    }
}

/// The host part of `url`, or `url` itself when it has none (a file path).
fn host_of(url: &str) -> String {
    url.split_once("://")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .filter(|host| !host.is_empty())
        .map_or_else(|| url.to_string(), str::to_string)
}

/// Replace every `{{name}}` in `template` with `value(name)`, in one pass.
/// Unknown names are left as they are, so a typo in the template shows up
/// on the page instead of vanishing.
fn fill(template: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}").and_then(|end| value(&after[..end]).map(|v| (end, v))) {
            Some((end, text)) => {
                out.push_str(&text);
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Escape text for an HTML text node or quoted attribute.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_text_in_a_value_is_not_expanded() {
        let out = fill("<p>{{a}} {{b}}</p>", |name| match name {
            "a" => Some("{{b}}".to_string()),
            "b" => Some("B".to_string()),
            _ => None,
        });
        assert_eq!(out, "<p>{{b}} B</p>");
    }

    #[test]
    fn unknown_placeholders_stay_visible() {
        assert_eq!(fill("{{nope}}", |_| None), "{{nope}}");
    }

    #[test]
    fn host_is_taken_from_the_authority() {
        assert_eq!(host_of("https://www.google.com/search?q=1"), "www.google.com");
        assert_eq!(host_of("htpps://www.google.com"), "www.google.com");
        assert_eq!(host_of("/tmp/page.html"), "/tmp/page.html");
    }

    #[test]
    fn engine_failure_page_names_the_message() {
        let html = render("https://a.test/", &Failure::Engine("layout <panicked>"));
        assert!(html.contains("Something went wrong while displaying this page"));
        assert!(html.contains("layout &lt;panicked&gt;"));
        assert!(html.contains("ERR_ENGINE"));
    }
}

