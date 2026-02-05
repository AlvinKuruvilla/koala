//! CSS tokenizer, parser, selector matching, cascade, and style computation for the Koala browser.
//!
//! # Scope
//!
//! This crate implements:
//! - **CSS Tokenizer** ([§ 4 Tokenization](https://www.w3.org/TR/css-syntax-3/#tokenization))
//!   - All token types: ident, function, at-keyword, hash, string, url, number, dimension, etc.
//!   - Comment handling
//!   - Escape sequences
//!
//! - **CSS Parser** ([§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing))
//!   - Stylesheet parsing
//!   - Rule parsing (style rules and at-rules)
//!   - Declaration parsing
//!
//! - **CSS Selectors** ([Selectors Level 4](https://www.w3.org/TR/selectors-4/))
//!   - Type, class, ID, and universal selectors
//!   - Compound selectors
//!   - Complex selectors with combinators (descendant, child, sibling)
//!   - Specificity calculation
//!
//! - **CSS Cascade** ([CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/))
//!   - Selector matching
//!   - Specificity-based ordering
//!   - Property inheritance
//!
//! - **Computed Styles** ([CSS Values Level 4](https://www.w3.org/TR/css-values-4/))
//!   - Color values (hex, named colors)
//!   - Length values (px)
//!   - Shorthand property expansion (margin, padding, border)
//!
//! - **Layout Engine** (stub) ([CSS Display Level 3](https://www.w3.org/TR/css-display-3/))
//!   - Display value types
//!   - Box model structures
//!
//! # Not Yet Implemented
//!
//! - Percentage and relative length units (em, rem, %)
//! - rgb(), hsl() color functions
//! - Pseudo-classes and pseudo-elements
//! - Attribute selectors
//! - Media queries
//! - Full layout algorithm

/// CSS backgrounds per [CSS Backgrounds and Borders Level 3](https://www.w3.org/TR/css-backgrounds-3/).
pub mod backgrounds;
/// CSS cascade and style computation per [CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/).
pub mod cascade;
/// Box model and layout structures per [CSS Display Level 3](https://www.w3.org/TR/css-display-3/).
pub mod layout;
/// Display list and painting per [CSS 2.1 Appendix E](https://www.w3.org/TR/CSS2/zindex.html).
pub mod paint;
/// CSS parser per [§ 5 Parsing](https://www.w3.org/TR/css-syntax-3/#parsing).
pub mod parser;
/// CSS selector parsing and matching per [Selectors Level 4](https://www.w3.org/TR/selectors-4/).
pub mod selector;
/// Computed style representation per [CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/).
pub mod style;
/// CSS tokenizer per [§ 4 Tokenization](https://www.w3.org/TR/css-syntax-3/#tokenization).
pub mod tokenizer;
/// User-agent stylesheet per [WHATWG HTML § 15 Rendering](https://html.spec.whatwg.org/multipage/rendering.html).
pub mod ua_stylesheet;

// Re-exports for convenience
pub use backgrounds::canvas_background;
pub use cascade::compute_styles;
pub use layout::{ApproximateFontMetrics, BoxDimensions, BoxType, EdgeSizes, FontMetrics, FontStyle, LayoutBox, Rect};
pub use paint::{DisplayCommand, DisplayList, Painter};
pub use parser::{CSSParser, ComponentValue, Declaration, Rule, Stylesheet};
pub use selector::{ParsedSelector, Specificity, parse_selector};
pub use style::ComputedStyle;
pub use tokenizer::{CSSToken, CSSTokenizer};
pub use style::{
    AutoLength, BorderValue, ColorValue, DisplayValue, InnerDisplayType,
    LengthValue, OuterDisplayType, DEFAULT_FONT_SIZE_PX,
};

// External stylesheet support - see extract_all_stylesheets() for main entry point
// NOTE: fetch_external_stylesheet() is stubbed with todo!() - implement to enable external CSS

use koala_dom::{DomTree, ElementData, NodeId, NodeType};

