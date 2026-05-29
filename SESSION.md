# SESSION — deferred tasks and engine gaps

Scratch space for things noticed while working that are worth
fixing but shouldn't block the current task. Per the global
CLAUDE.md convention: write observations here, don't fix them
inline unless they block progress.

## Perf harness — deferred items

- **`just bench-diff` for comparing JSON reports.** `just bench`
  emits structured JSON to stdout; comparing two reports today
  is manual (`diff`, `jq`, eyeballing). A small Rust subcommand
  (`koala --bench-diff before.json after.json`) that prints
  per-stage % deltas with red/green highlighting would close the
  regression-detection loop. Defer until we have enough bench
  runs that manual diffing gets cumbersome.

## koala-css gaps surfaced by the landing / error page redesign

The landing page (`koala-qt/res/landing.html`) and error page
(`koala-qt/res/error.html`) were intentionally designed for the
look we want rather than the look the current engine supports,
to give us a concrete target for filling gaps. They use the
following CSS features that koala-css doesn't support yet;
each one will emit a parser warning and likely cause some
visual degradation until it lands:

- **`letter-spacing`** — used for tracked section labels and
  negative tracking on large headlines. Currently emits
  `[Koala CSS] ⚠ unknown property 'letter-spacing'`.
- **`text-transform: uppercase`** — used for labels. Would
  otherwise force us to hand-uppercase every label in source.
- **`box-sizing: border-box`** — `* { box-sizing: border-box; }`
  reset. Without this, padded containers overflow their widths.
- **`:last-child` pseudo-class** — used on `.shortcut:last-child`
  to drop the bottom border of the last row. Falls back to an
  extra line if unsupported.
- **Universal selector `*`** — used only for the `box-sizing`
  reset. Depends on whether selector matching handles `*`.
- **`word-break: break-all` / `word-break: break-word`** — used
  on long URLs and error messages so they wrap inside their
  code blocks instead of horizontally overflowing.
- **`-webkit-font-smoothing: antialiased`** — vendor-prefixed;
  safe to ignore. Only listed for completeness.
- **`⌘` glyph (U+2318)** — used in the shortcut table. Text
  rasterizes as tofu until font fallback lands
  (`FontdueFontMetrics` should cascade into Apple Symbols or
  similar when the primary font can't provide a glyph).
- **Pill-shaped `border-radius: 999px`** — used on the error
  eyebrow badge. Should work with current border-radius impl
  but `999px` is intended as "half the height"; if border-radius
  is clamped to box dimensions this is fine, otherwise it may
  render oddly.

None of these are urgent. The landing and error pages already
render legibly without them; fixing them will progressively
polish the look.

## Other observations

- **`InheritedTextProps` bundle for `layout_inline_content`** —
  the inline-layout entry point now takes nine arguments
  (`inherited_font_size`, `inherited_color`, `inherited_font_weight`,
  `inherited_font_style`, `inherited_text_decoration`,
  `inherited_letter_spacing`, plus viewport/font_metrics/etc.) and
  grows one per new text property. The `add_text` /
  `place_text_fragment` / `find_break_opportunity` trio takes
  overlapping subsets of the same cluster. Worth wrapping the
  text-shaping subset in an `InheritedTextProps` struct so the
  next property (word-spacing, text-indent) adds one field
  instead of one parameter in five places. Same shape would
  collapse the four `LayoutBox` constructor sites where the
  cluster is duplicated. Surfaced while adding `letter-spacing`
  — not worth doing in that single-property commit, but the
  next text property is a natural trigger.

- **Migrate remaining property parsers to
  `style/values/helpers.rs`** — the helpers (`contains_keyword`,
  `first_keyword`, `first_px_length`, `first_number`,
  `first_percentage`) landed in commit `a18aff4` and
  `parse_letter_spacing` is the proof-of-concept customer.
  The codebase still has ~10 sites doing the old `for v in
  values { match v { … } }` dance that the helpers fully
  subsume. Inventory:

  - `style/values/font.rs` — three sites: `parse_line_height`
    (px), `parse_font_weight` (`normal` / `bold`).
  - `style/values/length.rs` line 247 — `parse_single_auto_length`
    `auto` keyword. (Lines 203–211 stay inline; they're the
    foundation `first_px_length` wraps.)
  - `style/display.rs` line 230 — `none` keyword.
  - `style/computed.rs` — lines 1271 / 1293 / 1613 (`none`
    checks), 2321 / 2327 (another `normal | <length-px>`
    parser, direct copy of letter-spacing), 2461 (`inset`
    for box-shadow), 2658 / 2724 / 2912 (`auto` scattered
    through track sizing).

  Recommended as a single sweep commit when next in the
  area — the helpers are stable, the sites are small, and
  per-parser consistency is the win.

  Outside the helpers' current vocabulary (would need new
  helpers first):
  - Grid track sizing in `computed.rs:2643-2715` matches
    `fr` / `px` / `em` in sequence. Needs a multi-unit
    length helper or an `fr` companion.
  - Function-name matching (`computed.rs:2664` `repeat(…)`,
    `style/substitute.rs` `var(…)`) is a different shape
    (`ComponentValue::Function { name, … }`) and would want
    its own `first_function_call(values, name)` helper.

