// The loader worker: fetches and parses a URL into a `PageState`, off
// the GUI thread.

use std::sync::mpsc::Sender;

use koala_browser::{LoadedDocument, load_document, parse_html_string};

use crate::error_page::Failure;
use crate::page_state::PageState;
use crate::worker::{Inbox, catch_panic};

/// Where a load request came from. Used by `try_take_load_result`
/// to decide whether to push onto the history stack (fresh
/// user-initiated navigation) or just swap state without touching
/// history (Back/Forward re-fetching an entry that's already there).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadSource {
    UserNavigation,
    HistoryNavigation,
}

/// A single load request sent from the GUI thread to the loader worker.
pub(crate) struct LoadRequest {
    pub(crate) url: String,
    pub(crate) source: LoadSource,
}

/// The loader worker's reply. Always carries a successful
/// `PageState` — load failures (HTTP errors, parse errors, caught
/// panics) are turned into an error-page `PageState` by the loader
/// worker itself, so the GUI thread only ever has to handle one
/// code path.
///
/// `BrowserPage` also injects results directly into the channel
/// from the GUI thread when the user hits Back/Forward onto a
/// `HistoryEntry::Landing` entry, or when the render worker
/// catches a panic and we need to swap state to the error page
/// without going through the loader.
pub(crate) struct LoadResult {
    pub(crate) url: String,
    pub(crate) source: LoadSource,
    pub(crate) state: Box<PageState>,
}

/// Print what went wrong while loading `doc` to stderr, headed with
/// `label` so concurrent loads can be told apart in the terminal. Prints
/// nothing for a clean load or under `--quiet`.
pub(crate) fn report_problems(label: &str, doc: &LoadedDocument) {
    if let Some(report) = doc.problem_report(label) {
        eprint!("{report}");
    }
}

/// Loader-worker entry point. Fetches each requested URL via
/// `koala_browser::load_document` (blocking HTTP on this thread
/// only) and turns the resulting document into a `PageState`.
///
/// The fetch + parse runs inside `catch_unwind` so a panic in any
/// of koala-browser's dependencies (HTML parser, CSS parser, cascade,
/// image decoder) doesn't kill the worker thread. On failure —
/// whether from an HTTP error, an empty layout tree, or a caught
/// panic — we build an error `PageState` from the built-in template
/// and return it as a successful `Loaded` result, preserving the
/// original `source` so the history stack behaves the same way as it
/// would for a real page.
pub(crate) fn run_load_worker(inbox: &Inbox<LoadRequest>, result_tx: &Sender<LoadResult>) {
    while let Some(LoadRequest { url, source }) = inbox.recv() {
        let attempt = catch_panic(|| load_document(&url));

        let state = match attempt {
            Ok(Ok(doc)) => {
                // Dropped CSS, parse warnings, and script errors, as
                // koala-cli prints them, so a partly-rendered site
                // isn't an opaque mystery to whoever launched the
                // app from a terminal.
                report_problems(&url, &doc);
                PageState::from_document(doc).unwrap_or_else(|| {
                    error_state(&url, &Failure::Engine("document produced no layout tree"))
                })
            }
            Ok(Err(e)) => error_state(&url, &Failure::Load(&e)),
            Err(message) => {
                eprintln!("[koala-ui] loader panicked for {url}: {message}");
                error_state(&url, &Failure::Engine(&message))
            }
        };

        if result_tx
            .send(LoadResult { url, source, state })
            .is_err()
        {
            break;
        }
    }
}

/// Builds a `PageState` from the built-in error template for
/// the given URL and failure. Falls back to a minimal
/// hard-coded page when the template itself fails to parse — if
/// that ever happens it means koala-css is broken enough that the
/// error page can't even be used, which we handle by showing a
/// plain-text message.
fn error_state(url: &str, failure: &Failure<'_>) -> Box<PageState> {
    let html = crate::error_page::render(url, failure);
    let doc = parse_html_string(&html);
    report_problems("the error page", &doc);
    if let Some(state) = PageState::from_document(doc) {
        return state;
    }
    let fallback = parse_html_string(
        "<!doctype html><html><body><h1>Page couldn't load</h1></body></html>",
    );
    PageState::from_document(fallback).expect("trivial fallback HTML must produce a layout tree")
}
