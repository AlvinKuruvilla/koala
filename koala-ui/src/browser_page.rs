// BrowserPage — Rust side of the viewport rendering bridge.
//
// A `BrowserPage` owns two worker threads so that neither network
// I/O nor CPU rasterization ever runs on the Slint event loop:
//
//   - **render worker**: owns the page being shown. It takes
//     `WorkerMessage`s (a new page, a new viewport size, pointer
//     movement), runs restyle → layout → hit test → paint → rasterize
//     as they require, and emits a `RenderResult` with the raw RGBA
//     bytes.
//   - **loader worker**: takes a `LoadRequest { url }`, does the
//     blocking HTTP fetch + HTML parse + cascade + layout-tree
//     build, and emits a fresh `PageState` via `LoadResult`.
//
// Each lives in its own file, `render_worker.rs` and `load_worker.rs`,
// and the page they pass between them in `page_state.rs`. This file is
// the GUI thread's side: history, navigation, and the channels.
//
// The two threads are independent. A URL load never blocks the
// rasterizer — the old page keeps rendering cleanly while the new
// page is being fetched, and once the new `PageState` lands the
// GUI hands it to the render worker.
//
// The render worker is the page's only owner because pointer movement
// changes the page: the hovered node lives on the `DomTree`, and a
// change to it can mean a new cascade and layout. Every mature engine
// splits it the same way: one thread owns the DOM, style and layout,
// and input reaches it as messages, which it merges so that only the
// latest pointer position is handled.
//
// The Slint main loop drives the flow: it calls `request_render`,
// `pointer_moved` and `request_load` from the event-loop thread (all
// non-blocking — they return as soon as the message is in the queue)
// and polls `try_take_render_image` / `try_take_load_result` from a
// `slint::Timer` callback at ~60 Hz.
//
// The JavaScript runtime that koala-browser builds during parse is
// discarded here. Boa's `Context` is not `Send`, which would prevent
// the rest of the document from crossing the thread boundary. Since
// koala-ui has no DOM↔JS bindings yet, keeping the runtime past
// parse time provides no value anyway; we run inline scripts inside
// `parse_html_string` / `load_document` and drop the interpreter
// immediately after.

use std::any::Any;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use koala_browser::parse_html_string;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::error_page::Failure;
use crate::load_worker::{LoadRequest, LoadResult, LoadSource, report_problems, run_load_worker};
use crate::page_state::PageState;
use crate::render_worker::{RenderResult, WorkerMessage, run_render_worker};

/// Stack size for the loader and render workers: 16 MiB, the size Boa
/// gives its own CLI for the same reason (boa-dev/boa#5537).
///
/// Boa parses JavaScript by recursive descent with no depth limit, about
/// fifteen frames per level of expression nesting. Measured 2026-09-26
/// on google.com's 1.1 MB script bundle, whose deepest nesting is only 19
/// brackets: a debug build needs 2-4 MiB (frames up to 20 KB each), a
/// release build 256-512 KiB. Rust's default for spawned threads is
/// 2 MiB, so debug builds overflowed and aborted the whole process.
///
/// 16 MiB leaves 4x headroom over debug google. The size only reserves
/// address space; memory is committed as the stack is actually used.
///
/// NOTE: this raises the threshold, it does not remove it. A script nested
/// deeply enough still overflows any stack and aborts the process, because
/// Boa's parser has no recursion guard. The real fix is one in the parser
/// (JavaScriptCore's turns this into a `RangeError`).
const WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Summary returned by `try_take_load_result`. The GUI side uses
/// `state_swapped` to decide whether to trigger a fresh render, and
/// `load_finished` to toggle any loading indicator (success and
/// failure both count as "finished" — the engine is no longer working
/// on that request).
pub struct LoadPollResult {
    /// `true` when the loader worker delivered a new page state on
    /// this poll (successful load only).
    pub state_swapped: bool,
    /// `true` when a load request completed (either with a new state
    /// or with an error). `false` means no load event was pending.
    pub load_finished: bool,
}

/// A rendered frame, ready to display.
pub struct Frame {
    pub image: Image,
    /// The [`BrowserPage::state_generation`] of the page state it shows.
    pub generation: u64,
}

/// A single entry in the per-tab history stack.
///
/// We need more than a flat `Vec<String>` because the built-in
/// landing page is also a history entry the user can navigate back
/// to — it lives in memory and has no URL. The enum lets `go_back`
/// dispatch to either the loader worker (for `Url`) or a
/// synchronous in-process render (for `Landing`).
#[derive(Clone)]
enum HistoryEntry {
    Url(String),
    Landing,
}

