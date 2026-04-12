---
created: 2026-04-12
area: koala-browser::renderer / koala-qt::browser_page
status: open — deferred optimization backlog
---

# Rasterizer future work

This is the backlog of rasterizer improvements surfaced during
tasks #6 (new-tab lag investigation) and #7 (worker-thread move).
None of it is required to ship the Qt browser UI, but each item
here is a known path to substantially faster rendering that would
unlock better interactive behavior (live resize, smoother
scrolling, animation).

## Measured baseline (macOS, Helvetica, landing page)

Numbers from task #6's timing instrumentation on a 2560×1488
physical viewport:

| Stage | Cost | Notes |
|---|---|---|
| `parse_html_string` (landing page) | 3 ms | HTML → DOM → cascade → layout tree → JS |
| `LayoutBox::clone` | 30 µs | Per-render clone before mutation |
| `LayoutBox::layout` | 250 µs | Second pass, runs every render |
| `DisplayListBuilder::build` | 17 µs | Layout tree → display list |
| `Renderer::new_with_fonts` | **35 ms** | Allocates + fills 15 MB RGBA buffer |
| `set_canvas_background` | folded | In-place fill (task #6) |
| `renderer.render(&display_list)` | **180 ms** | Executes the display list — glyph rasterization dominates |
| `rgba_bytes().to_vec()` | 0.7 ms | Copy out of `ImageBuffer` into `Vec<u8>` |
| **Total per full render** | **~220 ms** | |

Per-tab opening cost after task #6 optimizations:
~230 ms total (down from 735 ms — 3.2× improvement from font
caching alone). The remaining cost is almost entirely in the
two marked items above.

## Improvement backlog, ordered by value / effort ratio

### Tier 1 — high ROI, tractable

**1. Buffer pool for `ImageBuffer`.**
Every render calls `Renderer::new_with_fonts` which allocates a
fresh `vec![Rgba; w*h]` — that's 15 MB of page-fault-bound
allocation per 2560×1488 frame. Reusing the buffer across renders
saves ~30 ms per frame. Design: keep one persistent `Renderer`
per `BrowserPage` and add a `resize(w, h)` method that reallocates
only when dimensions change.
Lands close to: `crates/koala-browser/src/renderer.rs` + the
`render_state` helper in `koala-qt/src/browser_page.rs`.
Estimated saving: **~30 ms per full-size render**.

**2. Glyph rasterization cache.**
The 180 ms `render()` cost is dominated by `fontdue` rasterizing
each glyph from scratch every frame. A `HashMap<(GlyphKey), Bitmap>`
keyed on `(font_id, glyph_id, size, subpixel_quantized_offset)`
would make repeat renders of the same text essentially free.
Should live inside `Renderer` (or the future `koala-gfx`) so it's
shared across renders of the same document. Cache eviction: LRU
with a size cap, or keyed-off-font-size buckets.
Lands close to: `crates/koala-browser/src/renderer.rs::draw_text`.
Estimated saving: **50–150 ms per full-size render** on pages
with ≥20 glyphs (which is every real page).

**3. Parallelize rasterization across scanline bands.**
Split the display list into horizontal bands, rasterize each band
on a worker thread. `rayon` or a handful of `std::thread`s.
Glyph rasterization and path filling are both embarrassingly
parallel across non-overlapping pixel regions. The hard part is
dealing with display commands that cross band boundaries (a
border crossing a scanline divider) — either clip per band, or
accept a small overdraw at boundaries.
Estimated saving: **N× on N cores**, minus ~20% synchronization
overhead.

**4. Skip re-layout when only viewport size changed but content
didn't.**
Currently `render_state` clones `layout_tree` and calls
`layout.layout(viewport, ...)` on every render. For pages whose
content is unchanged, the second pass recomputes identical results
at different viewport widths. A cache keyed on `(content_hash,
viewport_w, viewport_h)` (or better, `viewport_w` only since
height is derived) would let us reuse the laid-out tree.
Estimated saving: **~250 µs per render** — small in absolute
terms but eliminates an O(n) walk every frame.

### Tier 2 — bigger investment, high ceiling

**5. Dirty-region tracking and partial repaint.**
Most viewport updates change only a small region (a hover state,
a scrolled line, a caret blink). Emit a dirty-rect list from the
display list builder and only rasterize the intersection. Requires
a notion of what "changed" relative to the previous frame, which
means keeping the previous frame's display list around for
diffing.
Estimated saving: **≥90% on animation / interaction paths** where
the change is localized.

**6. Resolution scaling during interactive updates.**
Render at half DPR during active drag / scroll / animation,
restore full DPR on pause. 2× faster rasterization with visible
quality degradation; combined with the smooth-transform scale in
the paint, mostly imperceptible during motion.
Lands close to: `BrowserView::request_render`, needs an
"interactive" flag.
Estimated saving: **~75% during interactive gestures** (4× fewer
pixels).

**7. Event-driven result delivery, replacing the 16 ms QTimer poll.**
`koala-qt::BrowserView::poll_render_result` currently polls at
~60 Hz via a QTimer. A Rust-side callback (`fn(&BrowserView)` held
across the cxx bridge) invoked on the worker thread, that calls
`QMetaObject::invokeMethod(view, "on_frame_ready", Qt::QueuedConnection)`,
would deliver frames with ~0 ms latency after the worker finishes,
and remove the always-on timer. Needs cxx function-pointer support
or a `rust::Fn` holder.
Saving: **8 ms average latency per frame** + one fewer running
timer.

### Tier 3 — big projects with their own scoping

**8. GPU backend via `wgpu` or Metal/Vulkan directly.**
Replace the software `Renderer` with a GPU-backed one. The display
list abstraction already exists, so the scope is "write a new
backend that executes `DisplayCommand`s via draw calls." Glyph
atlases, uniform buffers, the works. 10–100× speedup on anything
that's not text-heavy; still needs CPU rasterization for glyphs
unless we also do signed-distance-field fonts. Big project.
Probably only worth doing when the project decides to commit to
real-browser-class rendering.

**9. Incremental re-rasterization keyed on layout invalidation
flags.**
When the engine gains proper style / layout invalidation (needed
for DOM mutations and interactive editing), plumb that same
invalidation forward to the rasterizer so it only re-runs on
actually-dirty subtrees. Combines with #5.

**10. Rust-side SIMD fast paths.**
Alpha blending, gradient filling, and box-shadow blur are all
classic SIMD targets. `std::simd` (or `wide` on stable) could
2–4× the inner loops without touching the rasterizer's shape.
Low impact on the current bottleneck (glyph rasterization) since
that's already handled inside `fontdue`, but becomes relevant
after #2 and #5 land.

## What's already done

- **Task #6:** font loading cached globally via `RendererFonts` +
  `OnceLock` (commit `076d0fc` / `498324b`). Saved ~500 ms per
  tab, bringing new-tab open from 735 ms to ~230 ms.
- **Task #6:** `set_canvas_background` switched from reallocation
  to in-place fill. Saves ~10 ms per render.
- **Task #7:** rasterization moved to a dedicated Rust worker
  thread with coalescing (`std::sync::mpsc`). GUI thread stays
  responsive during renders; rapid resizes coalesce to a single
  render at the latest size.

## Out of scope (not rasterizer work)

- Parsing / tokenizing: already fast (~3 ms for the landing page).
- Layout: already fast (~250 µs) despite the 3,740-line
  `layout_box.rs`. Decomposing that file would help
  maintainability, not performance.
- Image loading: happens once at parse time; not in the hot path.
- JS execution: runs at parse time and the runtime is discarded.
  If we add DOM↔JS bindings later, interactive paths will need
  careful scheduling, but that's a different axis.
