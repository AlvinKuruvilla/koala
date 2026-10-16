// The render worker: owns the page shown in a tab and turns messages
// from the GUI thread into frames. `browser_page.rs` describes how it
// fits with the loader worker and the GUI thread.

use std::sync::OnceLock;
use std::sync::mpsc::Sender;

use koala_browser::css::selector::ElementState;
use koala_browser::css::{DisplayListBuilder, LayoutBox, Rect, canvas_background};
use koala_browser::{FontProvider, Renderer, RendererFonts, restyle};

use crate::page_state::PageState;
use crate::worker::{Inbox, catch_panic};

// Process-wide `RendererFonts` cache. Loading four font files from
// disk costs ~250 ms on macOS; doing it per render was the dominant
// cost before the rasterizer rework. Now loaded exactly once, shared
// across every `BrowserPage` and every render via `Arc` clones.
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

/// One finished frame produced by the render worker. Crate-private
/// — the public surface exposes `try_take_render_image` which wraps
/// these bytes into a Slint `Image`.
///
/// `error` is populated when the render worker caught a panic while
/// trying to rasterise the page. The Rust side handles this case
/// internally by swapping the current page for the built-in error
/// template and scheduling a fresh render; the GUI caller never sees
/// the error payload and only knows that no paintable frame arrived.
pub(crate) struct RenderResult {
    /// The generation of the page state this frame shows.
    pub(crate) generation: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
    pub(crate) error: String,
}

/// What the GUI thread tells the render worker. The worker handles every
/// message waiting in its queue before it renders, so a burst of pointer
/// moves or resizes costs one frame.
pub(crate) enum WorkerMessage {
    /// Show this page from now on.
    Replace {
        state: Box<PageState>,
        /// The generation of `state` (see `BrowserPage::state_generation`),
        /// carried through to its frames.
        generation: u64,
    },
    /// The viewport's size in physical pixels.
    Resize { width: u32, height: u32 },
    /// The pointer is over the viewport at this point, in physical pixels
    /// from its top-left corner.
    PointerMoved { x: f32, y: f32 },
    /// The pointer left the viewport.
    PointerLeft,
}

/// Render-worker entry point. On each wake-up it applies every message
/// waiting in the queue, then renders at most one frame. A burst of
/// resizes or pointer moves therefore collapses to its latest state:
/// the one-move-per-frame coalescing every engine does, here per
/// wake-up.
///
/// A panic while restyling or rendering drops the page and reports the
/// panic, and the GUI thread replaces the page with the error page.
pub(crate) fn run_render_worker(inbox: &Inbox<WorkerMessage>, result_tx: &Sender<RenderResult>) {
    let mut worker = RenderWorker::default();
    while let Some(batch) = inbox.recv_batch() {
        let mut changes = Changes::default();
        for message in batch {
            worker.apply(message, &mut changes);
        }

        let result = match catch_panic(|| worker.update(changes)) {
            Ok(Some(result)) => result,
            Ok(None) => continue,
            Err(message) => {
                eprintln!("[koala-ui] render worker panicked: {message}");
                let generation = worker.page.take().map_or(0, |(_, generation)| generation);
                worker.laid_out = None;
                RenderResult {
                    generation,
                    width: 0,
                    height: 0,
                    pixels: Vec::new(),
                    error: message,
                }
            }
        };

        if result_tx.send(result).is_err() {
            break;
        }
    }
}

/// What the render worker holds between wake-ups.
#[derive(Default)]
struct RenderWorker {
    /// The page shown, with its generation.
    page: Option<(Box<PageState>, u64)>,
    /// The viewport, in physical pixels.
    size: Option<(u32, u32)>,
    /// Where the pointer is over the viewport, if it is.
    pointer: Option<(f32, f32)>,
    /// `page`'s layout tree laid out at `size`, from the last frame. Hit
    /// testing reads it.
    laid_out: Option<LayoutBox>,
}

