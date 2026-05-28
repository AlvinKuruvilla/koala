---
created: 2026-05-27
area: koala-browser / koala-js / koala-ui — render-pipeline architecture
status: open — architectural-direction backlog
---

# Render-pipeline architectural levers

These are the three independent axes along which the render pipeline can be
made faster or more responsive. They surfaced during the live-bench
investigation that traced 30 s of user-felt "URL load is slow" on
google.com almost entirely to `js_pump_until_idle` — the post-`load`
pump waiting for `setTimeout` watchdog timers to fire.

The takeaway from that investigation: capping the pump duration would
hide the symptom in ~10 lines, but the underlying problem is that JS
execution is on the critical path of rendering at all. Real browsers
don't work this way. The levers below describe what "doing it right"
looks like, in increasing order of scope.

## Lever 1 — Decouple the load contract

**Problem:** `koala_browser::load_document` returns only after
`pump_until_idle_or` settles, i.e. after every `setTimeout` watchdog
has fired. That blocks the render pipeline waiting on analytics /
heartbeat code that has no user-visible effect on initial paint.

**Fix:** Change the contract so `load_document` returns as soon as the
page is *paintable* — parse + cascade + initial layout + DOMContentLoaded
done, inline scripts executed. The pump becomes the caller's job, called
incrementally between renders. DOM mutations from background pumping
signal the caller to re-render.

**Where the work lives:**
- `crates/koala-js/src/scheduler.rs` — add `pump_one_step()` /
  `pump_for(Duration)` alongside the existing `pump_until_idle_or`.
- `crates/koala-browser/src/lib.rs` — split `load_document` /
  `parse_html_with_base_url` so the all-or-nothing pump call is removed
  from the bottom and the function returns earlier with a "still pumping"
  marker if appropriate.
- `koala-ui/src/browser_page.rs` — the loader worker thread becomes the
  pump driver. After delivering the initial `PageState` to the GUI, it
  pumps JS in small chunks, signalling a re-render when
  `JsRuntime::take_dom_dirty()` returns `true`.

**Scope:** ~1–2 days. No threading change, no `Send`-ness rewrites, no
DOM ownership shuffle. The hardest part is deciding the cooperative-pump
API on `JsRuntime`.

**Expected impact:** On real pages, removes the bulk of `js_execute`
(98 % of it per the google.com bench) from setup. User-felt URL load
on google.com would drop from ~30 s to ~500 ms — the actual Boa CPU
cost of running site scripts.

**Bench signal to confirm:** `just bench-live https://google.com`
`setup_us` drops from ~31 s to <1 s, with `js_pump_until_idle` no
longer counted (it now runs in the background between renders).

## Lever 2 — GPU rendering

**Problem:** Software rasterization. Per-render is currently 4.7 ms on
the landing page (89 % in `rasterize`); a GPU backend would make this
~10–100× faster, unlocking animation / smooth-resize / scroll perf.

**Fix:** Replace the software `Renderer` in `crates/koala-browser/src/renderer.rs`
with a GPU-backed one. The display list abstraction already exists, so
the scope is "write a new backend that executes `DisplayCommand`s via
draw calls." Glyph atlases, uniform buffers, the works. Still needs CPU
rasterization for glyphs unless we also implement signed-distance-field
fonts.

**Where the work lives:** Big project of its own. Likely a new
`crates/koala-gfx` crate. `wgpu` is the obvious choice for the backend
(cross-platform Vulkan / Metal / DX12).

**Scope:** Multi-week. See also `project-memory/rasterizer-future-work.md`
Tier 3 item #8 for the existing notes.

**Expected impact:** Orthogonal to Lever 1 — helps raw throughput, does
nothing for the JS-blocks-render problem. Both can land independently;
the wins compound.

**Bench signal to confirm:** `just bench` `rasterize` mean drops from
~4 ms to ~50–500 µs on the landing page.

## Lever 3 — JS on a dedicated thread

**Problem:** Even with Lever 1 (cooperative pumping), JS execution still
contends with the main thread for CPU. A long-running synchronous script
still blocks renders for the duration of one pump slice.

**Fix:** Run Boa's `Context` on a dedicated thread. DOM access happens
through message passing or a shared `Arc<Mutex<DomTree>>`. Render thread
reads DOM snapshots without coordinating with JS. This is what Chrome
and Safari do (renderer process vs JS engine thread).

**Where the work lives:**
- `crates/koala-js/src/dom_handle.rs` — DOM ownership currently
  `Rc<RefCell<DomTree>>` (per the koala-js memory). Would need to become
  `Arc<RwLock<DomTree>>` or message-passing.
- Boa's `Context` is not `Send` (per the comment in
  `koala-ui/src/browser_page.rs`). Either work around with a
  thread-local-bound runtime channel, or accept the engine restriction.
- `crates/koala-browser/src/lib.rs` — `parse_html_with_base_url`'s JS
  block becomes "kick off the runtime thread; signal when the initial
  scripts settle".

**Scope:** Multi-week. The DOM ownership refactor is the dominant cost.

**Expected impact:** Removes JS / render contention entirely. Long
synchronous scripts no longer affect frame rate. Real-browser parity.

**Bench signal to confirm:** `just bench-live https://google.com` shows
the same `setup_us` as before, but a separate "interactivity" metric
(time-to-first-paint, time-to-first-input) drops to constant regardless
of JS load.

## Recommended order

1. **Lever 1 first.** Smallest scope, biggest immediate user-felt win,
   no architectural commitments we'd regret. Prerequisite for Levers 2
   and 3 to compound meaningfully.
2. **Lever 2 next.** Independent axis, big perf headroom for
   animation / interaction.
3. **Lever 3 last (if at all).** Real-browser parity, but Lever 1 +
   cooperative scheduling on a single thread is sufficient for most
   real-browser UX. Only worth doing if we hit "single-threaded
   cooperative pump can't keep up" — and we probably won't.
