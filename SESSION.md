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

- **Widen `Request` type when the request layer evolves,
  don't multiply senders** — the current
  `koala_common::net::RequestSender` trait takes a bare URL
  (`fn fetch(&self, url: &str) -> …`). When the project
  hits one of two triggers — async load (likely first,
  driven by the WPT path or multi-tab perf), or a need for
  per-resource-kind policy (caching, prioritization,
  blocking semantics) — the temptation will be to install
  multiple senders or use the existing `MappedSender`
  composition for kind-based routing. Don't. Real browsers
  (Chrome `URLLoaderFactory`, Firefox `nsIChannel`, Servo
  `CoreResourceManager`) all converged on the *same*
  answer: **one uniform trait surface, with a smart
  subsystem that specializes internally based on request
  metadata.** Call sites stay simple; everything about
  *how* (cache, threading, transport, priority) lives in
  the impl.

  ### Future-state trait shape

  ```rust
  enum ResourceKind { Html, Stylesheet, Script, Image, Font, Generic }
  enum Priority { High, Normal, Low }
  enum CacheMode { Default, NoStore, ReloadIgnoringCache }

  struct Request {
      url: String,
      kind: ResourceKind,
      blocking: bool,        // does load block render / parser?
      priority: Priority,
      cache: CacheMode,
      // room to grow: integrity hash, CORS mode, referrer, …
  }

  trait RequestSender {
      // Sync today; becomes `async fn` when koala-browser goes async.
      fn fetch(&self, req: Request) -> Result<Vec<u8>, FetchError>;
  }
  ```

  ### Worked example: `@font-face` URL load

  Inside `DefaultSender`, the dispatch fans out by kind
  and each branch implements the right policy:

  ```rust
  fn fetch(&self, req: Request) -> Result<Vec<u8>, FetchError> {
      match req.kind {
          ResourceKind::Font => self.fetch_font(&req),
          ResourceKind::Image => self.fetch_image(&req),
          ResourceKind::Stylesheet | ResourceKind::Script
              => self.fetch_blocking_text(&req),
          _ => self.fetch_generic(&req),
      }
  }

  fn fetch_font(&self, req: &Request) -> Result<Vec<u8>, FetchError> {
      if let Some(bytes) = self.font_cache.get(&req.url) {
          return Ok((*bytes).clone());        // Arc clone is cheap
      }
      let bytes = self.network_pool.get(&req.url, req.priority)?;
      if !is_valid_font_magic(&bytes) {
          return Err(FetchError::InvalidFont { url: req.url.clone() });
      }
      self.font_cache.insert(req.url.clone(), Arc::new(bytes.clone()));
      Ok(bytes)
  }
  ```

  Things only the font branch does (justifying the
  per-kind dispatch): font-magic validation, a process-wide
  font cache keyed by URL (fonts are reused across pages),
  low priority hint (fonts don't block render — FOUT). The
  image / script / stylesheet branches each have their own
  specifics (image format detection, script CORS + integrity,
  parser-blocking stylesheet semantics).

  ### Font cache shape

  ```rust
  struct FontCache {
      map: RwLock<HashMap<String, Arc<Vec<u8>>>>,
      lru: Mutex<LruOrder<String>>,
      max_bytes: usize,                  // ~50 MB ceiling
      current_bytes: AtomicUsize,
  }
  ```

  Decisions worth pinning down in the actual commit:
  canonicalize URL keys (lowercase scheme/host) so
  `Example.com/foo.woff` and `example.com/foo.woff` aren't
  two slots; share via `Arc<Vec<u8>>` so CSS + renderer +
  glyph atlas don't copy 500 KB × N times; use a coalescing
  `Arc<OnceLock<Vec<u8>>>` per slot so two threads
  requesting the same uncached font don't both fetch.
  Optional disk layer is safe because fonts are
  content-addressed by URL.

  ### Async transition is trivial

  Sync `let b = self.network_pool.get(&url, Priority::Low)?;`
  becomes `let b = self.network_pool.get(&url, Priority::Low).await?;`.
  Cache lookup stays sync (just a hashmap read). Call sites
  add `.await` once; nothing else changes.

  ### Migration trigger

  Do this when *either* (a) koala-browser's load pipeline
  goes async — likely driven by the WPT path or by needing
  multiple concurrent subresource loads, or (b) a real
  per-kind policy need surfaces (e.g. "fonts should cache
  across pages but images shouldn't"). Both are plausible
  within the next handful of milestones. The migration
  itself is bounded: widen the trait + Request type,
  update the ~10 call sites in koala-browser/koala-css to
  pass `kind`, leave `DefaultSender` impl simple at first
  (one cache, no priority queue) and grow it as needs
  emerge.

  ### Sibling concern

  `koala-js::dom_handle` and the koala-js scheduler use the
  same thread-local + RAII-guard pattern and share the same
  async-trigger. Their destination is different — Boa's
  `HostDefined` on the `Context`, not Request-widening —
  but the work will surface at the same moment. See the
  next entry for the worked migration plan.

- **Migrate `koala-js::dom_handle` and the scheduler to
  Boa `HostDefined`** — sibling to the `RequestSender`
  evolution above, with the same trigger (async / off-main-
  thread JS) but a different destination because the
  problem isn't "ambient context for call sites," it's
  "Boa native callbacks are `Fn + Copy + 'static` and
  can't capture references to host state." The thread-local
  pattern exists today as the workaround for that callback
  shape, not as a general-purpose ambient state choice.

  ### The destination: `Context::host_defined_mut()`

  Boa ships a typed slot map *on every `Context`*
  specifically for stashing host-side Rust state that
  callbacks need. We already depend on it — the
  `get_many_mut` → `get_disjoint_mut` patch in
  `crates/boa/core/engine/src/host_defined.rs` (commit
  `302922a`) is in that file. The migration shape:

  ```rust
  // Today (dom_handle.rs):
  thread_local! {
      static CURRENT: RefCell<Option<DomContext>> = const { RefCell::new(None) };
  }
  // … install via guard before every `runtime.execute(source)`.

  // After: install once on `JsRuntime::new`.
  context.host_defined_mut().insert(DomHandle::new(dom));
  context.host_defined_mut().insert(Scheduler::new());

  // Inside any native callback:
  fn document_query_selector(
      _this: &JsValue,
      args: &[JsValue],
      cx: &mut Context,
  ) -> JsResult<JsValue> {
      let dom = cx.host_defined().get::<DomHandle>().unwrap();
      // … same logic as today, just sourced from Context instead
      // of the thread-local.
  }
  ```

  Key property: `HostDefined` lives on the `Context`, so
  wherever the `Context` moves (off-main-thread execution,
  multiple concurrent contexts in the same process), the
  host state moves with it. **Thread-affinity becomes a
  non-issue** without async ceremony — this is the
  async-safe answer for koala-js the same way
  `tokio::task_local!` would be the async-safe answer for
  `RequestSender`.

  Cost: every callback pays one
  `cx.host_defined().get::<T>()` lookup (typed slot-map
  query) instead of a thread-local read. For the call
  rates real pages hit (a few thousand DOM ops + timer
  schedules per page load) immeasurable. The
  `pattern_thread_local_guard` memory entry will be
  retired the same day this lands — the pattern stops
  being our recommended idiom because Boa has a more
  appropriate primitive.

  ### Scheduler gets *both* widenings

  `dom_handle` is "just" the HostDefined migration — DOM
  operations are *already* specialized (`querySelector`,
  `getElementById`, `appendChild` are different paths in
  the host code), so there's no analogous "widen the type"
  story.

  The scheduler is different. Today koala-js has
  `setTimeout` + `setInterval`. The spec-compliant browser
  surface also includes:

  - `queueMicrotask` — microtasks (different queue, runs
    at end of current task; load-bearing for Promise
    behaviour we don't fully have yet).
  - `requestAnimationFrame` — runs before next paint,
    priority class of its own.
  - `requestIdleCallback` — runs when CPU idle.

  Each is a different priority class with different
  ordering rules. The natural shape:

  ```rust
  enum ScheduledKind {
      Timeout { delay_ms: i32 },
      Interval { interval_ms: i32 },
      Microtask,
      AnimationFrame,
      IdleCallback,
  }

  struct Scheduled {
      kind: ScheduledKind,
      callback_id: TimerId,
  }

  impl Scheduler {
      fn schedule(&mut self, sched: Scheduled) { … }
      // Pump drains per-kind queues in the spec order:
      // microtasks first, then due timers, then rAF, then idle.
  }
  ```

  So the scheduler gets both: HostDefined migration *and*
  Request-style metadata enrichment. They're independent —
  could land in separate commits — but they'll be ready
  for each other.

  ### Migration trigger and ordering

  All three (sender, dom_handle, scheduler) become
  async-relevant at the same moment: when the load
  pipeline or JS execution goes off the main thread.
  Likely first driver is the WPT path (real-browser
  parity) or multi-tab perf. When that happens:

  1. Land the `koala_common::net::RequestSender` widening
     first (smallest blast radius, doesn't depend on
     Boa internals).
  2. Migrate `dom_handle` to `HostDefined` — minimal
     surface, mechanical change, retires
     `pattern_thread_local_guard`.
  3. Migrate the scheduler to `HostDefined`, then in a
     follow-up widen `Scheduled` with `ScheduledKind` and
     add the per-kind queues. Splitting the migration
     from the widening keeps each commit's diff
     reviewable.

  None of this is speculative work — the triggers are
  visible on the roadmap, the destinations are forced by
  Boa's existing primitives, and the migration steps are
  bounded.

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

Additionally, `koala-css` (lib) has a pre-existing
`collapsible-if` clippy error that blocks `cargo clippy -p
koala-cli` (css compiles first in the chain). Surfaced while
wiring allocation counting into the bench harness; unrelated to
that work. Same "next round of crate hygiene" bucket.

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

## koala-ui location bar — residual selection gap

The URL bar now matches mainstream browsers for focus-select-all,
Esc/blur revert, and not clobbering in-progress edits. One gesture
is still imperfect: **double-clicking an *unfocused* URL bar selects
the whole address rather than the word under the cursor.** Slint's
`LineEdit` only exposes `changed has-focus` (not the originating
mouse gesture), so the focus-gain `select-all()` fires before the
double-click's word selection can take effect and wins. Once the bar
is already focused, double-click-to-select-a-word works normally.
Fixing the unfocused case would need lower-level access than the
`LineEdit` widget gives — defer unless it becomes a real annoyance.

## Developer HUD — deferred feature ideas

The `koala-ui` developer HUD (View → Developer HUD, ⌘⇧M) currently
graphs process heap + CPU over time. Brainstormed follow-ups, tabled
2026-06-01:

- **Live per-stage breakdown.** The engine already emits `tracing`
  spans for every load phase (`html_parse`, `css_cascade`,
  `js_execute`, `layout_tree_build`, `rasterize`) — the same data
  `just bench` reads. Surfacing the *last navigation's* per-stage
  timing live in the HUD would turn it into a "why is this page slow"
  view without running the bench harness. Cost wrinkle: koala-ui loads
  run on per-tab **worker threads**, so unlike the single-threaded
  bench harness this needs a thread-safe span collector
  (`Mutex`-guarded), not bench.rs's thread-local log.
- **Per-stage allocation attribution.** Extend the above to bucket heap
  bytes by the phase that allocated them — a cheap, koala-specific
  "where does the memory go" (no backtraces). Needs the allocator to
  read a thread-local "current phase" pushed/popped by a tracing layer.
  The most interesting memory feature; more plumbing than the timing
  version.
- **Leak / growth flag.** Light up when live heap trends monotonically
  upward over a window — directly serves the long-session motivation.
  Cheap heuristic.
- **Reset / pause / always-on-top.** Re-baseline peak + chart per
  navigation (`alloc_count::reset_peak` exists), freeze the chart to
  inspect, keep the HUD above the page. Cheap ergonomics.
- **True heaptrack-style per-call-site attribution.** Backtrace capture
  per allocation + symbolication. Real overhead; its own project.
- **Page-complexity counts.** DOM / layout-box / computed-style node
  counts for the active page; needs koala-browser to expose them.
