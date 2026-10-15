// The render worker: owns the page shown in a tab and turns messages
// from the GUI thread into frames. `browser_page.rs` describes how it
// fits with the loader worker and the GUI thread.

use std::sync::OnceLock;
use std::sync::mpsc::Sender;

use koala_browser::css::selector::ElementState;
use koala_browser::css::{Cursor, DisplayListBuilder, LayoutBox, Rect, canvas_background};
use koala_browser::dom::NodeId;
use koala_browser::{FontProvider, Renderer, RendererFonts, input, restyle};

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

/// What the render worker tells the GUI thread.
pub(crate) enum WorkerReply {
    Frame(RenderResult),
    /// The cursor to show over the viewport from now on. Sent only when
    /// it changes.
    Cursor(Cursor),
    /// A click followed a link: load this URL in the tab.
    Navigate(String),
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
    /// The primary button went down at this point, in physical pixels.
    PointerPressed { x: f32, y: f32 },
    /// The primary button came up at this point, in physical pixels.
    PointerReleased { x: f32, y: f32 },
}

/// Render-worker entry point. On each wake-up it applies every message
/// waiting in the queue, then renders at most one frame. A burst of
/// resizes or pointer moves therefore collapses to its latest state:
/// the one-move-per-frame coalescing every engine does, here per
/// wake-up.
///
/// A panic while restyling or rendering drops the page and reports the
/// panic, and the GUI thread replaces the page with the error page.
pub(crate) fn run_render_worker(inbox: &Inbox<WorkerMessage>, replies: &Sender<WorkerReply>) {
    let mut worker = RenderWorker::default();
    while let Some(batch) = inbox.recv_batch() {
        let mut changes = Changes::default();
        for message in batch {
            worker.apply(message, &mut changes);
        }

        let mut out = Vec::new();
        if let Err(message) = catch_panic(|| worker.update(changes, &mut out)) {
            eprintln!("[koala-ui] render worker panicked: {message}");
            let generation = worker.page.take().map_or(0, |(_, generation)| generation);
            worker.laid_out = None;
            out = vec![WorkerReply::Frame(RenderResult {
                generation,
                width: 0,
                height: 0,
                pixels: Vec::new(),
                error: message,
            })];
        }

        if out.into_iter().any(|reply| replies.send(reply).is_err()) {
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
    /// The cursor last sent to the GUI thread.
    cursor: Cursor,
    /// The node the primary button went down over, until it comes up.
    pressed: Option<NodeId>,
}

/// What the messages of one wake-up changed.
#[derive(Default)]
struct Changes {
    page: bool,
    size: bool,
    pointer: bool,
    /// Primary-button presses (`true`) and releases (`false`), in order,
    /// with where they happened.
    buttons: Vec<(bool, f32, f32)>,
}

impl RenderWorker {
    fn apply(&mut self, message: WorkerMessage, changes: &mut Changes) {
        match message {
            WorkerMessage::Replace { state, generation } => {
                self.page = Some((state, generation));
                // A press on the old page names a node of the old DOM.
                self.pressed = None;
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
            WorkerMessage::PointerPressed { x, y } => changes.buttons.push((true, x, y)),
            WorkerMessage::PointerReleased { x, y } => changes.buttons.push((false, x, y)),
        }
    }

    /// Bring the page up to date with `changes`, pushing a frame onto
    /// `out` if anything visible changed and the cursor if it changed.
    fn update(&mut self, changes: Changes, out: &mut Vec<WorkerReply>) {
        let Some((page, generation)) = self.page.as_mut() else { return };
        let Some((width, height)) = self.size else { return };

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
        if changes.pointer || repaint {
            let laid_out = self.laid_out.as_ref().expect("laid out in STEP 1");
            let hit = self.pointer.and_then(|(x, y)| laid_out.hit_test(x, y));
            let hovered = self
                .pointer
                .and_then(|_| input::hovered_node(&page.dom, hit));

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

            // STEP 4: The cursor, from the styles STEP 3 may have changed,
            // so that `a:hover { cursor: help }` applies on the move that
            // hovers the link.
            if self.pointer.is_some() {
                let cursor = input::cursor_at(&page.dom, &page.styles, hit);
                if cursor != self.cursor {
                    self.cursor = cursor;
                    out.push(WorkerReply::Cursor(cursor));
                }
            }
        }

        // STEP 5: Presses and releases, in the order they happened. A
        // release after a press is a click, which may follow a link.
        let laid_out = self.laid_out.as_ref().expect("laid out in STEP 1 or 3");
        for (pressed, x, y) in changes.buttons {
            let node = input::hovered_node(&page.dom, laid_out.hit_test(x, y));
            if pressed {
                self.pressed = node;
            } else if let (Some(down), Some(up)) = (self.pressed.take(), node)
                && let Some(url) = input::click(&page.dom, down, up, page.base_url.as_deref())
            {
                out.push(WorkerReply::Navigate(url));
            }
        }

        // STEP 6: Paint.
        if !repaint {
            return;
        }
        let laid_out = self.laid_out.as_ref().expect("laid out in STEP 1 or 3");
        out.push(WorkerReply::Frame(RenderResult {
            generation: *generation,
            width,
            height,
            pixels: paint(page, laid_out, width, height),
            error: String::new(),
        }));
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
    use super::{Changes, RenderWorker, WorkerMessage, WorkerReply};
    use crate::page_state::PageState;
    use koala_browser::css::Cursor;
    use koala_browser::parse_html_string;

    /// What one wake-up of the worker sent back.
    struct Outcome {
        repainted: bool,
        cursor: Option<Cursor>,
        navigate: Option<String>,
    }

    fn run(worker: &mut RenderWorker, changes: Changes) -> Outcome {
        let mut out = Vec::new();
        worker.update(changes, &mut out);
        let mut outcome = Outcome {
            repainted: false,
            cursor: None,
            navigate: None,
        };
        for reply in out {
            match reply {
                WorkerReply::Frame(_) => outcome.repainted = true,
                WorkerReply::Cursor(cursor) => outcome.cursor = Some(cursor),
                WorkerReply::Navigate(url) => outcome.navigate = Some(url),
            }
        }
        outcome
    }

    /// Press at one point and release at another, as one wake-up.
    fn click(worker: &mut RenderWorker, down: (f32, f32), up: (f32, f32)) -> Option<String> {
        let mut changes = Changes::default();
        worker.apply(WorkerMessage::PointerPressed { x: down.0, y: down.1 }, &mut changes);
        worker.apply(WorkerMessage::PointerReleased { x: up.0, y: up.1 }, &mut changes);
        run(worker, changes).navigate
    }

    /// A 300×600 viewport showing `body` under `css`.
    fn worker_with(css: &str, body: &str) -> RenderWorker {
        let html = format!(
            "<html><head><style>body {{ margin: 0 }} {css}</style></head>\
             <body>{body}</body></html>"
        );
        let mut worker = RenderWorker::default();
        let mut changes = Changes::default();
        let state = PageState::from_document(parse_html_string(&html)).expect("a layout tree");
        worker.apply(WorkerMessage::Replace { state, generation: 1 }, &mut changes);
        worker.apply(WorkerMessage::Resize { width: 300, height: 600 }, &mut changes);
        assert!(run(&mut worker, changes).repainted, "the first frame");
        worker
    }

    /// A 200px-tall link filling the top of the page, so (10, 10) is over
    /// it and (10, 400) is over the root element only.
    fn worker_showing(css: &str) -> RenderWorker {
        worker_with(
            &format!("a {{ display: block; height: 200px }} {css}"),
            "<a href='/'>link</a>",
        )
    }

    fn move_pointer(worker: &mut RenderWorker, x: f32, y: f32) -> Outcome {
        let mut changes = Changes::default();
        worker.apply(WorkerMessage::PointerMoved { x, y }, &mut changes);
        run(worker, changes)
    }

    fn hovered_tag(worker: &RenderWorker) -> Option<String> {
        let (page, _) = worker.page.as_ref()?;
        let node = page.dom.hovered()?;
        Some(page.dom.as_element(node)?.tag_name.to_string())
    }

    #[test]
    fn hover_change_repaints_when_a_rule_reads_hover() {
        let mut worker = worker_showing("a:hover { color: red }");
        assert!(move_pointer(&mut worker, 10.0, 10.0).repainted);
        assert_eq!(hovered_tag(&worker).as_deref(), Some("a"));

        // Moving within the same element changes nothing.
        assert!(!move_pointer(&mut worker, 20.0, 20.0).repainted);
    }

    #[test]
    fn hover_change_without_hover_rules_does_not_repaint() {
        let mut worker = worker_showing("a { color: blue }");
        assert!(!move_pointer(&mut worker, 10.0, 10.0).repainted);
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
        assert!(run(&mut worker, changes).repainted);
        assert_eq!(hovered_tag(&worker), None);
    }

    /// The UA stylesheet's `:link { cursor: pointer }`, then the arrow
    /// below the content. A move that keeps the cursor sends nothing.
    #[test]
    fn cursor_follows_the_element_under_the_pointer() {
        let mut worker = worker_showing("");
        assert_eq!(move_pointer(&mut worker, 10.0, 10.0).cursor, Some(Cursor::Pointer));
        assert_eq!(move_pointer(&mut worker, 20.0, 20.0).cursor, None);
        assert_eq!(move_pointer(&mut worker, 10.0, 400.0).cursor, Some(Cursor::Default));
    }

    /// [CSS UI 4 § 5.1.1.1](https://drafts.csswg.org/css-ui-4/#cursor):
    /// "auto behaves as text over selectable text ... and default
    /// otherwise." The paragraph's text starts at (0, 0); at y = 100 the
    /// pointer is inside the 200px-tall paragraph but below its one line.
    #[test]
    fn auto_cursor_is_text_over_text() {
        let mut worker = worker_with("p { margin: 0; height: 200px }", "<p>plain words</p>");
        assert_eq!(move_pointer(&mut worker, 5.0, 5.0).cursor, Some(Cursor::Text));
        assert_eq!(move_pointer(&mut worker, 5.0, 100.0).cursor, Some(Cursor::Default));
    }

    /// The cursor is read after the restyle a hover change causes, so a
    /// `:hover` rule's cursor applies on the move that hovers.
    #[test]
    fn hover_rule_cursor_applies_on_the_same_move() {
        let mut worker = worker_showing("a:hover { cursor: help }");
        assert_eq!(move_pointer(&mut worker, 10.0, 10.0).cursor, Some(Cursor::Help));
    }

    /// A page parsed from a string has no URL, so an absolute link is the
    /// only kind that resolves to somewhere loadable.
    #[test]
    fn click_on_a_link_navigates() {
        let mut worker = worker_with(
            "a { display: block; height: 200px }",
            "<a href='https://example.com/next'>next</a>",
        );
        assert_eq!(click(&mut worker, (10.0, 10.0), (12.0, 12.0)).as_deref(), Some("https://example.com/next"));
    }

    /// Pressed on the link, released below it: the click lands on their
    /// nearest common ancestor, which is not a link.
    #[test]
    fn press_on_link_release_elsewhere_does_not_navigate() {
        let mut worker = worker_showing("");
        assert_eq!(click(&mut worker, (10.0, 10.0), (10.0, 400.0)), None);
    }
}
