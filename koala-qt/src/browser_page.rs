// BrowserPage — Rust side of the viewport rendering bridge.
//
// A `BrowserPage` owns two worker threads so that neither network
// I/O nor CPU rasterization ever runs on the Qt GUI thread:
//
//   - **render worker**: takes a `RenderJob { Arc<PageState>, w, h }`,
//     runs layout → paint → rasterize, and emits a `RenderResult`
//     with the raw RGBA bytes.
//   - **loader worker**: takes a `LoadRequest { url }`, does the
//     blocking HTTP fetch + HTML parse + cascade + layout-tree
//     build, and emits a fresh `Arc<PageState>` via `LoadResult`.
//
// The two threads are independent. A URL load never blocks the
// rasterizer — the old page keeps rendering cleanly while the new
// page is being fetched, and once the new `PageState` lands the
// GUI schedules a fresh render.
//
// The C++ `BrowserView` widget drives the flow: it calls
// `request_render(w, h)` and `request_load(url)` from the Qt
// thread (both non-blocking — they return as soon as the job is
// in the queue) and polls `try_take_render_result` /
// `try_take_load_result` from a `QTimer` slot at ~60 Hz.
//
// The JavaScript runtime that koala-browser builds during parse is
// discarded here. Boa's `Context` is not `Send`, which would prevent
// the rest of the document from crossing the thread boundary. Since
// koala-qt has no DOM↔JS bindings yet, keeping the runtime past
// parse time provides no value anyway; we run inline scripts inside
// `parse_html_string` / `load_document` and drop the interpreter
// immediately after.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};

use koala_browser::css::{
    ComputedStyle, DisplayListBuilder, LayoutBox, Rect, canvas_background,
};
use koala_browser::dom::{DomTree, NodeId};
use koala_browser::{
    FontProvider, LoadedDocument, LoadedImage, Renderer, RendererFonts, load_document,
    parse_html_string,
};

use crate::bridge::ffi::{LoadPollResult, RenderResult};

// Process-wide `RendererFonts` cache. Loading four font files from
// disk costs ~250 ms on macOS; doing it per render was the dominant
// cost before task #6. Now loaded exactly once, shared across every
// `BrowserPage` and every render via `Arc` clones.
fn cached_fonts() -> &'static RendererFonts {
    static FONTS: OnceLock<RendererFonts> = OnceLock::new();
    FONTS.get_or_init(RendererFonts::from_system)
}

// Process-wide `FontProvider` for layout-time metrics. Separate from
// `RendererFonts` because koala-css's `FontMetrics` trait is keyed on
// a single `fontdue::Font` handle that `FontProvider` owns.
fn cached_font_provider() -> &'static FontProvider {
    static PROVIDER: OnceLock<FontProvider> = OnceLock::new();
    PROVIDER.get_or_init(FontProvider::load)
}

/// The Send-able subset of `LoadedDocument` needed to render a page.
///
/// Excludes the JS runtime, the raw token stream, the parsed
/// stylesheet AST, the HTML source, and parse diagnostics — nothing
/// in that list is consulted after layout. Everything here is
/// `Send + Sync`, so an `Arc<PageState>` can cross thread boundaries
/// without copying the underlying data.
struct PageState {
    dom: DomTree,
    styles: std::collections::HashMap<NodeId, ComputedStyle>,
    layout_tree: LayoutBox,
    images: std::collections::HashMap<String, LoadedImage>,
}

impl PageState {
    /// Destructure a freshly-parsed `LoadedDocument` into the
    /// render-relevant fields, dropping the JS runtime and other
    /// parse-time debris. Returns `None` if the document produced
    /// no layout tree (happens only on pathological input).
    fn from_document(doc: LoadedDocument) -> Option<Self> {
        doc.layout_tree.map(|layout_tree| Self {
            dom: doc.dom,
            styles: doc.styles,
            layout_tree,
            images: doc.images,
        })
    }
}

/// A single render request sent from the GUI thread to the render worker.
struct RenderJob {
    state: Arc<PageState>,
    width: u32,
    height: u32,
}