pub struct BrowserPage {
    // The current page's title. The page itself belongs to the render
    // worker once `set_state` hands it over.
    title: String,

    // Counts page replacements; see `state_generation`. Changed only
    // through `set_state`.
    state_generation: u64,

    // The URL of the most-recently-committed load, if any. Used by
    // `reload_current_url` to re-fetch the same address.
    current_url: Option<String>,

    // Per-tab history stack. `history[history_index]` is the entry
    // currently being displayed. Entries can be either URLs (loaded
    // through the loader worker) or the built-in landing page, so
    // a user can Back from their first navigated URL to the new-tab
    // page the same way every mainstream browser allows.
    //
    // A fresh user navigation truncates everything after
    // `history_index` before pushing the new entry, matching how
    // real browsers drop the "forward" history when you navigate
    // somewhere new from a back-stack position.
    history: Vec<HistoryEntry>,
    history_index: Option<usize>,

    // Channels to and from the render worker.
    worker_tx: Sender<WorkerMessage>,
    render_result_rx: Receiver<RenderResult>,
    _render_worker: JoinHandle<()>,

    // Channels to and from the loader worker.
    load_request_tx: Sender<LoadRequest>,
    load_result_rx: Receiver<LoadResult>,
    // Second sender on the same channel as `load_result_rx`, held
    // by the GUI thread so `go_back` / `go_forward` can inject
    // synthetic results when a history entry is the in-memory
    // landing page rather than a URL the worker would fetch.
    load_result_tx_main: Sender<LoadResult>,
    _load_worker: JoinHandle<()>,
}

impl BrowserPage {
    pub fn new() -> Self {
        let (worker_tx, worker_rx) = mpsc::channel::<WorkerMessage>();
        let (render_result_tx, render_result_rx) = mpsc::channel::<RenderResult>();
        let render_worker = thread::Builder::new()
            .name("koala-ui-render".to_owned())
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || run_render_worker(&worker_rx, &render_result_tx))
            .expect("failed to spawn koala-ui render worker");

