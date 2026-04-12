---
template: T12
created: 2026-04-12
task: 11
focus: crate architecture refactor (koala-css split, koala-gfx extraction)
phase: refinement
verdict: FAIL
---

# T12 — Architecture Review: Proposed Crate Split

## Review context

The author proposed splitting `koala-css` (which currently houses CSS
parser + cascade + layout engine + paint/DisplayList + style values)
and `koala-browser` (which houses document loading + software
rasterizer) into five crates modeled loosely on Ladybird's LibWeb /
LibGfx / LibWebView separation:

```
koala-gfx    (NEW) — fonts, glyph raster, bitmap, Painter primitives
koala-css    (SLIMMED) — tokenizer, parser, selector, cascade, values
koala-layout (NEW) — formatting contexts, LayoutBox, build_layout_tree
koala-paint  (NEW) — Paintable tree + DisplayList
koala-browser (SLIMMED) — pipeline, network, image loading
```

The author gave eight concerns justifying the split and asked for
(1) validation of the concerns, (2) validation of the proposal, (3)
unidentified problems, (4) Rabbit's opinion on whether this is
premature, and (5) the smallest incremental first step.

Panel consulted: **Rabbit, Queen, Cheshire, Tweedle, Dormouse, Dumpty.**

## Verdict: **FAIL**

Multiple findings across multiple agents. The proposal cannot ship as
written. However, a significantly smaller slice is both tractable and
valuable, and tasks #7–#10 do not depend on any of this, so the
project is not blocked.

## Headline findings

### Critical — must fix before any crate-split work starts

**C1. Circular dep hidden in `style::computed` — cannot split `koala-layout` without moving enums first.**
`crates/koala-css/src/style/computed.rs:10-12` imports
`layout::float::{ClearSide, FloatSide}`,
`layout::inline::{FontStyle, TextAlign, TextDecorationLine}`, and
`layout::positioned::PositionType`. `ComputedStyle` is the type the
cascade produces; those enums are style values miscategorized as
layout. Any split that puts layout in a new crate on top of css
creates an instant cycle: layout → css (for ComputedStyle) → layout
(for these enums). **Fix:** move these enums into `style/values/` *in
place*, under the existing crate, before touching any `Cargo.toml`.
Cheshire classes this as "the most likely-to-occur, most-painful-to-
untangle issue, invisible from the proposed diagram."

**C2. `Painter` name collision.**
`koala-css::paint::Painter`
(`crates/koala-css/src/paint/painter.rs:48`) is the current high-level
layout→DisplayList builder. A Ladybird-style `koala-gfx::Painter`
would occupy the same name for pixel-level ops. Shipping the split
without renaming one of them produces two `Painter` structs in two
crates in the same workspace. **Fix:** rename
`koala-css::paint::Painter` to `DisplayListBuilder` (or `PaintBuilder`)
before the split — one file, one mechanical change, unblocks every
downstream decision. Dumpty flagged this as "the highest-leverage
rename in the whole proposal."

**C3. `koala-gfx` as proposed is not web-agnostic.**
`crates/koala-browser/src/renderer.rs:21-22` imports `BorderRadius`,
`ColorValue`, `DisplayCommand`, `DisplayList`, `FontStyle`, and
`TextDecorationLine` directly from `koala_css`. Execute-command
switches on `DisplayCommand` variants, `fill_rect` takes a
`BorderRadius`, `draw_text` takes a `FontStyle`. The author
characterized this as "800 lines of pixel work, really a graphics
library." It is not. Extracting it as-is produces a crate that
depends on `koala-css` and therefore cannot sit below css in the
dependency graph — directly contradicting the proposed "gfx is
web-agnostic like LibGfx" framing. **Fix options:** (a) accept that
`koala-gfx` depends on `koala-css`, which is fine and non-cyclic but
falsifies the LibGfx analogy; (b) move `DisplayCommand` and its
payload types into `koala-gfx` and redesign them in gfx-native terms
(substantial rewrite). The author must pick before extracting.