/// [HTML Standard § 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
///
/// Extract CSS text from all `<style>` elements in the DOM tree.
pub fn extract_style_content(tree: &DomTree) -> String {
    let mut css = String::new();
    collect_style_content(tree, tree.root(), &mut css);
    css
}

/// Recursively collect CSS text from style elements.
fn collect_style_content(tree: &DomTree, id: NodeId, css: &mut String) {
    let Some(node) = tree.get(id) else { return };

    match &node.node_type {
        NodeType::Element(data) if data.tag_name.eq_ignore_ascii_case("style") => {
            // Collect text content of style element
            for &child_id in tree.children(id) {
                if let Some(text) = tree.as_text(child_id) {
                    css.push_str(text);
                    css.push('\n');
                }
            }
        }
        // External stylesheets (<link rel="stylesheet">) are handled by
        // collect_all_stylesheets() which calls fetch_external_stylesheet().
        _ => {}
    }

    // Recurse into children
    for &child_id in tree.children(id) {
        collect_style_content(tree, child_id, css);
    }
}

// ============================================================================
// External Stylesheet Support
// ============================================================================

/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// "Declarations from style sheets independently linked by the originating document
/// are treated as if they were concatenated in linking order."
///
/// The source of a stylesheet, used for cascade ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StylesheetSource {
    /// External stylesheet from `<link rel="stylesheet">`.
    ///
    /// [§ 4.2.4 The link element](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
    External {
        /// The href URL of the stylesheet.
        href: String,
    },
    /// Inline stylesheet from `<style>` element.
    ///
    /// [§ 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
    Inline,
}

/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// "The last declaration in document order wins."
///
/// A stylesheet with its source information for cascade ordering.
#[derive(Debug, Clone)]
pub struct SourcedStylesheet {
    /// The parsed stylesheet.
    pub stylesheet: Stylesheet,
    /// Where the stylesheet came from.
    pub source: StylesheetSource,
}

/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// "Declarations from style sheets independently linked by the originating document
/// are treated as if they were concatenated in linking order."
///
/// Collection of stylesheets from a document, in document order.
#[derive(Debug, Clone, Default)]
pub struct DocumentStylesheets {
    /// Stylesheets in document order.
    ///
    /// Per spec, stylesheets appear in the order their respective elements
    /// (`<link>` or `<style>`) appear in the document tree.
    pub sheets: Vec<SourcedStylesheet>,
}

impl DocumentStylesheets {
    /// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
    ///
    /// "Declarations from style sheets independently linked by the originating document
    /// are treated as if they were concatenated in linking order."
    ///
    /// Merge all stylesheets into a single stylesheet for cascade processing.
    /// Rules are concatenated in document order.
    #[must_use]
    pub fn into_merged_stylesheet(self) -> Stylesheet {
        let mut all_rules = Vec::new();
        for sheet in self.sheets {
            all_rules.extend(sheet.stylesheet.rules);
        }
        Stylesheet { rules: all_rules }
    }
}

/// [§ 4.2.4 The link element](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
/// [§ 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
///
/// Extract all stylesheet sources from the DOM tree.
///
/// This function finds both:
/// - `<link rel="stylesheet" href="...">` elements (external stylesheets)
/// - `<style>` elements (inline stylesheets)
///
/// Returns stylesheet sources in document order, which is important for cascade.
///
/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
/// "The last declaration in document order wins."
#[must_use]
pub fn collect_stylesheet_sources(tree: &DomTree) -> Vec<StylesheetSource> {
    let mut sources = Vec::new();
    collect_sources_recursive(tree, tree.root(), &mut sources);
    sources
}