- **Split `renderer.rs` along concern lines** — file is 980
  lines mixing four self-contained chunks: font search
  paths + `RendererFonts` loading (~150 lines), `draw_text`
  (~140), `draw_box_shadow` + its outer/inset helpers (~225),
  and the shared primitives (`fill_rect`, `is_visible`,
  buffer helpers). The mixing isn't tangled — each chunk is
  internally cohesive with negligible cross-coupling — so
  this is honest cleanup, not detangling. The split worth
  doing is `renderer/fonts.rs`, `renderer/shadow.rs`,
  `renderer/text.rs`, leaving `fill_rect` / `is_visible` /
  `draw_image` in the parent `renderer/mod.rs` as the
  shared primitives everything calls back to. Doesn't change
  the API or address the real structural smell (every paint
  method is `&mut self` on a god-object); a proper `Painter`
  trait extraction is a separate, bigger conversation.
  Low-risk, medium-reward, cosmetic.

- **Split `computed.rs` along property families** — the file
  is ~3000 lines and the property dispatcher is one giant
  `match` arm-per-property. Every new property has to land
  somewhere in that match, and finding the right
  neighbourhood today means scrolling thousands of lines.
  Splitting into `computed/text.rs`, `computed/font.rs`,
  `computed/box.rs`, `computed/flex.rs`, `computed/grid.rs`
  (one file per CSS module, mirroring the spec's own
  organisation) lets the dispatcher delegate to per-family
  handlers and tells you where any new arm belongs without
  thinking. Doesn't speed up the *first* property after the
  split; speeds up every subsequent property and makes PR
  diffs scoped to their family. Bigger lift than the
  renderer split because the dispatcher has more shared
  state to thread (the `ComputedStyle` it writes into, the
  cascade context), but mechanically straightforward — the
  big-match-statement-to-jump-table pattern is well-trodden.

- **`koala-shape` crate is missing** — currently we
  rasterise codepoints directly through `fontdue`, which has
  no concept of OpenType feature tags, shaping, ligatures,
  contextual alternates, or complex scripts (Arabic,
  Devanagari, etc.). This is the gap that blocks
  `font-feature-settings`, `font-variant-*`,
  `font-variation-settings`, and a large swath of typography
  work generally. Already referenced as a future
  spec-implementation track in project memory; flagging it
  in SESSION so it's visible when typography requests come
  up — those requests can't be partially served; they need
  shaping infrastructure to exist first. Build-from-scratch
  (HarfBuzz-equivalent, the koala way) or vendoring decision
  is its own conversation.