**C4. `layout_box.rs` (3,740 lines) is not decomposed by the
refactor.** The stated fix for concern #1 ("koala-css is a god crate")
is "split it into smaller crates." But after the split, the new
`koala-layout` is ~8,000 lines dominated by a 3,740-line
`layout_box.rs`, and `ComputedStyle` is still a 2,863-line god struct
in `style/computed.rs`. Queen: "concern #1 is not really fixed; it's
relabeled." The real decomposition — splitting `layout_box.rs` along
formatting-context lines (block/inline/flex/grid/table dispatch all
live there) and splitting `ComputedStyle` into per-property-group
structs — is out of scope in the current proposal and vastly larger
than the crate move. **Fix:** either include file decomposition in
the plan, or demote concern #1 to "partially addressed, second pass
needed" and stop selling the refactor as solving it.

**C5. "Paintable tree" is a phantom requirement.**
`grep -r Paintable crates/` returns zero hits. Koala has no Paintable
concept today; the pipeline is `LayoutBox → DisplayList → pixels` and
`DisplayList` is already a serializable intermediate (produced by
`painter.rs:67`, consumed by `renderer.rs:240`). Introducing a
Paintable tree as a *side effect* of a crate split conflates two very
different refactors. A real Paintable tree is where z-index resolution
and stacking-context construction live in browser engines — that's a
semantic refactor with its own scoping and justification, not a
free byproduct of moving files. **Fix:** drop "Paintable tree" from
the crate-split proposal. If Paintable is wanted, scope it as its own
work item.

### High — should fix, significant impact

**H1. Concerns 4 and 5 (`parse_html_string` monolithic, `LoadedDocument`
god struct) are not addressed by the crate split.**
`crates/koala-browser/src/lib.rs:156-212`: `parse_html_with_base_url`
is already factored into clean helpers (tokenize → parse → extract
stylesheets → compute styles → load images → build layout → execute
JS). What makes it "monolithic" is that the helpers are composed in
one function and bundled into one `LoadedDocument` struct with 11
fields. That's a type-state and API-design problem, not a crate-boundary
problem. Fixing it requires staging the pipeline into typed stages
(`HtmlSource → ParsedDocument → StyledDocument → LaidOutDocument →
PaintedDocument`, per Dumpty) and giving each stage its own type.
Moving files between crates does not address either concern.

**H2. `koala-paint` is too small to be a crate.**
`crates/koala-css/src/paint/` is 586 lines total (`painter.rs` 375 +
`display_list.rs` 181 + `mod.rs`). It has one consumer (the renderer)
and is tightly coupled to `LayoutBox`, `BoxType`, `FragmentContent`,
`PositionType`, and `ComputedStyle` (all imported at
`painter.rs:12-16`). Promoting it to a crate adds a build-graph node
and a publish boundary for a thin layout→display-list transformer.
**Recommendation:** keep `paint` as a module inside whichever crate
owns layout (or keep it in `koala-css` until layout moves). Do *not*
create `koala-paint` as its own crate.

**H3. `image_dimensions` layering wart survives the move.**
`build_layout_tree(image_dimensions: &HashMap<NodeId, (f32, f32)>)` at
`crates/koala-css/src/layout/layout_box.rs:1000-1004,1339`. Cheshire
confirmed this is the *only* place layout knows about loaded image
data — tractable, but not touched by the proposal. After the split,
`koala-layout::build_layout_tree` still receives a `HashMap` from
outside the layout crate. The wart is renamed, not fixed.
**Fix option:** introduce an `IntrinsicSizeProvider` trait (or move
image decoding above layout) in the same work package.

**H4. Ownership of `DisplayCommand`'s payload types is undefined.**
`FontStyle`, `TextDecorationLine`, `BorderRadius`, and `ColorValue`
are used by *both* layout and paint, and also by the renderer. Today
they all live under `koala-css` so the dependency is trivial. Under
the proposed graph (`css → layout → paint`), paint imports from
layout is fine, but the renderer (now in `koala-gfx`) still imports
these types from `koala-css`. The proposal does not specify which
crate owns these types after the split. **Fix:** name the home for
each of these types up front, audit every import site, and list them
in the migration plan.

