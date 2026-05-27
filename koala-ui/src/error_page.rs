// Error page shown when the render or loader pipeline can't
// produce a valid view for a page — whether because of a network
// failure, an HTTP error, a parse error, or a panic anywhere in
// the engine.
//
// The template lives in `res/error.html` so it can be edited as a
// real HTML file with the same `include_str!` trick used for the
// landing page. Two placeholders, `{{url}}` and `{{message}}`,
// are substituted in at call time after HTML-escaping the caller's
// strings so a malicious URL or error message can't inject markup
// into the rendered page.

const TEMPLATE: &str = include_str!("../res/error.html");

/// Builds a full HTML document for the error page with the failing
/// URL and an explanatory message filled in. Ready to be handed to
/// `koala_browser::parse_html_string`.
pub fn render(url: &str, message: &str) -> String {
    TEMPLATE
        .replace("{{url}}", &html_escape(url))
        .replace("{{message}}", &html_escape(message))
}

/// Minimal HTML text-content escaping. `parse_html_string` will
/// happily accept raw `<` and `&` inside attributes or text nodes
/// and do unexpected things with them, so we sanitise before
/// substituting into the template.
fn html_escape(s: &str) -> String {
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
