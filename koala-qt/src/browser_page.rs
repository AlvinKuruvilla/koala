// BrowserPage — Rust side of the viewport rendering bridge.
//
// The page engine runs on a dedicated worker thread so the Qt GUI
// loop stays responsive while the software rasterizer is busy. A
// `BrowserPage` instance holds:
//
//   - The parsed `PageState` (DOM, styles, layout tree, images),
//     wrapped in `Arc` so it can be shared cheaply with the worker
//     thread on every render request.
//   - A `Sender<RenderJob>` for posting work to the worker.
//   - A `Receiver<RenderResult>` for picking up finished frames.
//
// The C++ `BrowserView` widget drives the flow: it calls
// `request_render(w, h)` from the Qt thread (non-blocking — the call
// returns as soon as the job is in the queue) and polls
// `try_take_render_result` from a `QTimer` slot, painting whatever
// new frame has arrived. When the user resizes rapidly, the worker
// coalesces queued jobs and only renders the most recent dimensions,
// so intermediate sizes never hit the rasterizer.
//
// The JavaScript runtime that koala-browser builds during parse is
// discarded here. Boa's `Context` is not `Send`, which would prevent
// the rest of the document from crossing the thread boundary. Since
// koala-qt has no DOM↔JS bindings yet, keeping the runtime past
// parse time provides no value anyway; we run inline scripts inside
// `parse_html_string` and drop the interpreter immediately after.

use std::sync::{Arc, OnceLock};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use koala_browser::css::{
    ComputedStyle, DisplayListBuilder, LayoutBox, Rect, canvas_background,
};
use koala_browser::dom::{DomTree, NodeId};
use koala_browser::{
    FontProvider, LoadedImage, Renderer, RendererFonts, parse_html_string,
};

use crate::bridge::ffi::RenderResult;

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

/// A single render request sent from the main thread to the worker.
struct RenderJob {
    state: Arc<PageState>,
    width: u32,
    height: u32,
}

pub struct BrowserPage {
    // None until the first `load_html` / `load_landing_page` call.
    // Replaced wholesale on navigation; old `Arc`s held by in-flight
    // render jobs stay alive until those jobs complete.
    state: Option<Arc<PageState>>,

    // Channels to and from the render worker. The worker lives on a
    // dedicated thread spawned in `BrowserPage::new`.
    job_tx: Sender<RenderJob>,
    result_rx: Receiver<RenderResult>,

    // Owned handle to the worker thread. Kept alive so the thread
    // isn't detached; drops implicitly when `BrowserPage` drops,
    // after the `job_tx` drop signals the worker to shut down.
    _worker: JoinHandle<()>,
}

impl BrowserPage {
    fn new() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<RenderJob>();
        let (result_tx, result_rx) = mpsc::channel::<RenderResult>();
        let worker = thread::Builder::new()
            .name("koala-qt-render".to_owned())
            .spawn(move || run_render_worker(&job_rx, &result_tx))
            .expect("failed to spawn koala-qt render worker");
        Self {
            state: None,
            job_tx,
            result_rx,
            _worker: worker,
        }
    }

    /// Parses `html` and replaces the current page state. The JS
    /// runtime that koala-browser builds during parse is dropped
    /// here — see the module comment for why.
    pub fn load_html(&mut self, html: &str) {
        let doc = parse_html_string(html);
        self.state = doc.layout_tree.map(|layout_tree| {
            Arc::new(PageState {
                dom: doc.dom,
                styles: doc.styles,
                layout_tree,
                images: doc.images,
            })
        });
    }

    /// Loads the built-in landing page (see `landing.rs`).
    pub fn load_landing_page(&mut self) {
        self.load_html(crate::landing::LANDING_HTML);
    }

    /// Queues a render job on the worker thread. Returns immediately
    /// regardless of how long the render will take. Does nothing when
    /// there is no current page state or the requested dimensions are
    /// degenerate.
    pub fn request_render(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let Some(state) = self.state.as_ref() else {
            return;
        };
        // If the worker has exited (only possible if the thread
        // panicked — we never drop `job_tx` until `self` drops), let
        // the error propagate silently. The caller can't do anything
        // useful with it.
        let _ = self.job_tx.send(RenderJob {
            state: Arc::clone(state),
            width,
            height,
        });
    }

    /// Non-blocking check for a finished frame. Returns an empty
    /// `RenderResult` (`pixels.len() == 0`) when no frame is ready.
    /// Intended to be called from a Qt timer slot at ~60 Hz.
    pub fn try_take_render_result(&self) -> RenderResult {
        self.result_rx.try_recv().unwrap_or(RenderResult {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        })
    }
}

/// Worker-thread entry point. Drains the job queue on each wake-up,
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