/// A single load request sent from the GUI thread to the loader worker.
struct LoadRequest {
    url: String,
}

/// The loader worker's reply. Carries either a new page state or an
/// error message — the GUI thread swaps the state in on success and
/// logs the error on failure.
enum LoadResult {
    Loaded { url: String, state: Arc<PageState> },
    Failed { url: String, error: String },
}

pub struct BrowserPage {
    // None until the first `load_html` / `load_landing_page` call
    // (or a successful URL load). Replaced wholesale on navigation;
    // old `Arc`s held by in-flight render jobs stay alive until
    // those jobs complete.
    state: Option<Arc<PageState>>,

    // The URL of the most-recently-committed load, if any. Used by
    // `reload_current_url` to re-fetch the same address.
    current_url: Option<String>,

    // Channels to and from the render worker.
    render_job_tx: Sender<RenderJob>,
    render_result_rx: Receiver<RenderResult>,
    _render_worker: JoinHandle<()>,

    // Channels to and from the loader worker.
    load_request_tx: Sender<LoadRequest>,
    load_result_rx: Receiver<LoadResult>,
    _load_worker: JoinHandle<()>,
}

impl BrowserPage {
    fn new() -> Self {
        let (render_job_tx, render_job_rx) = mpsc::channel::<RenderJob>();
        let (render_result_tx, render_result_rx) = mpsc::channel::<RenderResult>();
        let render_worker = thread::Builder::new()
            .name("koala-qt-render".to_owned())
            .spawn(move || run_render_worker(&render_job_rx, &render_result_tx))
            .expect("failed to spawn koala-qt render worker");

        let (load_request_tx, load_request_rx) = mpsc::channel::<LoadRequest>();
        let (load_result_tx, load_result_rx) = mpsc::channel::<LoadResult>();
        let load_worker = thread::Builder::new()
            .name("koala-qt-loader".to_owned())
            .spawn(move || run_load_worker(&load_request_rx, &load_result_tx))
            .expect("failed to spawn koala-qt loader worker");

        Self {
            state: None,
            current_url: None,
            render_job_tx,
            render_result_rx,
            _render_worker: render_worker,
            load_request_tx,
            load_result_rx,
            _load_worker: load_worker,
        }
    }

    /// Parses `html` on the calling thread and replaces the current
    /// page state synchronously. Used for the built-in landing page
    /// and for any caller that already has the raw HTML in hand.
    pub fn load_html(&mut self, html: &str) {
        self.state = PageState::from_document(parse_html_string(html)).map(Arc::new);
        self.current_url = None;
    }

    /// Loads the built-in landing page (see `landing.rs`). Runs
    /// synchronously — no worker hop, since the HTML is already in
    /// memory and the parse is ~3 ms.
    pub fn load_landing_page(&mut self) {
        self.load_html(crate::landing::LANDING_HTML);
    }

    /// Queues a URL load on the loader worker. Returns immediately;
    /// the result lands in the result channel and is picked up by
    /// the next `try_take_load_result` call from the GUI thread.
    ///
    /// The loader handles `http://`, `https://`, and bare filesystem
    /// paths (forwarded to `koala_browser::load_document`).
    pub fn request_load(&self, url: &str) {
        let _ = self.load_request_tx.send(LoadRequest {
            url: url.to_owned(),
        });
    }

    /// Re-queues the most recently committed URL, if any. Returns
    /// `true` when a request was actually queued. `false` means
    /// the current page came from `load_html` / the built-in
    /// landing page and there is nothing to re-fetch.
    pub fn reload_current_url(&self) -> bool {
        let Some(url) = self.current_url.as_ref() else {
            return false;
        };
        self.load_request_tx
            .send(LoadRequest { url: url.clone() })
            .is_ok()
    }