- **`IDIOSYNCRASY` convention + aggregator** — for engine
  divergences that *accept* input but handle it
  approximately or incompletely. This is the silent class
  of bug: not a parse failure (those already surface as CSS
  warnings / parse_issues), not a panic, just "wrong but
  plausible" output. Examples: `letter-spacing: 2em`
  returns `None`, `font-feature-settings` parses but never
  reaches the rasterizer, box-shadow approximated with
  concentric-circle blur, margin-collapsing chains beyond
  parent-child, flex stretch sets `content.X` directly
  instead of re-running child layout. Other browsers
  handle this with DevTools "Issues" panel + WPT scoreboard
  (silent at render, queryable at inspection). Our
  equivalent should be the same: a `// IDIOSYNCRASY(cat):
  message` source-tag convention with categories
  `spec-deviation` / `approximation` / `unimplemented` /
  `partial`, plus a `koala-debug` binary that walks the
  workspace and emits a categorized report (`just
  probe-idiosync`). Static-time only — runtime accounting
  is the next graduation. First commit when picked up:
  add the convention to CLAUDE.md, convert ~4–6 existing
  `TODO(letter-spacing)` / `TODO(content-main-size)` /
  flex `§ 9.x` deviation comments to the new tag, write
  the aggregator + justfile recipe.

- **Boa 0.21+ has 6 `parse_issues` on overleaf** — the Boa
  bump fixed the 46 GB for-in OOM but the page still returns 6
  JS parse errors from the inline-script pump. They don't break
  rendering; they're a backlog of real-world JS constructs Boa
  doesn't yet accept. Look at `parse_issues` on a fresh
  `oom_probe https://www.overleaf.com` run when work toward
  better real-site fidelity resumes.

- **Native form control rendering** — `<input>`, `<select>`,
  `<textarea>`, `<button>`, and friends currently lay out from
  their HTML structural boxes with UA-stylesheet defaults; no
  widgets get painted. Consequence: checkboxes, radio buttons,
  dropdowns, and text fields look wrong (or invisible) in
  screenshots of any real site with a form. The `appearance`
  / `-webkit-appearance` arm in `computed.rs` is a *temporary*
  no-op tied to this gap — when form-control rendering lands,
  that arm must be replaced with real keyword handling.

  This work is also the natural home for the semantic state an
  agent API needs to expose (`checked`, `selected`, `value`,
  `disabled`), so it should land alongside or just after the
  render-tree-as-typed-API work rather than before. Painting
  controls is the easy part; the state model and event plumbing
  (which depends on JS being wired through the DOM) is where
  the real complexity lives.

## Real WPT testharness tests now report subtests (resolved)

**Resolved** in `fix(js): top-level Window self-references for testharness.js`.

Root cause: testharness.js's `_forEach_windows` walked
`self → self.parent → self.parent.parent …` looking for the
top-level WindowProxy, and `koala-js` exposed `self`/`window`
but not `parent`/`top`/`opener`. The loop dereferenced `.parent`
on `undefined`, throwing `TypeError: cannot convert 'null' or
'undefined' to object` inside `Tests.prototype.start →
notify_start → message_functions.start`. The throw escaped
through every `test()` call before the user function ran, so
`add_result_callback` never fired.

Fix: `register_window` now also installs `window.parent` and
`window.top` as self-references and `window.opener = null`,
which are the spec values for a top-level browsing context with
no parent/opener.