/// What the messages of one wake-up changed.
#[derive(Default, Clone, Copy)]
struct Changes {
    page: bool,
    size: bool,
    pointer: bool,
}

impl RenderWorker {
    fn apply(&mut self, message: WorkerMessage, changes: &mut Changes) {
        match message {
            WorkerMessage::Replace { state, generation } => {
                self.page = Some((state, generation));
                changes.page = true;
            }
            WorkerMessage::Resize { width, height } => {
                // The GUI asks again at the same size after every page
                // swap; that must not lay the page out a second time.
                if self.size != Some((width, height)) {
                    self.size = Some((width, height));
                    changes.size = true;
                }
            }
            WorkerMessage::PointerMoved { x, y } => {
                self.pointer = Some((x, y));
                changes.pointer = true;
            }
            WorkerMessage::PointerLeft => {
                self.pointer = None;
                changes.pointer = true;
            }
        }
    }

    /// Bring the page up to date with `changes` and return a frame if
    /// anything visible changed.
    fn update(&mut self, changes: Changes) -> Option<RenderResult> {
        let (page, generation) = self.page.as_mut()?;
        let (width, height) = self.size?;

        // STEP 1: Lay the page out again if it or the viewport changed.
        let mut repaint = false;
        if changes.page || changes.size || self.laid_out.is_none() {
            self.laid_out = Some(lay_out(&page.layout_tree, width, height));
            repaint = true;
        }

        // STEP 2: Find the hovered node. After a new layout this runs even
        // if the pointer stood still, since the element under it may have
        // changed: Chromium and Gecko send themselves a synthetic mouse
        // move after layout for the same reason.
        //
        // [CSSOM View § 5 elementFromPoint()](https://drafts.csswg.org/cssom-view/#dom-document-elementfrompoint)
        //
        // "If there is a box in the viewport that would be a target for
        // hit testing at coordinates x,y, ... return the associated element
        // and terminate these steps."
        //
        // "If the document has a root element, return the root element and
        // terminate these steps."
        if changes.pointer || repaint {
            let laid_out = self.laid_out.as_ref().expect("laid out in STEP 1");
            let hovered = self.pointer.and_then(|(x, y)| {
                laid_out
                    .hit_test(x, y)
                    .map(|hit| hit.node)
                    .or_else(|| page.dom.document_element())
            });

            // STEP 3: Restyle if the hovered node changed and some selector
            // reads :hover. Without such a selector no computed style can
            // change, so neither can the frame.
            //
            // The new layout can put another element under the pointer.
            // That waits for the next pointer move: hit testing again here
            // would loop forever under a rule like
            // `a:hover { margin-left: 100px }`, which moves its element
            // out from under the pointer. Gecko lets only scrolling
            // trigger another synthetic move, for the same reason.
            if hovered != page.dom.hovered() {
                page.dom.set_hovered(hovered);
                if page.state_dependencies.contains(ElementState::HOVER) {
                    let restyled = restyle(&page.dom, &page.stylesheet, &page.image_dims);
                    page.styles = restyled.styles;
                    page.inline_svgs = restyled.inline_svgs;
                    if let Some(layout_tree) = restyled.layout_tree {
                        page.layout_tree = layout_tree;
                    }
                    self.laid_out = Some(lay_out(&page.layout_tree, width, height));
                    repaint = true;
                }
            }
        }

        // STEP 4: Paint.
        if !repaint {
            return None;
        }
        let laid_out = self.laid_out.as_ref().expect("laid out in STEP 1 or 3");
        Some(RenderResult {
            generation: *generation,
            width,
            height,
            pixels: paint(page, laid_out, width, height),
            error: String::new(),
        })
    }
}

/// Lay out a copy of `layout_tree` in a viewport of this size. The copy
/// is laid out, not the tree itself, because layout writes used values
/// into the boxes and the next layout must start from computed values.
fn lay_out(layout_tree: &LayoutBox, width: u32, height: u32) -> LayoutBox {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
    };
    let mut laid_out = layout_tree.clone();
    let font_metrics = cached_font_provider().metrics();
    laid_out.layout(viewport, viewport, &*font_metrics, viewport);
    laid_out
}

