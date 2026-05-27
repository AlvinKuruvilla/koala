# SESSION — deferred tasks and engine gaps

Scratch space for things noticed while working that are worth
fixing but shouldn't block the current task. Per the global
CLAUDE.md convention: write observations here, don't fix them
inline unless they block progress.

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

## Real WPT testharness tests still don't report subtests

End-to-end `wpt run --product=koala dom/nodes/Element-childElementCount-nochild.html`
gets to a clean `TEST_END: OK` with a working
`testharnessreport-koala.js`, but the user's `test(...)` block
never fires `add_result_callback`. The fixture's HTML loads
testharness.js + testharnessreport.js + a synchronous
`test(function () { assert_equals(p.childElementCount, 0); })`,
and none of the post-setup sentinels I added to the reporter
fired for the user test (only the setup-time ones).

What we ruled out during the validation:
- `testharnessreport-koala.js` IS being served (sentinels prove
  it; the wptrunner HTTP route override works).
- `setup({...})` succeeds, `add_result_callback` /
  `add_completion_callback` succeed.
- Boa's Promise/microtask queue IS now being drained (separate
  fix in `fix(js): drain Boa microtask / Promise job queue`).
  Didn't unlock subtests.

The actual blocker is somewhere deeper in testharness.js's
test-scheduling internals — possibly `queueMicrotask`,
`MutationObserver`, structured cloning, `Promise.then` chain
order under our microtask drain, or some other API that
testharness.js expects but koala doesn't yet provide.

Reproducible cheaply: run
`.venv-wpt/bin/python3 third-party/wpt/wpt run --binary=$PWD/target/release/koala
--timeout-multiplier=5 --log-wptreport=/tmp/r.json koala
/dom/nodes/Element-childElementCount-nochild.html`
and inspect `/tmp/r.json` — `subtests` is empty.

Next debug step is to add per-line sentinels INSIDE
testharness.js's `Test.prototype.run` (or wherever test() lands)
to find the first line where execution stops. Or write a tiny
WPT-format test that bypasses `test()` entirely and just calls
`__koala_emit_result__` directly — that should at least confirm
the end-to-end path past the harness.