/// Recursively collect stylesheet sources in document order.
fn collect_sources_recursive(tree: &DomTree, id: NodeId, sources: &mut Vec<StylesheetSource>) {
    let Some(node) = tree.get(id) else { return };

    if let NodeType::Element(data) = &node.node_type {
        // [§ 4.2.4 The link element](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
        //
        // "A link element must have a rel attribute."
        // "The link element has several uses... loading stylesheets..."
        if data.tag_name.eq_ignore_ascii_case("link") {
            // STEP 1: Check if this is a stylesheet link.
            //
            // [§ 4.2.4](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
            // "If the rel attribute's value contains the token stylesheet,
            // then the link is a stylesheet link."
            if is_stylesheet_link(data) {
                // STEP 2: Get the href attribute.
                //
                // [§ 4.2.4](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
                // "The href attribute gives the address (a valid non-empty URL potentially
                // surrounded by spaces) of the linked resource."
                if let Some(href) = data.attrs.get("href") {
                    if !href.trim().is_empty() {
                        sources.push(StylesheetSource::External { href: href.clone() });
                    }
                }
            }
        }
        // [§ 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
        //
        // "The style element allows authors to embed CSS style sheets in their documents."
        else if data.tag_name.eq_ignore_ascii_case("style") {
            sources.push(StylesheetSource::Inline);
        }
    }

    // Continue in document order (depth-first traversal)
    for &child_id in tree.children(id) {
        collect_sources_recursive(tree, child_id, sources);
    }
}

/// [§ 4.2.4 The link element](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
///
/// Check if a link element is a stylesheet link.
///
/// "If the rel attribute's value contains the token stylesheet,
/// then the link is a stylesheet link."
///
/// [§ 2.4.7 Space-separated tokens](https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#space-separated-tokens)
/// "A set of space-separated tokens is a string containing zero or more words
/// separated by one or more ASCII whitespace characters."
fn is_stylesheet_link(data: &ElementData) -> bool {
    // STEP 1: Get the rel attribute value.
    let Some(rel) = data.attrs.get("rel") else {
        return false;
    };

    // STEP 2: Split on ASCII whitespace and check for "stylesheet" token.
    //
    // [§ 2.4.7](https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#space-separated-tokens)
    // Token comparison is ASCII case-insensitive.
    rel.split_ascii_whitespace()
        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
}