**H5. Documentation and memory drift is already present and will
compound.** Dormouse catalogued: `CLAUDE.md:170-179` (architecture
tree) still lists deleted `koala-gui/` and omits `koala-js/` and
`koala-common/`; `CLAUDE.md:214,220` still describe the deleted egui
Development GUI; `MEMORY.md` has at least 5 hard-coded paths into
`crates/koala-css/src/layout/*.rs` that will silently misdirect every
future Claude session if not updated atomically with any move. The
refactor must update both files in the same commits that move code,
or the project's own documentation will become actively misleading.

### Medium — fix when touched

- **M1.** The "pipeline is fused" claim is overstated.
  `paint/painter.rs:67` (`Painter::paint(&LayoutBox) -> DisplayList`)
  and `renderer.rs:240` (`render(&DisplayList)`) are already separate
  stages with a serializable intermediate (DisplayList). The author
  conflated "no Paintable *tree*" (true) with "no paint *stage*"
  (false).
- **M2.** "`parse_html_string` is monolithic and can't be
  decomposed" is factually wrong — the helpers are already
  independently callable. The real issue is API exposure, not
  function structure (Tweedle).
- **M3.** Font concern #6 is **already partially fixed** by the task
  #6 perf work (`RendererFonts`, `OnceLock`-cached `FontProvider` in
  `koala-qt/src/browser_page.rs:25-38`). The residual smell is that
  `RendererFonts` (raster-time) and `FontProvider` (layout-time) are
  two parallel caches of the same four files — a real issue, but a
  different one than the original claim.
- **M4.** Renderer name collision: after extracting pixel ops to
  `koala-gfx::Painter`, the bare `Renderer` in `koala-browser`
  residue becomes semantically meaningless ("what does it render?").
  Rename to `PageRenderer` or fold into a `render_page()` function.
- **M5.** `crates/koala-css/tests/layout_tests.rs` is 2,200+ lines
  of layout + painter tests with imports like
  `koala_css::{LayoutBox, Painter, default_display_for_element}`.
  Any crate move requires `git mv` + import rewrites.
- **M6.** ~400 lines of `crates/koala-css/src/lib.rs` are
  `extract_style_content`, `extract_all_stylesheets`,
  `fetch_external_stylesheet` — HTML/DOM/network plumbing, not CSS.
  "Slimming koala-css" should eject these too, or the slimmed crate
  still carries a `koala-dom` + `koala-common::net` dependency for
  non-CSS reasons.
- **M7.** Layout subdirectory must flatten. Preserving
  `koala-css/src/layout/` inside a new `koala-layout` crate creates
  `koala_layout::layout::flex` — stuttering. Crate precedent
  (`koala-html/src/parser/`, `koala-css/src/{cascade,selector}/`) is
  concept-first under crate root.
- **M8.** `layout-trace` cargo feature flag migration (CLAUDE.md
  lines 226-244 document it) must be planned.
- **M9.** The b746d15 typed-errors convention is narrower than the
  commit message suggests: only three `*Error` enums exist
  (`LoadError`, `ImageError`, `FetchError`), all at I/O boundaries.
  `koala-css`, `koala-html`, `koala-dom`, `koala-js` use no typed
  errors; the renderer still uses `anyhow`. Inventing a
  `GfxError → LayoutError → PaintError → BrowserError` `#[from]`
  chain would invent a convention that does not exist. The split
  should keep the renderer on `anyhow` to match precedent.
- **M10.** `koala-gfx` as a name mildly violates the workspace's
  spelled-out-hyphenated convention (`common`, `browser`, `html`,
  `dom`). `koala-render` would fit better, but it collides with the
  existing `Renderer` struct and with the verb "render" used in
  `paint/mod.rs`'s pipeline diagram. Dumpty's verdict: `koala-gfx`
  is the least-bad option — keep it.

