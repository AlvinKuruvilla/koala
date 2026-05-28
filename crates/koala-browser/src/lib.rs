//! Document loading and rendering pipeline for the Koala renderer.
//!
//! # Scope
//!
//! This crate provides:
//! - **Document Loading** - fetch and parse HTML documents
//! - **Style Computation** - integrate CSS with DOM
//! - **Render Tree** - styled DOM ready for layout
//! - **Layout Tree** - box tree with computed dimensions
//! - **Software Rendering** - headless screenshot generation
//! - **JavaScript Execution** - inline script execution via Boa
//!
//! # Not Yet Implemented
//!
//! - Resource loading (images, fonts, etc.)
//! - External script loading (`<script src="...">`)
//! - DOM manipulation from JavaScript

pub mod font_metrics;
pub mod image_loader;
pub mod renderer;

pub use koala_css as css;
pub use koala_dom as dom;
pub use koala_html as html;
pub use koala_js as js;

pub use renderer::{Renderer, RendererFonts};

// Re-export LoadedImage from koala-common for backwards compatibility.
pub use koala_common::image::LoadedImage;

/// WPT-style hosts-file DNS overrides for reqwest. See
/// [`koala_common::hosts`] for the full module docs.
pub use koala_common::hosts;

/// Engine-wide diagnostic-warning system, plus the process-wide
/// quiet flag toggled by `koala-cli --wpt-protocol`.
pub use koala_common::warning;

use image_loader::{
    ImageLoaderPipeline, fetch_image_bytes, strip_url_decorations, warn_url_decorations,
};
use koala_css::{
    ComputedStyle, LayoutBox, Stylesheet, compute_styles, extract_all_stylesheets,
    extract_style_content,
};
use koala_dom::{DomTree, NodeId};
use koala_html::{HTMLParser, HTMLTokenizer, Token};
use koala_js::JsRuntime;
use std::collections::HashMap;
use std::fs;

/// A fully loaded and parsed document.
///
/// Contains all the data needed to render a page: DOM, styles, layout tree,
/// and debugging information.
pub struct LoadedDocument {
    /// Original HTML source
    pub html_source: String,

    /// Source path or URL
    pub source_path: String,

    /// HTML tokens (for debugging)
    pub tokens: Vec<Token>,

    /// Parsed DOM tree
    pub dom: DomTree,

    /// Extracted CSS text
    pub css_text: String,

    /// Parsed stylesheet
    pub stylesheet: Stylesheet,

    /// Computed styles per node
    pub styles: HashMap<NodeId, ComputedStyle>,

    /// Layout tree (box tree, dimensions not yet computed)
    pub layout_tree: Option<LayoutBox>,

    /// Parse issues/warnings
    pub parse_issues: Vec<String>,

    /// Loaded images keyed by their `src` attribute value.
    ///
    /// [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
    ///
    /// Used by the renderer to draw `DrawImage` commands.
    pub images: HashMap<String, LoadedImage>,
}