/// Fetch the content of an external stylesheet.
///
/// [§ 4.2.4 The link element](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
/// [§ 4.2.4.3 Fetching and processing a resource from a link element](https://html.spec.whatwg.org/multipage/semantics.html#link-type-stylesheet)
///
/// # Algorithm (from WHATWG HTML spec)
///
/// [§ 4.2.4.3](https://html.spec.whatwg.org/multipage/semantics.html#link-type-stylesheet)
///
/// STEP 1: "Let options be the result of creating link options from element."
///
/// STEP 2: "If options is null, return."
///
/// STEP 3: "Let request be the result of creating a link request given options."
///         - The request URL is the href resolved against the document base URL
///         - The request destination is "style"
///
/// STEP 4: "Set request's initiator type to 'css'."
///
/// STEP 5: "Fetch request."
///         - This is where the actual network request happens
///         - Must handle redirects, CORS, content-type, etc.
///
/// STEP 6: "Process the linked resource when the fetch completes."
///         - Parse the response body as CSS
///         - If the resource is not available, treat as empty stylesheet
///
/// # Arguments
///
/// * `href` - The URL to fetch (may be relative)
/// * `base_url` - The document's base URL for resolving relative URLs
///
/// # Returns
///
/// The CSS text content on success, or an error message.
///
/// # Errors
///
/// Returns an error if the stylesheet cannot be fetched.
pub fn fetch_external_stylesheet(href: &str, base_url: Option<&str>) -> Result<String, String> {
    // [§ 4.2.4.3](https://html.spec.whatwg.org/multipage/semantics.html#link-type-stylesheet)
    //
    // STEP 1: "Let options be the result of creating link options from element."
    // Implementation note: We receive the href directly, base_url for resolution.

    // STEP 2: "If options is null, return."
    // Implementation note: We assume valid options if href is provided.

    // STEP 3: "Let request be the result of creating a link request given options."
    //
    // [§ 4.2.4](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
    // "The href attribute gives the address (a valid non-empty URL potentially
    // surrounded by spaces) of the linked resource."
    let resolved_url = resolve_url(href, base_url);

    // STEP 4: "Set request's initiator type to 'css'."
    // Implementation note: This is for resource timing APIs, not critical for MVP.

    // STEP 5: "Fetch request."
    //
    // TODO: Implement proper Fetch Standard (https://fetch.spec.whatwg.org/)
    // Currently a quick-and-dirty implementation that skips:
    // - CORS handling
    // - Content-Type validation
    // - Proper error handling per spec
    use std::time::Duration;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(&resolved_url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    // STEP 6: "Process the linked resource when the fetch completes."
    //
    // [§ 4.2.4](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
    // "If the resource is not available, the user agent must act as if
    // the resource was an empty style sheet."
    response
        .text()
        .map_err(|e| format!("Failed to read response body: {e}"))
}

/// [§ 4.2.3 The base element](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element)
/// [URL Standard](https://url.spec.whatwg.org/)
///
/// Resolve a potentially relative URL against a base URL.
///
/// # Algorithm
///
/// [§ 2.5 URLs](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#resolving-urls)
///
/// STEP 1: "If url is an absolute URL, return url."
///
/// STEP 2: "Otherwise, resolve url relative to base."
///
/// NOTE: This is a simplified implementation. Full URL resolution requires
/// implementing the URL Standard's URL parsing algorithm.
#[must_use]
pub fn resolve_url(href: &str, base_url: Option<&str>) -> String {
    // STEP 1: Check if href is already absolute.
    //
    // [URL Standard § 4.3](https://url.spec.whatwg.org/#url-parsing)
    // "An absolute-URL string is a URL-scheme string, followed by U+003A (:),
    // followed by a scheme-specific part."
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("data:")
        || href.starts_with("file:")
    {
        return href.to_string();
    }

    // STEP 2: Resolve relative URL against base.
    //
    // TODO(url-resolution): Implement proper URL resolution per URL Standard.
    // The full algorithm handles:
    // - Protocol-relative URLs (//example.com/path)
    // - Absolute paths (/path/to/file)
    // - Relative paths (../path, ./path, path)
    // - Query strings and fragments
    //
    // For now, do simple path joining for common cases.
    let Some(base) = base_url else {
        return href.to_string();
    };

    if href.starts_with("//") {
        // Protocol-relative URL - prepend scheme from base
        //
        // TODO(url-resolution): Extract scheme from base properly
        if base.starts_with("https:") {
            format!("https:{href}")
        } else {
            format!("http:{href}")
        }
    } else if href.starts_with('/') {
        // Absolute path - join with origin
        //
        // TODO(url-resolution): Extract origin from base_url properly
        // For now, find the third slash (after scheme://) and take everything before it
        if let Some(scheme_end) = base.find("://") {
            let after_scheme = &base[scheme_end + 3..];
            if let Some(path_start) = after_scheme.find('/') {
                let origin = &base[..scheme_end + 3 + path_start];
                format!("{origin}{href}")
            } else {
                // No path in base, just append
                format!("{base}{href}")
            }
        } else {
            href.to_string()
        }
    } else {
        // Relative path - join with base directory
        //
        // TODO(url-resolution): Handle . and .. path segments properly
        let base_dir = base.rsplit_once('/').map_or(base, |(dir, _)| dir);
        format!("{base_dir}/{href}")
    }
}

/// Extract and collect all stylesheets from the DOM in cascade order.
///
/// [§ 4.2.4 The link element](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
/// [§ 4.2.6 The style element](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
/// [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// This is the main entry point for stylesheet extraction. It:
/// 1. Finds all `<link rel="stylesheet">` and `<style>` elements in document order
/// 2. Fetches external stylesheets (currently stubbed with `todo!()`)
/// 3. Extracts inline stylesheet content
/// 4. Returns parsed stylesheets in document order for cascade processing
///
/// # Cascade Order
///
/// [§ 6.1](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
///
/// "Declarations from style sheets independently linked by the originating document
/// are treated as if they were concatenated in linking order."
///
/// "The last declaration in document order wins."
///
/// This function preserves document order so the cascade can be applied correctly.
///
/// # Arguments
///
/// * `tree` - The DOM tree to extract stylesheets from
/// * `base_url` - The document's base URL for resolving relative stylesheet URLs
///
/// # Returns
///
/// A `DocumentStylesheets` containing all stylesheets in document order.
#[must_use]
pub fn extract_all_stylesheets(tree: &DomTree, base_url: Option<&str>) -> DocumentStylesheets {
    let mut sheets = Vec::new();
    let mut inline_style_index = 0;

    // STEP 1: Collect all stylesheet sources in document order.
    //
    // [§ 6.1](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
    // "The last declaration in document order wins."
    let sources = collect_stylesheet_sources(tree);

    // STEP 2: Process each source to get the stylesheet content.
    for source in sources {
        match &source {
            // STEP 2a: External stylesheet - fetch and parse.
            //
            // [§ 4.2.4](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
            StylesheetSource::External { href } => {
                match fetch_external_stylesheet(href, base_url) {
                    Ok(css_text) => {
                        // Parse the CSS
                        let stylesheet = parse_css_text(&css_text);
                        sheets.push(SourcedStylesheet {
                            stylesheet,
                            source: source.clone(),
                        });
                    }
                    Err(e) => {
                        // [§ 4.2.4](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element)
                        //
                        // "If the resource is not available, the user agent must act as if
                        // the resource was an empty style sheet."
                        koala_common::warning::warn_once(
                            "Koala CSS",
                            &format!("Failed to load stylesheet '{href}': {e}"),
                        );
                        // Continue without this stylesheet (empty stylesheet per spec)
                    }
                }
            }

            // STEP 2b: Inline stylesheet - extract text content.
            //
            // [§ 4.2.6](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element)
            StylesheetSource::Inline => {
                // Extract the content of the nth inline style element
                let css_text = extract_nth_style_content(tree, inline_style_index);
                inline_style_index += 1;

                if !css_text.is_empty() {
                    let stylesheet = parse_css_text(&css_text);
                    sheets.push(SourcedStylesheet {
                        stylesheet,
                        source: source.clone(),
                    });
                }
            }
        }
    }

    DocumentStylesheets { sheets }
}

/// Extract the content of the nth `<style>` element in document order.
fn extract_nth_style_content(tree: &DomTree, n: usize) -> String {
    let mut css = String::new();
    let mut count = 0;
    let _ = extract_nth_style_recursive(tree, tree.root(), n, &mut count, &mut css);
    css
}

/// Recursively find and extract the nth style element's content.
fn extract_nth_style_recursive(
    tree: &DomTree,
    id: NodeId,
    target: usize,
    count: &mut usize,
    css: &mut String,
) -> bool {
    let Some(node) = tree.get(id) else {
        return false;
    };

    if let NodeType::Element(data) = &node.node_type {
        if data.tag_name.eq_ignore_ascii_case("style") {
            if *count == target {
                // Found the target style element - extract its content
                for &child_id in tree.children(id) {
                    if let Some(text) = tree.as_text(child_id) {
                        css.push_str(text);
                    }
                }
                return true; // Stop searching
            }
            *count += 1;
        }
    }

    // Continue searching children
    for &child_id in tree.children(id) {
        if extract_nth_style_recursive(tree, child_id, target, count, css) {
            return true;
        }
    }

    false
}

/// Helper to parse CSS text into a Stylesheet.
fn parse_css_text(css: &str) -> Stylesheet {
    let mut tokenizer = CSSTokenizer::new(css.to_string());
    tokenizer.run();
    let mut parser = CSSParser::new(tokenizer.into_tokens());
    parser.parse_stylesheet()
}