## Deferred (explicitly out of scope)

These are real issues but belong to separate work packages:

- **D1.** `layout_box.rs` decomposition along formatting-context
  lines (block/inline/flex/grid/table). ~3,740 lines, its own
  project.
- **D2.** `ComputedStyle` decomposition into per-property-group
  structs. ~2,863 lines.
- **D3.** Paintable tree as a semantic refactor — z-index resolution,
  stacking context construction. Needs its own justification doc.
- **D4.** `parse_html_string` type-state refactor (`HtmlSource →
  ParsedDocument → StyledDocument → LaidOutDocument →
  PaintedDocument`). High-value API improvement, independent of crate
  moves.
- **D5.** `LoadedImage → DecodedImage` rename (Dumpty's concurrent
  cleanup suggestion — accurate but orthogonal).
- **D6.** JS↔DOM binding infrastructure. `koala-js` has no
  `koala-dom` dependency today; Cheshire confirms there is no
  JS→DOM path. Until this exists, `koala-js`'s place in the
  dependency graph is effectively frozen.

## Agent findings table

| Agent | Focus | Top finding | Category | Impact |
|---|---|---|---|---|
| Rabbit | Scope & timeline | Defer the full split; do one micro-step (`koala-gfx` extraction of renderer.rs + font_metrics.rs) and go straight to #7 | scope | — |
| Queen | Correctness | Only 3 of 8 concerns cleanly solved by the proposal; `layout_box.rs` not decomposed so "god crate" is relabeled | correctness | critical |
| Cheshire | Edge cases | `style::computed` already imports from `layout/`; circular dep the moment the split lands | integration | critical |
| Tweedle | Claims | 3 of 8 claims factually wrong against the current code (pipeline fused, parse monolithic, dep backwards); 1 already fixed by task #6 | correctness | high |
| Dormouse | Consistency | MEMORY.md + CLAUDE.md already drifting; refactor compounds without atomic doc updates | assumption | high |
| Dumpty | Nomenclature | `Painter` name collision between `koala-css::paint::Painter` and proposed `koala-gfx::Painter`; rename first | nomenclature | critical |

## Consensus findings

What more than one agent independently flagged:

1. **FAIL verdict.** Queen and Tweedle both returned explicit FAIL.
   Rabbit recommended DEFER. Cheshire, Dormouse, and Dumpty each
   found blocking issues.
2. **Paint cannot be extracted below CSS.** Queen, Cheshire, and
   Tweedle all confirmed `paint/painter.rs` imports `LayoutBox`,
   `BoxType`, `FragmentContent`, `PositionType`, `ComputedStyle`
   directly. Paint is fundamentally tied to layout and computed
   style; moving it to a separate crate does not reduce coupling.
3. **Pipeline is not fused.** Queen and Tweedle both confirmed a
   distinct paint stage already exists with `DisplayList` as the
   intermediate. The author conflated "no Paintable tree" with "no
   paint stage."
4. **`layout_box.rs` is the elephant.** Rabbit and Queen both
   flagged the 3,740-line file as unchanged by the refactor and as
   the real location of concern #1's problem.
5. **`koala-paint` shouldn't be a crate.** Queen (586 lines, too
   small, one consumer) and Cheshire (fundamentally coupled to
   layout types) independently recommended module-not-crate.
6. **`koala-gfx` is not web-agnostic.** Queen, Cheshire, and
   Tweedle all identified that the Renderer imports CSS-domain
   types and cannot be a leaf crate without a type redesign.
7. **Tasks #7–#10 don't depend on this.** Rabbit explicitly;
   Queen implicitly via the list of unaddressed concerns.

## Recommended course of action

### Phase 0 — precursor (unblocks everything else)

1. **Move style-value enums out of `layout/`.**
   `FontStyle`, `TextDecorationLine`, `TextAlign`, `ClearSide`,
   `FloatSide`, `PositionType` → `style/values/`. Single-crate
   change inside `koala-css`. Fixes C1 and prepares the ground for
   any future layout extraction. **Atomic, landable now, breaks no
   public API.**

