# Active Session State

**Updated:** 2026-04-12

## Current goal

Building a Qt-based browser UI (`koala-qt`) on top of the existing
koala HTML renderer engine. End-to-end path works: HTML → layout →
paint → pixels → QImage in a real QWidget tab viewport.

## Active task queue

- #7 Move `render_to_rgba` to worker thread — pending
- #8 Wire real URL navigation — pending, depends on #7
- #9 Per-tab back/forward history stack — pending, depends on #8
- #10 Sync tab title from document `<title>` — pending, depends on #8
- #11 Architecture review — **under review, see T12 below**

## Last Alice review

**T12 review:** `project-memory/reviews/T12-2026-04-12-architecture-review.md`

- Verdict: **FAIL** on the proposed full crate split as written
- Tasks #7–#10 do not depend on the refactor and can proceed
- **Phase 0 prerequisite work** (if any refactor happens):
  1. Move `FontStyle`, `TextDecorationLine`, `TextAlign`,
     `ClearSide`, `FloatSide`, `PositionType` out of
     `koala-css/src/layout/` into `style/values/` — fixes the
     `style::computed` → `layout::` circular dep
  2. Rename `koala-css::paint::Painter` → `DisplayListBuilder` —
     frees the name for any future `koala-gfx::Painter`
  3. Fix stale `CLAUDE.md` architecture tree and `MEMORY.md` paths
- **Phase 1** (after Phase 0): extract `koala-browser::renderer.rs`
  + `font_metrics.rs` into `koala-gfx`. Do NOT extract layout or
  paint. Accept that `koala-gfx` depends on `koala-css`.
- **Phase 2** (deferred): layout extraction, `layout_box.rs`
  decomposition, `ComputedStyle` split, Paintable tree, typed
  pipeline for `parse_html_string`.

## Validated assumptions

- Tasks #7–#10 can proceed without the crate refactor (Rabbit, Queen)
- `koala-css::paint` has a distinct paint stage today — the pipeline
  is NOT fused (Queen, Tweedle, confirmed via `painter.rs:67` +
  `renderer.rs:240`)
- `parse_html_string` is already factored into independent helpers
  (Tweedle, confirmed via `lib.rs:156-212`)
- `RendererFonts` + `OnceLock` cache from task #6 already partially
  addresses the "font loading mixed with rasterization" concern

## Unvalidated assumptions

- Task #7 (worker thread) will surface a concrete Send-audit
  requirement on `LoadedDocument`'s `JsRuntime` field. Not yet
  tested.
- The author's "LibGfx-style web-agnostic graphics layer" framing
  is aspirational; current Renderer imports 5 CSS-domain types and
  cannot be web-agnostic without substantial redesign.

## Invalidated assumptions

- ~~"Pipeline is fused; koala has no paint stage"~~ — false,
  `DisplayList` already exists as the intermediate
- ~~"`parse_html_string` is monolithic and can't be decomposed"~~ —
  false, helpers are already independently callable
- ~~"Font loading is mixed with rasterization"~~ — partially fixed
  by task #6's `RendererFonts` refactor

## Decisions made

- **Defer the full crate split.** Small incremental Phase 0 + Phase 1
  only, after tasks #7–#10 ship. Rationale in T12.
- **Do not create `koala-paint` as a separate crate.** 586 lines,
  tightly coupled to layout types, one consumer. Keep as module.
- **Drop "Paintable tree" from the crate-split proposal.** Real
  semantic refactor, separate work item if wanted.
- **`koala-gfx` is the least-bad name** for the eventual extracted
  graphics crate (per Dumpty). Mild drift from the spelled-out
  hyphenated convention, but no better alternative survives the
  collision audit.
