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

- **Grid layout panics on real-world sites** — `grid.rs:502`
  slice out-of-bounds when rendering overleaf.com. Worked
  around by catching panics in the render worker (task #12);
  the actual bug in the grid formatting context is still open.

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