/// Error type for document loading.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// Failed to read a local file.
    #[error("failed to read '{path}': {source}")]
    FileRead {
        /// The filesystem path that could not be read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to fetch a URL.
    #[error(transparent)]
    Fetch(#[from] koala_common::net::FetchError),
}

/// Extension points into the JS lifecycle for callers who need
/// to install globals before scripts run or drain state after the
/// document settles.
///
/// Both methods are default no-ops, so the simplest implementor
/// (`()`) just acts as "no extension." More substantial hooks
/// can be implemented by any caller — the WPT runner uses one to
/// install the testharness bridge via `koala_wpt::install` and
/// to drain results via `koala_wpt::take_test_results`.
///
/// The hook trait deliberately exposes only `&mut JsRuntime`
/// rather than the full pipeline state — keeps the surface
/// narrow and forces any cross-cutting state to live on the
/// hook implementor itself.
pub trait JsHooks {
    /// Called once after [`JsRuntime`] construction and after
    /// `location` is set, but *before* any `<script>` from the
    /// document executes. The right place to register additional
    /// globals or pre-populate hidden state.
    fn before_scripts(&mut self, _rt: &mut JsRuntime) {}

    /// Consulted between iterations of the post-`load`
    /// [`pump_until_idle`](JsRuntime::pump_until_idle_or) loop.
    /// Returning `true` exits the pump immediately.
    ///
    /// Default implementation never stops, matching the
    /// pre-hook behaviour. Hooks that observe a definite
    /// completion signal — most notably the WPT testharness
    /// bridge, which has captured `__koala_emit_completion__` —
    /// override this so the pump doesn't keep sleeping for
    /// stray watchdog timers once the verdict is in.
    fn should_stop_pumping(&mut self, _rt: &mut JsRuntime) -> bool {
        false
    }

    /// Called once after the post-`load` pump returns and right
    /// before the runtime is dropped. The right place to read
    /// out any state the hook accumulated during script
    /// execution.
    fn after_settled(&mut self, _rt: &mut JsRuntime) {}
}

/// No-op hook implementation. `load_document` delegates to
/// `load_document_with_hooks` with `&mut ()` so the existing
/// non-hooked entry points stay unchanged.
impl JsHooks for () {}

/// Load a document from a file path or URL.
///
/// This is the main entry point for loading a document. It handles:
/// - File reading for local paths
/// - URL fetching for http:// and https:// URLs
/// - HTML parsing and tokenization
/// - CSS extraction and parsing (including external stylesheets)
/// - Style computation
/// - Layout tree construction
///
/// # Arguments
///
/// * `path` - A file path or URL to load
///
/// # Errors
///
/// Returns [`LoadError::FileRead`] if the path is a local file that cannot
/// be read, or [`LoadError::Fetch`] if it is a URL that cannot be
/// fetched.
pub fn load_document(path: &str) -> Result<LoadedDocument, LoadError> {
    load_document_with_hooks(path, &mut ())
}

/// Load a document with a [`JsHooks`] implementation installed
/// during script execution.
///
/// Identical to [`load_document`] except that `hooks` gets
/// `before_scripts` called after `JsRuntime` construction and
/// `after_settled` called once the post-`load` pump returns.
/// Use this from the WPT runner / any caller that needs to wire
/// custom globals or drain JS state.
///
/// # Errors
///
/// Same as [`load_document`].
pub fn load_document_with_hooks<H: JsHooks>(
    path: &str,
    hooks: &mut H,
) -> Result<LoadedDocument, LoadError> {
    // Fetch or read the HTML source.
    //
    // `file://` URLs strip down to a plain absolute path before
    // hitting `fs::read_to_string`. Supporting them is mostly for
    // host-side tooling (the koala-cli WPT protocol passes URLs
    // verbatim, and `file:///path` is the natural way to point at
    // a local fixture from a JSON command).
    let (html_source, base_url) = if path.starts_with("http://") || path.starts_with("https://") {
        let text = koala_common::net::fetch_text(path)?;
        (text, Some(path))
    } else {
        let fs_path = path.strip_prefix("file://").unwrap_or(path);
        let content = fs::read_to_string(fs_path).map_err(|e| LoadError::FileRead {
            path: path.to_string(),
            source: e,
        })?;
        (content, None)
    };

    // Parse the document with base URL for resolving external stylesheets
    let mut doc = parse_html_with_base_url(&html_source, base_url, hooks);
    doc.source_path = path.to_string();

    Ok(doc)
}

/// Parse an HTML string into a `LoadedDocument`.
///
/// Use this when you already have the HTML content as a string.
/// Note: External stylesheets cannot be loaded without a base URL.
#[must_use]
pub fn parse_html_string(html: &str) -> LoadedDocument {
    parse_html_with_base_url(html, None, &mut ())
}

/// Parse an HTML string with an optional base URL for resolving external resources.
fn parse_html_with_base_url<H: JsHooks>(
    html: &str,
    base_url: Option<&str>,
    hooks: &mut H,
) -> LoadedDocument {
    let (tokens, dom, mut parse_issues) = tokenize_and_parse(html);
    let stylesheet = extract_stylesheet(&dom, base_url);
    // Inline CSS text kept for debugging.
    let css_text = extract_style_content(&dom);
    // [§ 6.1 Cascade Sorting Order](https://www.w3.org/TR/css-cascade-4/#cascade-sort)
    // "Each style rule has a cascade origin... User-Agent origin rules
    // have the lowest priority."
    let ua = koala_css::ua_stylesheet::ua_stylesheet();
    let styles = compute_initial_styles(&dom, ua, &stylesheet);
    let (images, image_dims) = load_images(&dom, base_url);
    let layout_tree = build_initial_layout_tree(&dom, &styles, &image_dims);

    // Execute JavaScript.
    // [§ 4.12.1.1 Processing model](https://html.spec.whatwg.org/multipage/scripting.html)
    //
    // Wrap the DOM in `Rc<RefCell<>>` for the duration of script
    // execution so JsRuntime can hand a shared handle to its
    // DOM-bridge globals. After the runtime is dropped its handle
    // clone drops with it, leaving the Rc unique — `into_inner`
    // recovers the owned `DomTree` for `LoadedDocument`.
    let scripts = load_scripts(&dom, base_url, &mut parse_issues);
    let dom_cell = std::rc::Rc::new(std::cell::RefCell::new(dom));
    let dom_was_mutated =
        execute_document_scripts(&dom_cell, scripts, base_url, hooks, &mut parse_issues);
    let dom = std::rc::Rc::try_unwrap(dom_cell)
        .expect("JsRuntime is dropped above; no other holders of the DOM handle")
        .into_inner();

    // If JS mutated the DOM (setAttribute, appendChild, textContent
    // setter, …), the styles + layout tree we built before scripts
    // ran no longer reflect the actual tree. We deliberately reuse
    // the already-loaded image cache rather than re-fetching, since
    // image loads are network-bound and the post-script DOM rarely
    // adds <img> tags pointing to never-fetched URLs in practice.
    let (styles, layout_tree) = if dom_was_mutated {
        recompute_styles_and_layout(&dom, ua, &stylesheet, &image_dims)
    } else {
        (styles, layout_tree)
    };

    LoadedDocument {
        html_source: html.to_string(),
        source_path: String::new(),
        tokens,
        dom,
        css_text,
        stylesheet,
        styles,
        layout_tree,
        parse_issues,
        images,
    }
}

// --- Setup phases ---
//
// Each phase below is a small named function decorated with
// `#[tracing::instrument]`. The instrumentation is invisible at
// the call site (see `parse_html_with_base_url` above): the
// orchestrator reads as a sequence of named function calls, and
// the span coverage comes from the attribute on each phase. Going
// finer than this means adding a phase (= adding a small named
// function), not sprinkling spans through existing code.

/// HTML tokenize + tree-build. Tokenize alone is fast; the tree
/// builder dominates.
#[tracing::instrument(name = "html_parse", skip_all)]
fn tokenize_and_parse(html: &str) -> (Vec<Token>, DomTree, Vec<String>) {
    let mut tokenizer = HTMLTokenizer::new(html.to_string());
    tokenizer.run();
    let tokens = tokenizer.into_tokens();
    let parser = HTMLParser::new(tokens.clone());
    let (dom, issues) = parser.run_with_issues();
    let parse_issues: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();
    (tokens, dom, parse_issues)
}

/// Walk the DOM for `<link rel="stylesheet">` + `<style>` elements
/// and merge their stylesheets. External-stylesheet HTTP fetches
/// happen here; on real pages that's often the dominant per-page
/// network cost.
///
/// TODO: Implement proper Fetch Standard and CSSOM spec compliance.
#[tracing::instrument(name = "css_extract", skip_all)]
fn extract_stylesheet(dom: &DomTree, base_url: Option<&str>) -> Stylesheet {
    let doc_stylesheets = extract_all_stylesheets(dom, base_url);
    doc_stylesheets.into_merged_stylesheet()
}

/// Initial cascade — compute styles for every element from the
/// merged author stylesheet + the UA stylesheet. Re-run after JS
/// DOM mutation in [`recompute_styles_and_layout`].
#[tracing::instrument(name = "css_cascade", skip_all)]
fn compute_initial_styles(
    dom: &DomTree,
    ua: &Stylesheet,
    stylesheet: &Stylesheet,
) -> HashMap<NodeId, ComputedStyle> {
    compute_styles(dom, ua, stylesheet)
}

/// Initial layout-tree build from the cascade result. Re-run after
/// JS DOM mutation in [`recompute_styles_and_layout`].
#[tracing::instrument(name = "layout_tree_build", skip_all)]
fn build_initial_layout_tree(
    dom: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    image_dims: &HashMap<NodeId, (f32, f32)>,
) -> Option<LayoutBox> {
    LayoutBox::build_layout_tree(dom, styles, dom.root(), image_dims)
}

/// Cascade + layout-tree-build redone against a post-JS DOM.
#[tracing::instrument(name = "post_js_relayout", skip_all)]
fn recompute_styles_and_layout(
    dom: &DomTree,
    ua: &Stylesheet,
    stylesheet: &Stylesheet,
    image_dims: &HashMap<NodeId, (f32, f32)>,
) -> (HashMap<NodeId, ComputedStyle>, Option<LayoutBox>) {
    let post_styles = compute_styles(dom, ua, stylesheet);
    let post_layout = LayoutBox::build_layout_tree(dom, &post_styles, dom.root(), image_dims);
    (post_styles, post_layout)
}

// --- JS lifecycle phases ---
//
// HTML § 13.2.6 "Stop parsing" lifecycle:
//   1. Run sync scripts
//   2. Fire DOMContentLoaded at the document
//   3. Drain the task queue (setTimeout / setInterval callbacks)
//   4. Fire load at the window
//   5. Drain anything queued by load handlers
//
// Errors thrown by listeners or timer callbacks are recorded as
// parse issues rather than aborting the document.

/// Top-level wrapper for the JS lifecycle. Returns `true` if the
/// DOM was mutated during script execution — the caller uses that
/// to decide whether to re-cascade + re-layout.
#[tracing::instrument(name = "js_execute", skip_all)]
fn execute_document_scripts<H: JsHooks>(
    dom_cell: &std::rc::Rc<std::cell::RefCell<DomTree>>,
    scripts: Vec<LoadedScript>,
    base_url: Option<&str>,
    hooks: &mut H,
    parse_issues: &mut Vec<String>,
) -> bool {
    let mut runtime = init_js_runtime(dom_cell, base_url, hooks);
    execute_inline_scripts(&mut runtime, scripts, parse_issues);
    dispatch_dcl(&mut runtime, parse_issues);
    drain_due_tasks(&mut runtime, parse_issues);
    dispatch_load(&mut runtime, parse_issues);
    pump_until_idle(&mut runtime, hooks, parse_issues);
    after_settled(&mut runtime, hooks);
    runtime.take_dom_dirty()
}

/// Construct the `JsRuntime`, plumb the document URL into
/// `location.href` so scripts see the real URL instead of the
/// default `about:blank`, and run the `before_scripts` hook —
/// the install point for callers that need extra JS-side globals
/// (the WPT testharness bridge, in particular).
#[tracing::instrument(name = "js_runtime_init", skip_all)]
fn init_js_runtime<H: JsHooks>(
    dom_cell: &std::rc::Rc<std::cell::RefCell<DomTree>>,
    base_url: Option<&str>,
    hooks: &mut H,
) -> JsRuntime {
    let mut runtime = JsRuntime::new(std::rc::Rc::clone(dom_cell));
    if let Some(url) = base_url {
        runtime.set_location(url);
    }
    hooks.before_scripts(&mut runtime);
    runtime
}

/// Execute each `<script>`'s body in document order. Errors get
/// dispatched as `window.error` events (so testharness.js's
/// failure path triggers) and recorded as parse issues for the
/// human-facing diagnostic stream.
#[tracing::instrument(name = "js_script_execute", skip_all)]
fn execute_inline_scripts(
    runtime: &mut JsRuntime,
    scripts: Vec<LoadedScript>,
    parse_issues: &mut Vec<String>,
) {
    for script in scripts {
        if let Err(e) = runtime.execute(&script.source) {
            let message = format!(
                "JavaScript error (in {label}): {e}",
                label = script.label,
            );
            if let Err(dispatch_err) = runtime.dispatch_error(&message) {
                parse_issues
                    .push(format!("JavaScript error (in error handler): {dispatch_err}"));
            }
            parse_issues.push(message);
        }
    }
}

#[tracing::instrument(name = "js_dispatch_dcl", skip_all)]
fn dispatch_dcl(runtime: &mut JsRuntime, parse_issues: &mut Vec<String>) {
    if let Err(e) = runtime.dispatch_dom_content_loaded() {
        parse_issues.push(format!("JavaScript error (in DOMContentLoaded): {e}"));
    }
}

/// Process currently-due `setTimeout` / `setInterval` callbacks
/// without sleeping for future timers. Sleeping here would force
/// every WPT testharness run to wait the harness's watchdog
/// `setTimeout` (10 s × `timeout_multiplier`) before firing
/// `load` — the watchdog is scheduled at testharness.js
/// IIFE-load time and is the longest single timer in the queue.
/// Future-timer waiting belongs in [`pump_until_idle`] instead,
/// where `should_stop_pumping` can exit early once a hook
/// signals it's done.
#[tracing::instrument(name = "js_drain_due_tasks", skip_all)]
fn drain_due_tasks(runtime: &mut JsRuntime, parse_issues: &mut Vec<String>) {
    if let Err(e) = runtime.drain_due_tasks() {
        parse_issues.push(format!("JavaScript error (in timer): {e}"));
    }
}

#[tracing::instrument(name = "js_dispatch_load", skip_all)]
fn dispatch_load(runtime: &mut JsRuntime, parse_issues: &mut Vec<String>) {
    if let Err(e) = runtime.dispatch_load() {
        parse_issues.push(format!("JavaScript error (in load): {e}"));
    }
}

/// The post-load pump — drains tasks until the queue settles or
/// until `hooks.should_stop_pumping` returns true. Any
/// `setTimeout(fn, N)` scheduled by document scripts causes the
/// pump to sleep until N elapses; on real pages with watchdog
/// timers this often dominates total `js_execute` cost. See
/// `project-memory/render-pipeline-levers.md` for the
/// decoupled-load contract that fixes this architecturally.
#[tracing::instrument(name = "js_pump_until_idle", skip_all)]
fn pump_until_idle<H: JsHooks>(
    runtime: &mut JsRuntime,
    hooks: &mut H,
    parse_issues: &mut Vec<String>,
) {
    let pump_result = runtime.pump_until_idle_or(|rt| hooks.should_stop_pumping(rt));
    if let Err(e) = pump_result {
        parse_issues.push(format!("JavaScript error (in timer): {e}"));
    }
}

/// Hook point: caller drains any state it accumulated during
/// script execution (testharness results, custom globals, etc.)
/// before the runtime drops.
#[tracing::instrument(name = "js_after_settled", skip_all)]
fn after_settled<H: JsHooks>(runtime: &mut JsRuntime, hooks: &mut H) {
    hooks.after_settled(runtime);
}

/// Load images referenced by `<img>` elements in the DOM.
///
/// [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
///
/// Walks the DOM for `<img>` elements with a `src` attribute, fetches the
/// image data (network or filesystem), and decodes it to RGBA pixels.
///
/// Uses [`ImageLoaderPipeline`] to detect format (SVG vs raster) and
/// dispatch to the appropriate decoder.
///
/// Returns:
/// - A map of src → `LoadedImage` for the renderer
/// - A map of `NodeId` → (width, height) for layout intrinsic dimensions
#[tracing::instrument(name = "image_loading", skip_all)]
fn load_images(
    dom: &DomTree,
    base_url: Option<&str>,
) -> (HashMap<String, LoadedImage>, HashMap<NodeId, (f32, f32)>) {
    let mut images: HashMap<String, LoadedImage> = HashMap::new();
    let mut image_dims: HashMap<NodeId, (f32, f32)> = HashMap::new();
    let pipeline = ImageLoaderPipeline::new();

    for node_id in dom.iter_all() {
        if let Some(element) = dom.as_element(node_id) {
            if !element.tag_name.eq_ignore_ascii_case("img") {
                continue;
            }

            let Some(src) = element.attrs.get("src") else {
                continue;
            };
            let src = src.trim();
            if src.is_empty() {
                continue;
            }

            // If we already loaded this src, just record its dims for this node.
            if let Some(existing) = images.get(src) {
                let _ = image_dims.insert(node_id, existing.dimensions_f32());
                continue;
            }

            // Resolve URL.
            let resolved = koala_common::url::resolve_url(src, base_url);

            // Strip query/fragment for extension-based format detection.
            let path_for_ext = strip_url_decorations(&resolved);

            // Emit warnings for unhandled URL decorations.
            warn_url_decorations(src, &resolved);

            // Fetch bytes (HTTP / data URL / local file).
            let bytes = match fetch_image_bytes(&resolved) {
                Ok(b) => b,
                Err(e) => {
                    if !warning::is_quiet() {
                        eprintln!("[Koala] Warning: failed to load image '{src}': {e}");
                    }
                    continue;
                }
            };

            // Detect format and decode.
            match pipeline.decode(&bytes, path_for_ext, &resolved) {
                Ok(loaded) => {
                    let _ = image_dims.insert(node_id, loaded.dimensions_f32());
                    let _ = images.insert(src.to_string(), loaded);
                }
                Err(e) => {
                    if !warning::is_quiet() {
                        eprintln!(
                            "[Koala] Warning: skipping <img src=\"{src}\">: {e}. \
                             The page will still render but this image will be missing."
                        );
                    }
                }
            }
        }
    }

    (images, image_dims)
}

/// One script extracted from the document, ready to feed
/// [`JsRuntime::execute`].
///
/// `source` is the UTF-8 JavaScript text. `label` is a
/// human-readable diagnostic tag — `"inline"` for inline
/// `<script>` blocks, or the resolved URL for fetched external
/// scripts — included in any error message the runtime emits
/// so a stack trace points at the right place.
struct LoadedScript {
    source: String,
    label: String,
}

/// Walk the DOM for `<script>` elements in tree order, fetching
/// each `src=`'d script's body and collecting each inline
/// script's text content.
///
/// [§ 4.12.1 The script element](https://html.spec.whatwg.org/multipage/scripting.html#the-script-element)
///
/// Order of the returned `Vec` matches document order — the
/// caller is expected to execute each script in that order, the
/// "classic script, parse-blocking" path from § 4.12.1.1. We
/// don't actually interleave with parsing (the parse is finished
/// before this runs), so the parse-time-side-effects of executing
/// a `document.write`-style script are out of scope. `async` and
/// `defer` attributes are recognized but treated as synchronous
/// for now; real ordering is deferred until tests demand it.
///
/// Fetch failures are appended to `issues` rather than aborting
/// the document load — the rest of the page still renders, the
/// script just doesn't run.
#[tracing::instrument(name = "script_loading", skip_all)]
fn load_scripts(
    dom: &DomTree,
    base_url: Option<&str>,
    issues: &mut Vec<String>,
) -> Vec<LoadedScript> {
    let mut scripts = Vec::new();

    for node_id in dom.iter_all() {
        let Some(element) = dom.as_element(node_id) else {
            continue;
        };
        if !element.tag_name.eq_ignore_ascii_case("script") {
            continue;
        }

        // External script: `src=` present. Fetch the body and
        // record either the source or a parse issue.
        if let Some(src) = element.attrs.get("src") {
            let src_trim = src.trim();
            if src_trim.is_empty() {
                continue;
            }
            let resolved = koala_common::url::resolve_url(src_trim, base_url);
            match fetch_script_source(&resolved) {
                Ok(source) => scripts.push(LoadedScript {
                    source,
                    label: resolved,
                }),
                Err(reason) => {
                    issues.push(format!(
                        "Failed to load <script src=\"{src_trim}\">: {reason}"
                    ));
                }
            }
            continue;
        }

        // Inline script: concatenate child text nodes per
        // § 4.12.1.3 ("the script block's source is the value
        // of the text content"). Empty inline blocks are
        // skipped — passing an empty string to the runtime
        // is a no-op and would just clutter diagnostics.
        let mut inline = String::new();
        for child_id in dom.children(node_id) {
            if let Some(text) = dom.as_text(*child_id) {
                inline.push_str(text);
            }
        }
        if !inline.is_empty() {
            scripts.push(LoadedScript {
                source: inline,
                label: "inline".into(),
            });
        }
    }

    scripts
}

/// Fetch the body of an external script as a UTF-8 string.
///
/// Mirrors `image_loader::fetch_image_bytes` but stays in UTF-8
/// land — `<script>` resources are always character data per
/// HTML § 4.12.1.1.6 ("Decoding the response's body as
/// UTF-8"). Invalid UTF-8 is replaced with `U+FFFD` rather than
/// rejected, matching the spec's lossy decode.
fn fetch_script_source(resolved_url: &str) -> Result<String, String> {
    let bytes = if resolved_url.starts_with("http://") || resolved_url.starts_with("https://")
    {
        koala_common::net::fetch_bytes(resolved_url).map_err(|e| e.to_string())?
    } else if resolved_url.starts_with("data:") {
        koala_common::net::fetch_bytes_from_data_url(resolved_url)
            .map_err(|e| e.to_string())?
    } else {
        // Strip the `file://` scheme for fs access — same shape
        // as the top-level document loader. Plain absolute paths
        // pass through unchanged.
        let fs_path = resolved_url.strip_prefix("file://").unwrap_or(resolved_url);
        fs::read(fs_path).map_err(|e| format!("{resolved_url}: {e}"))?
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Try to load a system font for text measurement and rendering.
///
/// Searches common system font paths (macOS, Linux, Windows) and returns
/// the first font that loads successfully, or None if no font is found.
#[must_use]
pub fn load_system_font() -> Option<fontdue::Font> {
    Renderer::load_system_font()
}

/// Create a [`FontMetrics`](koala_css::FontMetrics) provider, using real
/// font metrics if a font is available, falling back to approximation.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
#[must_use]
pub fn create_font_metrics(font: Option<&fontdue::Font>) -> Box<dyn koala_css::FontMetrics + '_> {
    match font {
        Some(f) => Box::new(font_metrics::FontdueFontMetrics::new(f)),
        None => Box::new(koala_css::ApproximateFontMetrics),
    }
}

/// Opaque font handle for text measurement during layout.
///
/// Wraps the underlying font library so that downstream crates (koala-gui,
/// koala-cli) can use real font metrics without depending on fontdue directly.
///
/// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
///
/// "CSS assumes that every font has font metrics that specify a
/// characteristic height above the baseline and a depth below it."
pub struct FontProvider {
    /// The loaded system font, if one was found.
    font: Option<fontdue::Font>,
}

impl FontProvider {
    /// Load a system font for text measurement.
    ///
    /// Searches common system font paths and loads the first one found.
    /// If no font is available, [`metrics()`](Self::metrics) will return
    /// an approximate metrics provider.
    #[must_use]
    pub fn load() -> Self {
        Self {
            font: Renderer::load_system_font(),
        }
    }

    /// Create a [`FontMetrics`](koala_css::FontMetrics) provider from this font.
    ///
    /// Returns real per-glyph metrics if a font was loaded, or an
    /// approximation (0.6 × font size per character) otherwise.
    #[must_use]
    pub fn metrics(&self) -> Box<dyn koala_css::FontMetrics + '_> {
        create_font_metrics(self.font.as_ref())
    }
}