/// Paint and rasterize a laid-out page.
fn paint(state: &PageState, laid_out: &LayoutBox, width: u32, height: u32) -> Vec<u8> {
    let builder = DisplayListBuilder::new(&state.styles);
    let display_list = builder.build(laid_out);

    let mut renderer = Renderer::new_with_fonts(
        width,
        height,
        state.images.clone(),
        cached_fonts().clone(),
    )
    .with_inline_svgs(state.inline_svgs.clone());

    // Propagate the canvas background (CSS 2.1 § 14.2) so regions
    // of the viewport not covered by painted content still show the
    // html/body background colour instead of Renderer's default
    // white fill.
    if let Some(bg) = canvas_background(&state.dom, &state.styles) {
        renderer.set_canvas_background(&bg);
    }

    renderer.render(&display_list);
    renderer.rgba_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::{Changes, RenderWorker, WorkerMessage};
    use crate::page_state::PageState;
    use koala_browser::parse_html_string;

    /// A 200px-tall link filling the top of the page, so (10, 10) is over
    /// it and (10, 400) is over the root element only.
    fn worker_showing(css: &str) -> RenderWorker {
        let html = format!(
            "<html><head><style>body {{ margin: 0 }} \
             a {{ display: block; height: 200px }} {css}</style></head>\
             <body><a href='/'>link</a></body></html>"
        );
        let mut worker = RenderWorker::default();
        let mut changes = Changes::default();
        let state = PageState::from_document(parse_html_string(&html)).expect("a layout tree");
        worker.apply(WorkerMessage::Replace { state, generation: 1 }, &mut changes);
        worker.apply(WorkerMessage::Resize { width: 300, height: 600 }, &mut changes);
        assert!(worker.update(changes).is_some(), "the first frame");
        worker
    }

    fn move_pointer(worker: &mut RenderWorker, x: f32, y: f32) -> bool {
        let mut changes = Changes::default();
        worker.apply(WorkerMessage::PointerMoved { x, y }, &mut changes);
        worker.update(changes).is_some()
    }

    fn hovered_tag(worker: &RenderWorker) -> Option<String> {
        let (page, _) = worker.page.as_ref()?;
        let node = page.dom.hovered()?;
        Some(page.dom.as_element(node)?.tag_name.to_string())
    }

    #[test]
    fn hover_change_repaints_when_a_rule_reads_hover() {
        let mut worker = worker_showing("a:hover { color: red }");
        assert!(move_pointer(&mut worker, 10.0, 10.0));
        assert_eq!(hovered_tag(&worker).as_deref(), Some("a"));

        // Moving within the same element changes nothing.
        assert!(!move_pointer(&mut worker, 20.0, 20.0));
    }

    #[test]
    fn hover_change_without_hover_rules_does_not_repaint() {
        let mut worker = worker_showing("a { color: blue }");
        assert!(!move_pointer(&mut worker, 10.0, 10.0));
        // The hovered node is still tracked for when rules need it.
        assert_eq!(hovered_tag(&worker).as_deref(), Some("a"));
    }

    /// [CSSOM View § 5](https://drafts.csswg.org/cssom-view/#dom-document-elementfrompoint)
    ///
    /// "If the document has a root element, return the root element"
    #[test]
    fn pointer_below_content_hovers_the_root_element() {
        let mut worker = worker_showing("");
        move_pointer(&mut worker, 10.0, 400.0);
        assert_eq!(hovered_tag(&worker).as_deref(), Some("html"));
    }

    #[test]
    fn pointer_leaving_clears_hover() {
        let mut worker = worker_showing("a:hover { color: red }");
        move_pointer(&mut worker, 10.0, 10.0);
        let mut changes = Changes::default();
        worker.apply(WorkerMessage::PointerLeft, &mut changes);
        assert!(worker.update(changes).is_some());
        assert_eq!(hovered_tag(&worker), None);
    }
}