Verification: `wpt run --product=koala
/dom/nodes/Element-childElementCount-nochild.html` now produces
`status=OK` with one subtest reported (FAIL, because
`Element.childElementCount` itself isn't implemented yet —
that's a separate DOM gap, captured below).

## Engine pump waits for harness setTimeout (resolved)

**Resolved** in `perf(js): early-exit pump_until_idle once
testharness completion fires`.

Two changes landed together:

- `JsRuntime::pump_until_idle_or<F>` accepts a stop predicate
  consulted between iterations and before sleeping. The
  existing `pump_until_idle` delegates with `|_| false`.
- The DCL→`load` lifecycle now uses a new
  `JsRuntime::drain_due_tasks` that processes currently-due
  timers + microtasks without sleeping for future ones. The
  testharness watchdog `setTimeout` no longer blocks `load`
  from firing.

`TestharnessHook::should_stop_pumping` reads
`koala_wpt::has_test_completion` (a non-draining peek of the
`__koala_test_completion__` slot). Once the harness completion
callback has fired the post-load pump exits on its next
iteration. async / promise tests still drain correctly — they
register their completion via the same callback path; the
pump just keeps running until that callback fires.

Verification: the sync test
`/dom/nodes/Element-childElementCount-nochild.html` now runs in
~19 ms inside wptrunner (was ~50 003 ms — bounded by the
harness timeout × `timeout-multiplier`).

## Pre-existing clippy errors unmasked

`cargo clippy --workspace` previously failed on the first error
in `koala-common/src/net.rs:144` (collapsible-if). With that one
fixed, clippy now runs further into the tree and surfaces ~8
pre-existing style errors across `koala-js` (mostly
`doc_markdown` and a stray `needless_borrow` in
`globals/events.rs`). None are new regressions — they were
silently masked while the koala-common one short-circuited the
run. Mechanical to fix; bundling them with the next round of
crate hygiene rather than rolling them into the WPT-fix
changeset.

## DOM gaps surfaced by the now-working WPT pipeline

With testharness reporting fixed, the first concrete DOM gap
that fails real tests:

- `Element.childElementCount` is `undefined`. WPT test
  `/dom/nodes/Element-childElementCount-nochild.html` fails with
  `assert_equals: expected (number) 0 but got (undefined) undefined`.
  The property is straightforward (count of `Element` children),
  and once it lands the test should pass cleanly.
  **Resolved** in the Tier-1 Element / HTMLElement migration —
  `childElementCount` now lives on `Element.prototype` and the
  smoke test flips from FAIL to PASS.

## Bugs surfaced by running koala-cli against real sites

After wiring JS-error surfacing into the Qt browser, smoke
tests against a handful of real pages with
`./target/release/koala <url>` turned up two follow-ups worth
filing.

### Relative `<script src>` URL resolution is wrong on bare-name paths

Loading `https://news.ycombinator.com` produces:

```
! Failed to load <script src="hn.js?SMNcJPuowwn2FRyKwpFD">:
  request to 'https://hn.js?SMNcJPuowwn2FRyKwpFD' failed:
  error sending request for url (https://hn.js/?SMNcJPuowwn2FRyKwpFD)
```

The HTML is `<script src="hn.js?SMNcJPuowwn2FRyKwpFD"></script>`
and the base URL is `https://news.ycombinator.com/`. Per RFC
3986 § 5.2 the relative reference should resolve to
`https://news.ycombinator.com/hn.js?SMNcJPuowwn2FRyKwpFD`. Our
`koala_common::url::resolve_url` is instead treating `hn.js`
as a hostname.

Likely cause: the resolver sees no leading `/` and no scheme,
and the URL crate's `Url::parse` interprets `hn.js?…` as
`scheme: hn`, `host: js`, `query: …` or similar. The fix is
probably to detect "purely relative reference, no scheme, no
authority" and join against the base path explicitly.

Reproducer: `./target/release/koala https://news.ycombinator.com`
— the bug shows up in the "Parse Issues" section.

### Boa parser rejects some real-world inline JS

Loading `https://en.wikipedia.org/wiki/Web_browser` surfaces:

```
! JavaScript error (in inline): SyntaxError: expected token ';',
  got ':' in expression statement at line 1, col 12
```

Boa's parser can't handle whatever Wikipedia's first inline
script does at line 1 col 12. Probably a modern-ES feature
that 0.20 doesn't accept (labelled statement? destructuring
assignment as a statement? property shorthand in a place we
read as an expression?). Diagnosis would mean extracting the
exact inline source and bisecting against Boa.

Tier-3 work — fixing this means either upgrading Boa to a
newer release with broader ES coverage or working around the
specific syntax. Not blocking koala's WPT path (testharness.js
doesn't use the offending syntax), but a recurring hit when
testing against real sites.
