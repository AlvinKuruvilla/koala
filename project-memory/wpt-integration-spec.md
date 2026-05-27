---
created: 2026-05-26
area: wpt + koala-browser + koala-js
status: in progress — Phases 1, 1.5, 2 complete; Phase 3 chunk 1 of 3 landed
last_updated: 2026-05-26
---

## Progress at a glance

| Phase | Status | Notes |
|------|--------|-------|
| 1   — wptrunner integration            | ✓ DONE | M1 hit: `wpt run --product=koala /css/CSS2/visudet/content-height-001.html` returns `TEST_END: PASS` end-to-end. Plugin at `wpt-tools/wptrunner-koala/`; subprocess JSON-line protocol implemented in `koala-cli --wpt-protocol`. |
| 1.5 — Observable Framework dashboard  | ✓ DONE | Single-page Linear-styled dashboard at `dashboard/`. Parquet + DuckDB-WASM for per-test drill-down. Crash-reason aggregation, click-to-filter from top crash reasons → tests table. |
| 2   — DOM bridge                       | ✓ DONE | Every spec bullet live (see Phase 2 section). `document.*` + `Element.*` + `window`. DOM mutation triggers a re-cascade + re-layout in `koala-browser`. |
| 3   — event loop                       | ✓ DONE | Chunk 1 (`setTimeout` / `clearTimeout` + `pump_until_idle`), chunk 2 (`setInterval` / `clearInterval` with the shared id pool), and chunk 3 (`EventTarget` mixin on `window` / `document` / `Element`, `Event` constructor with `preventDefault` + `stopImmediatePropagation`, and `DOMContentLoaded` / `load` fired from the koala-browser pipeline) all landed. Dispatch is strict-target-only — bubbling / capture phases are deferred. |
| 4   — external `<script src>`         | ✓ DONE | `load_scripts` walks the DOM in tree order, fetching external scripts via the existing `koala_common::net` paths (HTTP / data: / file). All scripts execute *after* parse rather than mid-parse — true parse-blocking semantics would require parser-side hooks and are deferred. `async` / `defer` recognized but treated as synchronous. Fetch failures recorded in `parse_issues` rather than aborting. |
| 5   — testharness.js result reporting | ✓ DONE | **M2 hit.** Chunk 1 (callback bridge in `crates/koala-wpt/`), chunk 2 (testharness.js dependency stubs: `self`, `location`, `setTimeout` trailing args, `'error'` event), and chunk 3 (`koala-cli --wpt-protocol`'s new `testharness` command emitting `testharness_complete` events; Python `KoalaTestharnessPart` + `KoalaTestharnessExecutor` registered under `__wptrunner__["executor"]["testharness"]`) all landed. The pipeline wires through `koala-browser`'s new `JsHooks` trait so `koala-cli` can install `koala_wpt::install` pre-script and drain results post-settle without `koala-browser` knowing about WPT. End-to-end test in `koala-cli/tests/wpt_protocol_testharness.rs` exercises the full subprocess protocol. |

Reality-check / known limitations recorded as we hit them:

- WPT-side **hosts setup** sidesteps `/etc/hosts` via `koala-cli --hosts-file` + the plugin generating a temp file via `wpt make-hosts-file` on every subprocess start. The plugin also monkey-patches `tools.wpt.run.check_environ` to bypass the env-check for `--product=koala`. Decision 1 in this doc captured this.
- **DOCTYPE PUBLIC/SYSTEM**: implemented in `crates/koala-html/src/tokenizer/core.rs` per WHATWG § 13.2.5.55–68. `.xht` reftests no longer crash; the whole `/css/CSS2/visudet/` directory went from 1 PASS / 37 CRASH+FAIL to 7 PASS / 31 FAIL / 0 CRASH.
- **TLS**: reqwest is on `rustls-tls`, not `native-tls` (closed 8 openssl dependabot alerts).
- **dashboard/runs/** holds archived wptreport JSON; one /css/ run from `acf0045` is committed-out locally but not pushed (12MB).
- **DOM-mutation re-layout** runs after all sync scripts AND `pump_until_idle` finishes — single re-cascade per document, not per-mutation. Good enough until rendering-between-iterations matters.

---


# WPT integration spec

This document is the source of truth for getting koala to run real
[Web Platform Tests](https://web-platform-tests.org/) under the
upstream `wpt run` harness, and for the conformance dashboard that
sits on top of those results. It captures the dependency graph, the
five locked architectural decisions, the per-phase implementation
plan, and the deferred work. Future sessions should start here rather
than re-deriving anything from conversation history.

## Scope and non-goals

### In scope

- A wptrunner executor and browser plugin (`browsers/koala.py`,
  `executors/executorkoala.py`) that lets upstream WPT drive koala
  via the existing `koala-cli` binary as a subprocess. This is the
  *only* supported way to invoke koala from WPT; we do not embed a
  WPT runner inside koala itself.
- A `--wpt-protocol` mode on `koala-cli` that emits structured
  JSON-lines on stdout for wptrunner consumption (load URL, return
  screenshot path / serialized DOM / testharness JSON blob).
- Full DOM bridge from JavaScript into koala-dom: `document`,
  `window`, `JsElement`, attribute and tree mutation, selectors. Boa
  remains the JS engine; replacement is on the koala roadmap but not
  gated by WPT.
- A minimal main-thread task queue with `setTimeout` /
  `setInterval` / EventTarget plumbing sufficient for testharness.js's
  `async_test()` and `add_completion_callback()`.
- Synchronous classic-script loading via `<script src="...">`,
  including resolution against the document base URL.
- testharness.js result reporting back to wptrunner via the stdout
  JSON-lines protocol.
- An Observable Framework dashboard at `dashboard/` that consumes
  accumulated wptrunner JSON, renders top-line + per-area + per-test
  views, and ships to GitHub Pages.
- Conformance tracking for *all* of WPT, not just `css/`, `html/`,
  and `dom/`. We accept that areas without engine support will sit
  near 0% indefinitely.

### Explicitly not in scope

- **A WebDriver / BiDi implementation.** Chrome and Firefox expose
  WebDriver because they ship browser binaries that need to be
  driven from arbitrary clients. Koala has a single in-tree client
  (`koala-cli`), so the custom executor path is strictly less work.
  Revisit only if a non-WPT consumer needs to drive koala remotely.
- **ES module scripts** (`<script type="module">`). Classic scripts
  cover the testharness.js bootstrap and the vast majority of WPT
  tests; modules are a Phase-6+ concern.
- **`async` / `defer` script ordering nuance.** Phase 4 ships
  parse-blocking classic scripts only. Real `async` / `defer`
  semantics arrive when a test failure surfaces a need.
- **Workers, ServiceWorker, SharedWorker.** Each is a multi-week
  subsystem in its own right and would only move the dial on a
  handful of WPT areas.
- **`fetch()`, XMLHttpRequest, EventSource, WebSocket.** Most
  testharness tests do not need them. If a target area depends on
  one (e.g. `fetch/` itself), that area is deferred until the
  network stack lands.
- **HTTPS, multi-origin, CORS.** Phase 1 ships against
  `wpt serve --no-https` on a single origin. Multi-origin tests
  (`http://www1.web-platform.test/`, `https://...`) are deferred.
- **Touch, pointer, keyboard event synthesis.** UI events arrive
  with the event-loop work; WebDriver-style synthetic input is
  deferred.
- **Replacing Boa.** A hand-rolled JS engine is on the koala
  roadmap, but WPT integration is *not* gated by it. The DOM
  bridge in Phase 2 is written against Boa's `NativeObject` API
  and will need a rewrite when the hand-rolled engine lands. That
  rewrite is acknowledged and deferred.
- **Upstream contributions.** We consume WPT; we do not author
  tests upstream during this integration. Bidirectional sync (the
  `wpt-import` pattern Chromium / Firefox use) is explicitly out
  of scope; we use a git submodule pinned to a known-good commit
  (see Decision 2).
- **Multi-product comparison on the dashboard.** wpt.fyi-style
  comparisons against Chrome / Firefox / WebKit pass rates are
  deferred. Single-product dashboard only.

## Background

### Why WPT

Koala's spec-driven correctness philosophy needs an external
yardstick. Internal unit and snapshot tests prove the code does
what we *think* the spec says; WPT proves the code does what the
spec *actually* says, as agreed by every browser vendor. It is the
only conformance suite that targets the full web platform surface
area, and every major engine reports against it.

Concretely, WPT gives us:

- A pass-rate number that can be tracked over time and compared
  against other engines.
- Regression detection — any change that flips a test pass→fail
  is visible immediately.
- A priority signal — features with the largest WPT impact rise
  to the top of the backlog.
- A long-tail bug discovery mechanism — the parser, selector
  matcher, layout engine, and cascade have all already been
  shaken out by koala-internal tests, but WPT will surface edge
  cases none of those tests cover.

### How other browsers integrate WPT

Two patterns, with very different operational profiles:

**In-tree vendored copy with bidirectional sync.** Chromium,
Firefox, and WebKit all ship a full copy of WPT inside their
source tree (`third_party/blink/web_tests/external/wpt/`,
`testing/web-platform/tests/`,
`LayoutTests/imported/w3c/web-platform-tests/`). A bidirectional
import bot (`wpt-import`) keeps the in-tree copy in sync with
upstream and ships changes the browser team authors back as
upstream PRs. This makes sense because those teams *are* the test
authors — they need a tight in-tree workflow for both directions.

**External submodule, no upstream contributions.** Servo and
Ladybird both use a git submodule pointing at upstream WPT
(`tests/wpt/web-platform-tests/` in Servo, similar layout in
Ladybird). Tests are consumed read-only; bug fixes that surface
test issues are reported upstream as bug reports, not PRs. This
is roughly an order of magnitude less infrastructure to maintain
and matches what koala needs at this stage.

We adopt the submodule pattern. See Decision 2.

### Why a custom executor instead of WebDriver

Wptrunner is the upstream WPT runner. It speaks two protocols to
the browser under test:

1. **WebDriver / BiDi** — the standard remote-control protocol.
   Requires the browser to implement a substantial network and
   command-handling layer.
2. **A `WdspecExecutor` / `MarionetteExecutor` / `ServoExecutor`
   plugin** — a Python class that knows how to launch and drive
   a specific browser via whatever protocol that browser actually
   speaks (Marionette for Firefox in pre-Geckodriver days, a
   custom stdout protocol for Servo).

Servo's approach: implement the executor plugin in Python, drive
`servo` as a subprocess, communicate via a small custom protocol
on stdin/stdout. This costs ~500 lines of Python and ~200 lines
of Rust, vs the multi-month effort of a real WebDriver
implementation.

Koala adopts Servo's approach. See Decision 1.

## Architecture overview

### Dependency graph

```
Phase 1: wptrunner executor shim
            │
            ├──► [M1] real reftests run via `wpt run --product=koala`
            │
Phase 1.5: Observable Framework dashboard
            │
            └──► [M1+] dashboard on GitHub Pages updates from CI
                                │
Phase 2: DOM bridge (document, JsElement, window)
            │
Phase 3: event loop + timers + EventTarget
            │
Phase 4: external <script src>
            │
Phase 5: testharness.js result reporting
            │
            └──► [M2] real testharness.js tests + per-test detail in dashboard
```

Reftests do not depend on Phases 2–5. testharness.js tests depend
on all of them.

### End-to-end data flow

A single `wpt run --product=koala css/CSS2/normal-flow/` invocation:

```
wptrunner (Python)
    │
    │ 1. Reads test manifest, picks reftest `block-formatting-001.html`
    │    with ref `block-formatting-001-ref.html`
    │
    │ 2. Spawns koala-cli subprocess via browsers/koala.py
    │
    ▼
koala-cli --wpt-protocol
    │
    │ 3. Reads commands from stdin as JSON lines:
    │    {"cmd": "render", "url": "http://web-platform.test/css/CSS2/...html"}
    │
    │ 4. koala-browser loads URL, parses, lays out, paints to PNG
    │
    │ 5. Emits result on stdout as JSON lines:
    │    {"event": "rendered", "screenshot": "/tmp/koala-wpt-xxx.png"}
    │
    ▼
wptrunner
    │
    │ 6. Sends second render command for the ref
    │
    │ 7. Reads both PNGs, runs internal pixel-diff
    │
    │ 8. Emits `PASS` / `FAIL` to its results JSON
    │
    ▼
wptrunner-results.json
    │
    ▼
dashboard/ (Observable Framework)
    │
    │ 9. CI workflow runs `dashboard/build-data.py`
    │    Appends row to `data/runs.parquet`
    │    Computes deltas vs previous run for regressions
    │
    │ 10. `npm run build` produces static site
    │
    │ 11. Pushed to GitHub Pages
```

testharness.js tests follow steps 1–4 identically, then diverge:

```
    │ 5. koala-cli loads testharness.js via <script src>, runs the test
    │ 6. testharness.js calls add_completion_callback(JSON.stringify(results))
    │ 7. JsRuntime hands the blob to a koala-side bridge
    │ 8. Bridge writes the blob to stdout:
    │    {"event": "testharness", "results": [...]}
    │ 9. wptrunner reads results, marks pass/fail per assertion
```

## Five locked design decisions

Locked 2026-05-26 during the WPT planning conversation. Each
decision records the rationale and the rejected alternatives.

### 1. Executor mechanism — Servo-style subprocess + custom stdout protocol

**Decision**: implement a wptrunner plugin (`browsers/koala.py`,
`executors/executorkoala.py`) that spawns `koala-cli
--wpt-protocol` as a subprocess and communicates via a custom
JSON-lines protocol on stdin/stdout. One subprocess invocation
per test in Phase 1; daemon mode (one long-running koala-cli
serving many tests) is deferred to Phase 1.5 if pace is painful.

**Rationale**: this is the path of least resistance to real
`wpt run` results. wptrunner is designed to be extended this
way — Servo, WebKitTestRunner, and several niche engines all
ship custom executor plugins. We get the upstream test
discovery, manifest handling, reftest pixel-diffing, results
aggregation, expectation-file machinery, and chunking for
parallel runs for free.

**Rejected alternatives**:

- **Implement WebDriver in koala-browser**. Estimated multi-month
  effort. Includes an HTTP server, the full WebDriver command set,
  session state, and the BiDi protocol if we want modern features.
  Premature unless a non-WPT consumer needs remote control.
- **Embed wptrunner inside koala**. Would require pulling Python
  into the koala build and managing the WPT manifest from Rust.
  Loses the ability to share upstream infrastructure.
- **Write a wholly custom test runner**. We'd lose every wptrunner
  feature listed above and reinvent each badly. Already covered
  by the earlier "html5lib + CSS reftests" pitch, now superseded.

### 2. WPT vendoring — git submodule at `third-party/wpt/`

**Decision**: add upstream `web-platform-tests/wpt` as a git
submodule at `third-party/wpt/`, pinned to a specific commit.
Bumped manually via `git submodule update --remote
third-party/wpt && git commit`. No `wpt-import` infrastructure,
no bidirectional sync.

**Rationale**: koala is a downstream consumer. The Chromium /
Firefox / WebKit in-tree-copy-with-bot pattern only pays off
when the team is *authoring* tests, and we are not. Submodule
matches what Servo and Ladybird (the closest analogues to
koala's scale) do.

**Rejected alternatives**:

- **In-tree vendored copy with import bot** (Chromium / Firefox
  pattern). Multi-week infrastructure investment that adds value
  only when contributing tests upstream. Out of scope.
- **No vendoring; fetch WPT in CI on each run**. Loses
  reproducibility — upstream WPT changes break runs unpredictably.
  Submodule pin gives us a known-good revision.
- **Git subtree** (one historical alternative). Adds the entire
  WPT history into koala's history and makes upstream bumps
  awkward. Submodule wins on operational simplicity.

### 3. Conformance scope — full WPT, with all areas tracked

**Decision**: track conformance against *every* WPT area, not
just the ones koala has engine support for. Each area shows up
on the dashboard with its current pass rate; areas with no
engine support (e.g. `webaudio/`, `webgpu/`) will sit at or
near 0% indefinitely until those subsystems land.

**Rationale**: a pass rate that excludes "areas we don't support"
is gamed by construction. The honest number is "% of WPT we
pass," with the breakdown showing where the work is. This also
gives us a clear ROI signal — areas where a small amount of
implementation work would move many tests from FAIL to PASS
rise to the top of the backlog automatically.

**Rejected alternatives**:

- **Subset to CSS / HTML / DOM only**. Hides the long tail and
  removes the incentive structure that makes WPT useful as a
  prioritization tool.
- **Per-area opt-in**. Adds bookkeeping (a list of "enabled"
  areas) that drifts out of sync with reality.

### 4. Dashboard — Observable Framework, static site on GitHub Pages

**Decision**: build the dashboard with [Observable
Framework](https://observablehq.com/framework/), serving from
GitHub Pages. Data ingestion: a Python script in
`dashboard/build-data.py` reads accumulated wptrunner JSON
results, writes a Parquet file under `dashboard/data/`, which
the framework queries client-side via DuckDB-WASM. Charts use
Observable Plot.

Site structure:

- `index.md` — top-line totals, pass-rate sparkline, area
  heatmap.
- `areas/[area].md` — per-area drill-down (e.g. `areas/css.md`,
  `areas/dom.md`), one page per top-level WPT directory.
- `regressions.md` — tests that flipped pass→fail since the
  previous run, with diff snippets.
- `tests/[id].md` — per-test detail page (status history,
  stack trace if FAIL, pixel-diff if reftest).
- `runs.md` — list of every accumulated wptrunner run with
  metadata (commit, date, total counts).

**Rationale**: Observable Framework gives us a zero-backend
deployment, a real time-series story, modern visuals, full
per-test drill-down, and stays inside the repo. The "data as
Parquet + DuckDB-WASM in browser" pattern means the dashboard
loads fast even with 100k+ test results and several years of
history. GitHub Pages hosting means zero ops cost.

**Rejected alternatives**:

- **Grafana + SQLite**. Strong time-series story but weaker
  drill-down. Needs a server (Grafana Cloud free tier works
  but is operationally annoying for a static-content product).
- **Datasette**. Beautiful SQL-first exploration; weaker
  out-of-the-box visual polish than Observable Framework.
  Reconsider if we end up needing richer SQL ad-hoc queries
  more than time-series charts.
- **Apache Superset**. Way too heavy — Docker, Postgres,
  Redis, multi-user auth. Wrong tool for a one-product
  conformance dashboard.
- **Static HTML report committed to repo**. The original
  Phase 1 pitch. Loses interactivity, regression detection,
  and historical tracking. Rejected.

### 5. koala-std blocker — none; use `std::collections::HashMap` for now

**Decision**: the DOM bridge (Phase 2) uses
`std::collections::HashMap` for any internal maps. Swap to
`koala_std::collections::HashMap` once that lands, but do
not gate WPT work on koala-std's HashMap shipping first.

**Rationale**: the swap will be mechanical (single import
line change in a small number of files); blocking on
koala-std would push WPT integration out by months for no
end-user benefit.

**Rejected alternatives**:

- **Block until koala-std HashMap ships**. No reason to —
  the bridge interface is identical between std and koala-std.
- **Use `Vec<(K,V)>` and migrate later**. Slower lookups for
  no architectural gain.

## Phase 1 — wptrunner executor shim

**Target**: `wp run --product=koala third-party/wpt/css/CSS2/`
produces a real conformance report. Estimated effort: ~1 week
of focused work.

### 1.1 `koala-cli --wpt-protocol` mode

Add a `--wpt-protocol` flag on `koala-cli`. When set:

- Read JSON-lines from stdin. Each line is one command.
- Write JSON-lines to stdout. Each line is one event.
- Log diagnostics to stderr (never stdout — stdout is the
  protocol channel).

**Command schema (v0)**:

```
{"cmd": "render", "url": "<url>", "viewport": [w, h], "timeout_ms": N}
{"cmd": "shutdown"}
```

**Event schema (v0)**:

```
{"event": "rendered", "url": "<url>", "screenshot": "<path>"}
{"event": "load_failed", "url": "<url>", "error": "<msg>"}
{"event": "timeout", "url": "<url>"}
{"event": "ready"}
```

Subprocess-per-test in Phase 1: koala-cli starts, emits
`ready`, processes one `render` + one `shutdown`, exits.
Daemon mode (multiple `render` commands per process) is
Phase 1.5 territory.

### 1.2 HTTP fetching against `web-platform.test`

WPT serves tests from `web-platform.test` (an actual TLD WPT
controls via `/etc/hosts`-style mapping during `wpt serve`).
koala-browser's existing reqwest path needs to:

- Resolve `web-platform.test` and `*.web-platform.test`
  hostnames via the local hosts file (which `wpt serve`
  manages) or via wptrunner's `--hosts` flag.
- Allow plain HTTP — Phase 1 runs against `wpt serve
  --no-https`. HTTPS is deferred.
- Honor the `--server-port` wptrunner passes through (the
  default is `8000`).

### 1.3 wptrunner plugin

Create two Python files inside the WPT submodule's `tools/`
directory, registered as a koala "product":

- `third-party/wpt/tools/wptrunner/wptrunner/browsers/koala.py`
  — implements the `Browser` interface: `start()`, `stop()`,
  `is_alive()`. Subprocess management for `koala-cli`.
- `third-party/wpt/tools/wptrunner/wptrunner/executors/executorkoala.py`
  — implements `RefTestExecutor` (and later `TestharnessExecutor`).
  Wraps the stdin/stdout protocol.

Register the product in
`third-party/wpt/tools/wptrunner/wptrunner/browsers/__init__.py`
under the `product_list` so `--product=koala` is recognized.

Because these files live inside the WPT submodule, they need
to be either (a) patched into the submodule via a maintenance
branch, or (b) injected via wptrunner's `--config` and
`--prefs-root` flags. **Decision**: ship them outside the
submodule under `wpt-tools/wptrunner-plugins/` and use
wptrunner's plugin discovery (`--browser-impl`,
`--executor-impl` are documented for this exact case). This
keeps the WPT submodule clean and bumpable.

### 1.4 First green reftest

Pick one simple `css/CSS2/visudet/...` reftest. Get it passing
end-to-end:

1. `wpt serve --no-https` running locally.
2. `wpt run --product=koala --browser-impl=...` invocation.
3. wptrunner spawns koala-cli, requests the test URL.
4. koala-cli renders, returns the screenshot path.
5. wptrunner requests the ref URL.
6. koala-cli renders, returns the screenshot path.
7. wptrunner pixel-diffs, declares PASS.

Once one test passes, expand to the full `css/CSS2/` subtree
and accept whatever pass rate falls out.

### 1.5 CI integration

A GitHub Actions workflow at `.github/workflows/wpt.yml`:

- Triggered on push to `master` and weekly via cron.
- Clones koala + WPT submodule.
- Builds `koala-cli` in release mode.
- Runs `wpt serve` in the background.
- Runs `wpt run --product=koala --log-wptreport=results.json`.
- Uploads `results.json` as an artifact.
- Triggers the dashboard rebuild (Phase 1.5).

Initial CI runs a small subset (`css/CSS2/`) for fast feedback;
full WPT runs land on the weekly cron.

## Phase 1.5 — Observable Framework dashboard

**Target**: dashboard live on GitHub Pages, updating from CI
on every wptrunner run. Estimated effort: ~3 days.

### 1.5.1 Data ingestion

`dashboard/build-data.py`:

- Reads all `results-*.json` from `dashboard/runs/` (the
  archive of every CI run, kept in-repo or in a sidecar
  branch).
- Normalizes each run into rows: `(run_id, timestamp, commit,
  test_path, status, duration_ms, message)`.
- Writes the union to `dashboard/data/runs.parquet`.
- Computes a `regressions.parquet` showing tests that
  changed status between the latest run and the previous one.

Parquet because (a) DuckDB-WASM reads it efficiently in the
browser, (b) it compresses well for an in-repo archive, and
(c) columnar storage is the right fit for the queries the
dashboard runs.

### 1.5.2 Site structure

```
dashboard/
├── README.md
├── observablehq.config.js
├── build-data.py
├── data/
│   ├── runs.parquet
│   └── regressions.parquet
├── runs/
│   ├── 2026-05-27T01-00.json     # one per run
│   └── ...
├── src/
│   ├── index.md                   # top-line + sparklines
│   ├── areas.md                   # heatmap of all 40 areas
│   ├── regressions.md             # pass→fail since last run
│   ├── runs.md                    # list of all runs
│   ├── areas/
│   │   ├── css.md                 # area drill-down
│   │   ├── html.md
│   │   ├── dom.md
│   │   └── ... (one per top-level WPT dir)
│   └── tests/
│       └── [id].md                # per-test detail (generated)
└── package.json
```

### 1.5.3 GitHub Pages deployment

`.github/workflows/dashboard.yml`:

- Triggered after `wpt.yml` completes successfully.
- Runs `python3 dashboard/build-data.py`.
- Runs `npm install && npm run build` in `dashboard/`.
- Deploys `dashboard/dist/` to `gh-pages` branch.

## Phase 2 — DOM bridge

**Target**: JavaScript can read and mutate the DOM tree.
Estimated effort: ~2–3 weeks.

### 2.1 DomHandle threaded through JsRuntime

`JsRuntime::new()` currently takes no arguments. Change to:

```rust
pub fn new(dom: Rc<RefCell<DomTree>>) -> Self
```

Update construction sites: `koala-browser::lib.rs:191`, any
examples, any tests.

### 2.2 JsElement (Boa NativeObject)

A `JsElement` wraps `NodeId + Rc<RefCell<DomTree>>` and
implements Boa's `NativeObject` trait so it participates in
Boa's GC. Methods to expose (read-only `r`, read-write `rw`):

```
.tagName       r
.id            rw   (mirrors `id` attribute)
.className     rw   (mirrors `class` attribute)
.textContent   rw
.innerHTML     r    (write deferred to Phase 3+)
.parentNode    r
.firstChild    r
.lastChild     r
.nextSibling   r
.previousSibling r
.children      r    (HTMLCollection — live)
.childNodes    r    (NodeList — live)

.getAttribute(name)            r
.setAttribute(name, value)     rw
.removeAttribute(name)         rw
.hasAttribute(name)            r
```

### 2.3 document global

Methods on the `document` object:

```
.getElementById(id)                → JsElement | null
.getElementsByTagName(name)        → HTMLCollection
.getElementsByClassName(names)     → HTMLCollection
.querySelector(selectors)          → JsElement | null
.querySelectorAll(selectors)       → NodeList

.body           → JsElement | null
.head           → JsElement | null
.documentElement → JsElement | null
.title          → string

.createElement(tagName)            → JsElement
.createTextNode(data)              → JsText
```

`querySelector` / `querySelectorAll` reuse the selector
matcher from `crates/koala-css/src/selector/`.

### 2.4 window global

`window` is self-referential (`window === window.window`),
forwards property access to `document` for the bits
testharness.js touches, and exposes the few
window-as-globalThis properties (`window.console`,
`window.location` stubbed).

### 2.5 Mutation

`appendChild`, `insertBefore`, `removeChild`, `replaceChild`.
Each triggers a full re-layout via the existing
`koala-browser` document pipeline. Optimization (incremental
layout invalidation) is a separate workstream and not gated
by WPT.

## Phase 3 — event loop, timers, EventTarget

**Target**: testharness.js's `async_test()` works; timer
callbacks fire. Estimated effort: ~1–2 weeks.

### 3.1 Main-thread task queue

A simple priority queue on the main thread, ordered by
firing time. The koala-browser document pipeline becomes:

```
1. parse document
2. layout pass 1
3. run pending scripts (sync)
4. dispatch DOMContentLoaded
5. event loop:
   - drain task queue
   - if any DOM mutated: layout pass N
   - if timer fired: dispatch
   - if no work and timeout: exit
6. dispatch load
```

This is a simplification of HTML § 8.1.6.3 (Processing
model), but it covers the cases testharness.js exercises.

### 3.2 Timers

`setTimeout(callback, delay_ms, ...args)`,
`setInterval(callback, interval_ms, ...args)`,
`clearTimeout(id)`, `clearInterval(id)`. Backed by
`std::time::Instant`. Returns a `u32` handle.

### 3.3 EventTarget

A minimal `EventTarget` mixin on `Window` and `Element`:

```
.addEventListener(type, listener, options?)
.removeEventListener(type, listener)
.dispatchEvent(event)        → bool
```

Built-in events that fire: `DOMContentLoaded`, `load`,
`error` (on script load failure). User-synthesized events
via `dispatchEvent` work; native UI event synthesis is
deferred.

## Phase 4 — external `<script src>`

**Target**: `<script src="/resources/testharness.js">` loads
and executes the real upstream testharness. Estimated
effort: ~3–5 days.

### 4.1 DOM walk for scripts

During HTML parsing, when a `<script>` element with a `src`
attribute is closed, fetch and execute the script
synchronously before continuing to parse. This is the
"classic script, parse-blocking" path from § 4.12.1.1.

`async` / `defer` attributes are recognized but treated as
synchronous for now. Real async/defer ordering is deferred.

### 4.2 Fetching

Reuse `koala-browser`'s existing reqwest path. Resolve
relative URLs against the document base URL.

### 4.3 Execution

The fetched bytes are decoded as UTF-8 and passed to
`JsRuntime::execute()`. Errors are logged via the existing
parse-error infrastructure; they do not abort document
load.

## Phase 5 — testharness.js result reporting

**Target**: `wpt run --product=koala dom/nodes/` runs
testharness.js tests and reports real pass/fail counts.
Estimated effort: ~1 week.

### 5.1 testharness.js callback bridge

testharness.js exposes
`add_completion_callback(callback)`; when all tests finish,
the callback is invoked with the results array. A
koala-side native function bridges this into the
`--wpt-protocol` stdout channel:

```
{"event": "testharness", "results": [
  {"name": "...", "status": 0, "message": null},
  {"name": "...", "status": 1, "message": "AssertionError: ..."},
  ...
]}
```

Status codes match testharness.js's `Test.PASS = 0`,
`FAIL = 1`, `TIMEOUT = 2`, `NOTRUN = 3`, `PRECONDITION_FAILED = 4`.

### 5.2 Stubs for testharness.js dependencies

testharness.js touches several APIs that don't strictly
need to work for the harness to function:

- `window.postMessage` — stub as a no-op for single-origin.
- `location.search` — stub returning `""`.
- `XMLHttpRequest` — most tests don't need it. If a target
  area does, the test fails noisily and we add it later.

### 5.3 executor_koala.py extension

The wptrunner executor learns to read `{"event":
"testharness", ...}` events from the koala-cli stdout
protocol and map them onto wptrunner's internal
`TestharnessResult` type. The reftest executor remains
unchanged.

## Milestones

| ID  | Description                                           | Gates on phases |
|-----|-------------------------------------------------------|-----------------|
| M1  | `wpt run --product=koala css/CSS2/` produces a JSON   | Phase 1         |
|     | conformance report and the dashboard renders it       | + Phase 1.5     |
| M2  | `wpt run --product=koala dom/nodes/` runs testharness | Phases 1–5      |
|     | tests with real pass/fail counts in the dashboard     | **HIT** 2026-05 |

## Risks and mitigations

### R1 — Boa performance under WPT scale

Boa is significantly slower than V8 / SpiderMonkey / JSC. A
full WPT run with thousands of testharness.js invocations
may take many hours.

*Mitigation*: subprocess mode caps any individual
slowdown to that one test. Daemon mode (Phase 1.5) helps
by amortizing startup. If pace becomes unworkable, the
hand-rolled JS engine on the koala roadmap moves up.

### R2 — Parser bugs surfaced by testharness.js

testharness.js exercises HTML parsing edge cases that
koala-internal tests don't hit. Expect a backlog of small
parser fixes once Phase 4 lands.

*Mitigation*: budget time for parser triage as part of
Phase 4 / Phase 5 close-out. Each surfaced parser bug
gets a fix in `koala-html` and a regression test there.

### R3 — wptrunner Python version churn

wptrunner is actively developed; submodule bumps may
silently break the executor plugin.

*Mitigation*: pin to a specific WPT commit (Decision 2).
Bump explicitly. Re-run the M1 smoke set after every bump.

### R4 — `web-platform.test` host resolution

DNS / `/etc/hosts` mismatches can make WPT URLs
unreachable from koala-cli even when `wpt serve` is
running locally.

*Mitigation*: wptrunner can pass an explicit `--host` /
`--ports` config. Phase 1 uses that path. Documentation
in `dashboard/README.md` explains the local-run setup.

### R5 — Test flakiness in the dashboard

testharness.js tests with timers or async work may flake
under load.

*Mitigation*: dashboard tracks per-test stability — a
test that flips status often shows up as "flaky" rather
than "regressing." Implement in Phase 1.5.

## Open questions

None at the time of writing — Decisions 1–5 above resolve
the open questions raised during planning. New questions
discovered during implementation should be appended below
with a `### Q.<n>` heading and resolved before moving
between phases.
