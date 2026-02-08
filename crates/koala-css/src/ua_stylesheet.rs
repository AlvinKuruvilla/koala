//! User-Agent Stylesheet
//!
//! [WHATWG HTML § 15 Rendering](https://html.spec.whatwg.org/multipage/rendering.html)
//!
//! "User agents are expected to have a default style sheet that presents elements
//! of HTML documents in ways consistent with general user expectations."
//!
//! [CSS Cascading § 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
//!
//! "Each style rule has a cascade origin, which determines where it enters the
//! cascade. CSS defines three core origins: Author, User, and User-Agent."
//!
//! UA rules have the lowest priority in the cascade — any author rule overrides
//! a UA rule regardless of specificity.

use std::sync::OnceLock;

use crate::parser::{CSSParser, Stylesheet};
use crate::tokenizer::CSSTokenizer;

/// [WHATWG HTML § 15.3 Rendering — Suggested default style sheet](https://html.spec.whatwg.org/multipage/rendering.html#the-css-user-agent-style-sheet-and-presentational-hints)
///
/// Default CSS rules for HTML elements. This is a subset of the full UA
/// stylesheet covering the elements Koala currently supports.
const UA_CSS: &str = r#"
/* [§ 15.3.1 Hidden elements](https://html.spec.whatwg.org/multipage/rendering.html#hidden-elements) */
/* "The following elements must have their 'display' property set to 'none'." */
area, base, basefont, datalist, head, link, meta, noembed,
noframes, param, rp, script, style, template, title {
    display: none;
}

/* [§ 15.3.3 Flow content](https://html.spec.whatwg.org/multipage/rendering.html#flow-content-3) */
/* "The following elements must have their 'display' property set to 'block'." */
address, article, aside, blockquote, body, center, dd, details,
dialog, dir, div, dl, dt, fieldset, figcaption, figure, footer,
form, h1, h2, h3, h4, h5, h6, header, hgroup, hr, html, legend,
listing, main, menu, nav, ol, p, plaintext, pre, search,
section, summary, ul, xmp {
    display: block;
}

/* [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists) */
/* "li { display: list-item; }" */
li {
    display: list-item;
}

/* [§ 15.3.6 Sections and headings](https://html.spec.whatwg.org/multipage/rendering.html#sections-and-headings) */

/* "h1 { ... font-weight: bold; font-size: 2.00em; margin-block-start: 0.67em; margin-block-end: 0.67em; }" */
h1 {
    font-size: 2em;
    font-weight: bold;
    margin-block-start: 0.67em;
    margin-block-end: 0.67em;
}

/* "h2 { ... font-weight: bold; font-size: 1.50em; margin-block-start: 0.83em; margin-block-end: 0.83em; }" */
h2 {
    font-size: 1.5em;
    font-weight: bold;
    margin-block-start: 0.83em;
    margin-block-end: 0.83em;
}

/* "h3 { ... font-weight: bold; font-size: 1.17em; margin-block-start: 1.00em; margin-block-end: 1.00em; }" */
h3 {
    font-size: 1.17em;
    font-weight: bold;
    margin-block-start: 1em;
    margin-block-end: 1em;
}

/* "h4 { ... font-weight: bold; margin-block-start: 1.33em; margin-block-end: 1.33em; }" */
h4 {
    font-weight: bold;
    margin-block-start: 1.33em;
    margin-block-end: 1.33em;
}

/* "h5 { ... font-weight: bold; font-size: 0.83em; margin-block-start: 1.67em; margin-block-end: 1.67em; }" */
h5 {
    font-size: 0.83em;
    font-weight: bold;
    margin-block-start: 1.67em;
    margin-block-end: 1.67em;
}

/* "h6 { ... font-weight: bold; font-size: 0.67em; margin-block-start: 2.33em; margin-block-end: 2.33em; }" */
h6 {
    font-size: 0.67em;
    font-weight: bold;
    margin-block-start: 2.33em;
    margin-block-end: 2.33em;
}

/* [§ 15.3.5 Grouping content](https://html.spec.whatwg.org/multipage/rendering.html#grouping-content) */

/* "p, blockquote, figure, listing, plaintext, pre, xmp {
      margin-block-start: 1em; margin-block-end: 1em; }" */
p, blockquote, figure, listing, plaintext, pre, xmp {
    margin-block-start: 1em;
    margin-block-end: 1em;
}

/* "blockquote, figure { margin-inline-start: 40px; margin-inline-end: 40px; }" */
blockquote, figure {
    margin-left: 40px;
    margin-right: 40px;
}

/* [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists) */

/* "ol, ul, menu { ... margin-block-start: 1em; margin-block-end: 1em; padding-inline-start: 40px; }" */
ol, ul, menu {
    margin-block-start: 1em;
    margin-block-end: 1em;
    padding-left: 40px;
}

/* [§ 15.3.7 Lists](https://html.spec.whatwg.org/multipage/rendering.html#lists) */
/* "ul, menu { list-style-type: disc; }" */
ul, menu {
    list-style-type: disc;
}

/* "ol { list-style-type: decimal; }" */
ol {
    list-style-type: decimal;
}

/* [§ 15.3.4 The page](https://html.spec.whatwg.org/multipage/rendering.html#the-page) */
/* "body { margin: 8px; }" */
body {
    margin: 8px;
}

/* [§ 15.3.8 Text-level semantics](https://html.spec.whatwg.org/multipage/rendering.html#text-level-semantics) */

/* "b, strong { font-weight: bolder; }" */
/* NOTE: Using "bold" instead of "bolder" because our parse_font_weight()
   does not yet handle relative keywords. Functionally equivalent here. */
b, strong {
    font-weight: bold;
}

/* "i, cite, em, var, dfn { font-style: italic; }" */
em, i, cite, dfn, var {
    font-style: italic;
}

/* [§ 15.5.12–15.5.15 Form controls](https://html.spec.whatwg.org/multipage/rendering.html#the-input-element-as-a-form-control) */
input, textarea, select, button {
    display: inline-block;
    border: 2px inset;
    padding: 1px 2px;
}

button {
    padding: 1px 6px;
}

/* [§ 15.3.10 Tables](https://html.spec.whatwg.org/multipage/rendering.html#tables-2) */
table {
    display: table;
}

td, th {
    padding: 1px;
}

th {
    font-weight: bold;
}
"#;

/// Return the parsed UA stylesheet, parsing only once.
///
/// [CSS Cascading § 6.1](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// "Each style rule has a cascade origin... User-Agent origin rules have
/// the lowest priority."
///
/// The stylesheet is parsed once and cached via `OnceLock`.
pub fn ua_stylesheet() -> &'static Stylesheet {
    static STYLESHEET: OnceLock<Stylesheet> = OnceLock::new();
    STYLESHEET.get_or_init(|| {
        let mut tokenizer = CSSTokenizer::new(UA_CSS.to_string());
        tokenizer.run();
        let mut parser = CSSParser::new(tokenizer.into_tokens());
        parser.parse_stylesheet()
    })
}