    /// Non-blocking check for a completed URL load. Both successful
    /// and failed loads set `load_finished = true` so the GUI can
    /// hide any spinner; only successes set `state_swapped = true`.
    pub fn try_take_load_result(&mut self) -> LoadPollResult {
        match self.load_result_rx.try_recv() {
            Ok(LoadResult::Loaded { url, state }) => {
                self.state = Some(state);
                self.current_url = Some(url);
                LoadPollResult {
                    state_swapped: true,
                    load_finished: true,
                }
            }
            Ok(LoadResult::Failed { url, error }) => {
                eprintln!("[koala-qt] load failed for {url}: {error}");
                LoadPollResult {
                    state_swapped: false,
                    load_finished: true,
                }
            }
            Err(_) => LoadPollResult {
                state_swapped: false,
                load_finished: false,
            },
        }
    }

    /// Queues a render job on the render worker thread. Returns
    /// immediately regardless of how long the render will take.
    /// No-op when there is no current state or the requested
    /// dimensions are degenerate.
    pub fn request_render(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let _ = self.render_job_tx.send(RenderJob {
            state: Arc::clone(state),
            width,
            height,
        });
    }

    /// Non-blocking check for a finished frame. Returns an empty
    /// `RenderResult` (`pixels.len() == 0`) when no frame is ready.
    /// Intended to be called from a Qt timer slot at ~60 Hz.
    pub fn try_take_render_result(&self) -> RenderResult {
        self.render_result_rx.try_recv().unwrap_or(RenderResult {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        })
    }
}

/// Render-worker entry point. Drains the job queue on each wake-up,
/// coalescing queued jobs so we only render the most recent
/// dimensions. On rapid resizes this keeps us from rendering every
/// intermediate frame.
fn run_render_worker(
    job_rx: &Receiver<RenderJob>,
    result_tx: &Sender<RenderResult>,
) {
    while let Ok(first) = job_rx.recv() {
        let mut latest = first;
        while let Ok(newer) = job_rx.try_recv() {
            latest = newer;
        }
        let pixels = render_state(&latest.state, latest.width, latest.height);
        if result_tx
            .send(RenderResult {
                width: latest.width,
                height: latest.height,
                pixels,
            })
            .is_err()
        {
            break;
        }
    }
}

/// Loader-worker entry point. Fetches each requested URL via
/// `koala_browser::load_document` (blocking HTTP on this thread
/// only) and turns the resulting document into an `Arc<PageState>`.
fn run_load_worker(
    request_rx: &Receiver<LoadRequest>,
    result_tx: &Sender<LoadResult>,
) {
    while let Ok(req) = request_rx.recv() {
        let result = match load_document(&req.url) {
            Ok(doc) => match PageState::from_document(doc) {
                Some(state) => LoadResult::Loaded {
                    url: req.url,
                    state: Arc::new(state),
                },
                None => LoadResult::Failed {
                    url: req.url,
                    error: "document produced no layout tree".to_owned(),
                },
            },
            Err(e) => LoadResult::Failed {
                url: req.url,
                error: e.to_string(),
            },
        };
        if result_tx.send(result).is_err() {
            break;
        }
    }
}

/// The full layout → paint → rasterize pipeline, taking a borrowed
/// `PageState` rather than `&self` so it can run off the main thread.
fn render_state(state: &PageState, width: u32, height: u32) -> Vec<u8> {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
    };

    let mut layout = state.layout_tree.clone();
    let font_metrics = cached_font_provider().metrics();
    layout.layout(viewport, viewport, &*font_metrics, viewport);

    let builder = DisplayListBuilder::new(&state.styles);
    let display_list = builder.build(&layout);

    let mut renderer = Renderer::new_with_fonts(
        width,
        height,
        state.images.clone(),
        cached_fonts().clone(),
    );

    // Propagate the canvas background (CSS 2.1 § 14.2) so regions
    // of the viewport not covered by painted content still show the
    // html/body background colour instead of Renderer's default
    // white fill.
    if let Some(bg) = canvas_background(&state.dom, &state.styles) {
        renderer.set_canvas_background(bg);
    }

    renderer.render(&display_list);
    renderer.rgba_bytes().to_vec()
}

/// Factory used by the cxx bridge.
pub fn new_browser_page() -> Box<BrowserPage> {
    Box::new(BrowserPage::new())
}