        let (load_request_tx, load_request_rx) = mpsc::channel::<LoadRequest>();
        let (load_result_tx, load_result_rx) = mpsc::channel::<LoadResult>();
        let load_result_tx_for_worker = load_result_tx.clone();
        let load_worker = thread::Builder::new()
            .name("koala-ui-loader".to_owned())
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || run_load_worker(&load_request_rx, &load_result_tx_for_worker))
            .expect("failed to spawn koala-ui loader worker");

        Self {
            title: String::new(),
            state_generation: 0,
            current_url: None,
            history: Vec::new(),
            history_index: None,
            worker_tx,
            render_result_rx,
            _render_worker: render_worker,
            load_request_tx,
            load_result_rx,
            load_result_tx_main: load_result_tx,
            _load_worker: load_worker,
        }
    }

    /// Replace the page, bump its generation, and hand the page to the
    /// render worker. With `None` (a document that produced no layout
    /// tree), the worker keeps showing the previous page.
    fn set_state(&mut self, state: Option<Box<PageState>>) {
        self.state_generation += 1;
        self.title = state.as_ref().map(|s| s.title.clone()).unwrap_or_default();
        if let Some(state) = state {
            let _ = self.worker_tx.send(WorkerMessage::Replace {
                state,
                generation: self.state_generation,
            });
        }
    }

    /// A number that increases every time the page state is replaced:
    /// by a finished load, the landing page, or an error page. Frames
    /// carry the generation they were rendered from, so the caller can
    /// tell a frame of the current page from a late frame of a previous
    /// one.
    pub fn state_generation(&self) -> u64 {
        self.state_generation
    }

    /// Parses `html` on the calling thread and replaces the current
    /// page state synchronously. Used for ad-hoc in-memory HTML
    /// (tests, debugging) — clears the history stack because the
    /// HTML has no identity the user could navigate back to.
    pub fn load_html(&mut self, html: &str) {
        self.set_state(PageState::from_document(parse_html_string(html)));
        self.current_url = None;
        self.history.clear();
        self.history_index = None;
    }

    /// Loads the built-in landing page (see `landing.rs`). Runs
    /// synchronously — no worker hop, since the HTML is already in
    /// memory and the parse is ~3 ms.
    ///
    /// Unlike `load_html`, this seeds the history stack with a
    /// sentinel `HistoryEntry::Landing` so pressing Back from the
    /// user's first fetched URL takes them back to the landing
    /// page instead of hitting a disabled button.
    pub fn load_landing_page(&mut self) {
        let doc = parse_html_string(crate::landing::LANDING_HTML);
        report_problems("the new tab page", &doc);
        self.set_state(PageState::from_document(doc));
        self.current_url = None;
        self.history.clear();
        self.history.push(HistoryEntry::Landing);
        self.history_index = Some(0);
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
            source: LoadSource::UserNavigation,
        });
    }

    /// Re-queues the most recently committed URL, if any. Returns
    /// `true` when a request was actually queued. `false` means
    /// the current page came from `load_html` / the built-in
    /// landing page and there is nothing to re-fetch.
    ///
    /// Reload is treated as a user navigation for history purposes:
    /// it replaces the current entry without advancing the stack,
    /// which happens naturally because `try_take_load_result`'s
    /// truncate-then-push logic bottoms out at the same index.
    pub fn reload_current_url(&self) -> bool {
        let Some(url) = self.current_url.as_ref() else {
            return false;
        };
        self.load_request_tx
            .send(LoadRequest {
                url: url.clone(),
                source: LoadSource::UserNavigation,
            })
            .is_ok()
    }

    /// Walks one step back in the history stack. Returns `true`
    /// when a back-navigation was actually issued (either as an
    /// async load request or a synthetic in-memory result for the
    /// landing-page entry); `false` when there's nothing earlier
    /// to navigate to.
    pub fn go_back(&mut self) -> bool {
        let Some(idx) = self.history_index else {
            return false;
        };
        if idx == 0 {
            return false;
        }
        let new_idx = idx - 1;
        self.history_index = Some(new_idx);
        self.navigate_to_entry(new_idx)
    }

    /// Walks one step forward in the history stack.
    pub fn go_forward(&mut self) -> bool {
        let Some(idx) = self.history_index else {
            return false;
        };
        if idx + 1 >= self.history.len() {
            return false;
        }
        let new_idx = idx + 1;
        self.history_index = Some(new_idx);
        self.navigate_to_entry(new_idx)
    }

    /// Dispatches on the history entry at `idx`: URLs go to the
    /// loader worker for an async re-fetch, landing-page entries
    /// are rendered synchronously on the calling thread and fed
    /// back through the result channel as a synthetic
    /// `LoadResult::Loaded` so the main poll loop picks them up
    /// through the same code path as a worker result.
    fn navigate_to_entry(&mut self, idx: usize) -> bool {
        match self.history[idx].clone() {
            HistoryEntry::Url(url) => self
                .load_request_tx
                .send(LoadRequest {
                    url,
                    source: LoadSource::HistoryNavigation,
                })
                .is_ok(),
            HistoryEntry::Landing => {
                let Some(state) =
                    PageState::from_document(parse_html_string(crate::landing::LANDING_HTML))
                else {
                    return false;
                };
                self.load_result_tx_main
                    .send(LoadResult {
                        url: String::new(),
                        source: LoadSource::HistoryNavigation,
                        state,
                    })
                    .is_ok()
            }
        }
    }

    /// Whether there is an earlier entry to navigate back to.
    /// Drives the Back toolbar action's enabled state.
    pub fn can_go_back(&self) -> bool {
        matches!(self.history_index, Some(i) if i > 0)
    }

    /// Whether there is a later entry to navigate forward to.
    /// Drives the Forward toolbar action's enabled state.
    pub fn can_go_forward(&self) -> bool {
        match self.history_index {
            Some(i) => i + 1 < self.history.len(),
            None => false,
        }
    }

    /// The most-recently-committed URL, if any. `None` when the
    /// current page came from `load_html` or the built-in landing
    /// page — those have no addressable identity. Used to keep the
    /// URL bar in sync after Back / Forward jumps.
    pub fn current_url(&self) -> Option<String> {
        self.current_url.clone()
    }

    /// The current page's `<title>` text, or an empty string when
    /// there is no current page or it has no title. Read after every
    /// load to update the tab label.
    pub fn current_title(&self) -> String {
        self.title.clone()
    }

    /// Non-blocking check for a completed URL load. The loader
    /// always produces a valid `PageState` (failures are turned
    /// into an error page inside the worker), so every received
    /// result counts as both `state_swapped` and `load_finished`.
    pub fn try_take_load_result(&mut self) -> LoadPollResult {
        let Ok(LoadResult { url, source, state }) = self.load_result_rx.try_recv() else {
            return LoadPollResult {
                state_swapped: false,
                load_finished: false,
            };
        };

        self.set_state(Some(state));

        match source {
            LoadSource::UserNavigation => {
                self.current_url = Some(url.clone());
                // Drop any "forward" entries past the current
                // position before pushing — a fresh navigation
                // from a back-stack position replaces the tail.
                if let Some(idx) = self.history_index {
                    self.history.truncate(idx + 1);
                }
                // A reload lands on the same URL we're already
                // displaying; avoid an infinitely growing
                // duplicate stack.
                let last_matches = matches!(
                    self.history.last(),
                    Some(HistoryEntry::Url(last)) if last == &url
                );
                if !last_matches {
                    self.history.push(HistoryEntry::Url(url));
                }
                self.history_index = Some(self.history.len() - 1);
            }
            LoadSource::HistoryNavigation => {
                // `go_back` / `go_forward` already advanced
                // `history_index` when they queued the request.
                // For a URL entry, the fetched URL is the new
                // `current_url`; for the synthetic landing-page
                // result, `url` is empty and there is no current
                // URL.
                self.current_url = if url.is_empty() { None } else { Some(url) };
            }
        }

        LoadPollResult {
            state_swapped: true,
            load_finished: true,
        }
    }

    /// Asks the render worker for a frame at this size, in physical
    /// pixels. Returns immediately regardless of how long the render
    /// will take. No-op when the dimensions are degenerate.
    pub fn request_render(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let _ = self.worker_tx.send(WorkerMessage::Resize { width, height });
    }

    /// Tells the render worker where the pointer is over the viewport, in
    /// physical pixels. A change of hovered element re-renders the page.
    pub fn pointer_moved(&self, x: f32, y: f32) {
        let _ = self.worker_tx.send(WorkerMessage::PointerMoved { x, y });
    }

    /// Tells the render worker the pointer left the viewport, so nothing
    /// on the page is hovered.
    pub fn pointer_left(&self) {
        let _ = self.worker_tx.send(WorkerMessage::PointerLeft);
    }

    /// Non-blocking check for a finished frame. Returns `None` when
    /// no frame is ready. Intended to be called from a `slint::Timer`
    /// at ~60 Hz. The frame's `generation` says which page state it
    /// shows, which may be older than the current one.
    ///
    /// When the render worker caught a panic while rasterising the
    /// current page (e.g. a slice out-of-bounds in real-world HTML),
    /// the worker sends back a result with `error` populated and no
    /// pixels. We handle that case here on the main thread by
    /// synthesising an error-page `PageState` and injecting it
    /// through the load channel, so the next poll tick swaps state
    /// and schedules a fresh render of the built-in error template.
    /// The caller always sees either a real frame or `None` — it
    /// never has to know about engine panics.
    pub fn try_take_render_image(&self) -> Option<Frame> {
        let Ok(result) = self.render_result_rx.try_recv() else {
            return None;
        };

        if !result.error.is_empty() {
            self.install_engine_error(&result.error);
            return None;
        }

        if result.pixels.is_empty() || result.width == 0 || result.height == 0 {
            return None;
        }

        // RGBA bytes are already in `Rgba8Pixel`'s memory layout
        // (R, G, B, A; 4 bytes per pixel, no padding). `make_mut_bytes`
        // exposes the buffer as a flat `&mut [u8]` of length
        // `width * height * 4`, so a single `copy_from_slice` does the
        // whole transfer — no per-pixel iteration, no allocation past
        // the initial buffer.
        let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(result.width, result.height);
        buf.make_mut_bytes().copy_from_slice(&result.pixels);
        Some(Frame {
            image: Image::from_rgba8(buf),
            generation: result.generation,
        })
    }

    /// Builds the error page for the currently-displayed URL and
    /// injects it into the load result channel. The main poll loop
    /// then picks it up on its next tick and runs it through the
    /// normal `try_take_load_result` code path, which swaps the
    /// `state` and updates the title for free.
    ///
    /// Marks the injection as `HistoryNavigation` so the existing
    /// history stack is left alone — the failing URL was already
    /// pushed (if it came from a user navigation) before the
    /// render worker took over, and we just want to replace the
    /// content shown at that entry.
    fn install_engine_error(&self, message: &str) {
        let url = self.current_url.clone().unwrap_or_default();
        let error_html = crate::error_page::render(&url, &Failure::Engine(message));
        let Some(state) = PageState::from_document(parse_html_string(&error_html)) else {
            eprintln!("[koala-ui] error template failed to parse; engine error was: {message}");
            return;
        };
        let _ = self.load_result_tx_main.send(LoadResult {
            url,
            source: LoadSource::HistoryNavigation,
            state,
        });
    }
}

/// Best-effort extraction of a human-readable message from a panic
/// payload. `catch_unwind` hands us a `Box<dyn Any>`, so we try the
/// two concrete types the standard library uses before falling back
/// to a placeholder.
pub(crate) fn panic_message(payload: &Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_owned()
    } else {
        "engine panicked (no message)".to_owned()
    }
}