2. **Rename `koala-css::paint::Painter` → `DisplayListBuilder`.**
   One file, mechanical. Frees the name for `koala-gfx::Painter`
   later. Fixes C2. **Atomic, landable now.**

3. **Update `CLAUDE.md` architecture tree and `MEMORY.md` file
   paths to reflect current reality** (pre-existing drift from the
   koala-gui removal). H5.

### Phase 1 — minimum viable extraction (after Phase 0 lands cleanly)

4. **Extract `koala-gfx` from `koala-browser::renderer.rs` +
   `font_metrics.rs` only.** Rabbit's recommendation. Do NOT touch
   `layout/`, `paint/`, or `LoadedDocument`. Accept and document
   that `koala-gfx` depends on `koala-css` for
   `DisplayCommand`/`BorderRadius`/`ColorValue`/`FontStyle` (C3):
   this is not a cycle, but it falsifies the "web-agnostic LibGfx"
   framing — update the proposal's wording accordingly. The
   extracted crate gives task #7 (worker thread) a clean
   `Send`-able boundary to depend on.

### Then proceed with tasks #7–#10 unchanged.

### Phase 2 — deferred (re-evaluate after #10 ships)

Only revisit these if concrete pain points surface during #7–#10:

- Extract `koala-css::layout::*` into `koala-layout`.
- Split `layout_box.rs` along formatting-context lines (D1).
- Split `ComputedStyle` (D2).
- Stage `parse_html_string` into typed pipeline (D4).
- Introduce Paintable tree if a real z-index / stacking-context
  need emerges (D3).

## Rabbit hole status

**Not in a rabbit hole, but adjacent to one.** The proposed refactor
is a classic architecture rabbit hole: grievances that are partly
real, partly mischaracterized, plus a Ladybird analogy that doesn't
survive contact with koala's actual code. Doing the full split now
would burn 1–2 days of build-break window while introducing
circular deps, phantom requirements, and two `Painter` structs.

**Return to main thread:** finish task #6 cleanup (already done),
proceed with Phase 0 (~90 min of mechanical work), then task #7
(worker thread — the real architectural work hiding behind the
refactor proposal). Defer the crate split until there's concrete
evidence it's needed.

## Files cited

- `crates/koala-css/src/lib.rs` (re-export surface)
- `crates/koala-css/src/style/computed.rs:10-12` (circular dep)
- `crates/koala-css/src/layout/layout_box.rs` (3,740 lines)
- `crates/koala-css/src/layout/inline.rs` (FontStyle, TextDecorationLine, FontMetrics trait)
- `crates/koala-css/src/layout/float.rs` (ClearSide, FloatSide)
- `crates/koala-css/src/layout/positioned.rs` (PositionType)
- `crates/koala-css/src/paint/painter.rs:48,67` (Painter struct, paint method)
- `crates/koala-css/src/paint/display_list.rs` (DisplayCommand)
- `crates/koala-css/tests/layout_tests.rs` (2,200+ lines)
- `crates/koala-browser/src/renderer.rs:21-22` (CSS type imports)
- `crates/koala-browser/src/renderer.rs:103-110` (RendererFonts)
- `crates/koala-browser/src/lib.rs:50-87` (LoadedDocument, 11 fields)
- `crates/koala-browser/src/lib.rs:156-212` (parse_html_with_base_url)
- `crates/koala-browser/src/font_metrics.rs`
- `koala-qt/src/browser_page.rs:14-17,25-38` (cached fonts + bridge imports)
- `koala-qt/src/bridge.rs:13-35` (extern "Rust" BrowserPage)
- `CLAUDE.md:170-220` (stale architecture + egui references)
- `~/.claude/projects/-Users-alvinkuruvilla-Dev-koala/memory/MEMORY.md`
  (5 hard-coded layout paths)
