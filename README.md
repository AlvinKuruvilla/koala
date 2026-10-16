<p align="center">
  <img src="koala-ui/macos/icon.svg" width="128" alt="Koala icon">
</p>

<h1 align="center">Koala</h1>

<p align="center">
  A browser engine AI agents can rewind, written from scratch in Rust.
</p>

<p align="center">
  <a href="https://github.com/AlvinKuruvilla/koala/actions/workflows/miri.yml"><img src="https://github.com/AlvinKuruvilla/koala/actions/workflows/miri.yml/badge.svg" alt="miri"></a>
  <a href="https://github.com/AlvinKuruvilla/koala/actions/workflows/python.yml"><img src="https://github.com/AlvinKuruvilla/koala/actions/workflows/python.yml/badge.svg" alt="python"></a>
  <img src="https://img.shields.io/badge/rust-2024_edition-orange?logo=rust" alt="Rust 2024 edition">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
</p>

Web agents that search, learn, or are evaluated on websites need to try an
action and go back. No browser can copy a running page, so today they
relaunch the browser and replay every step from the start, or clone a whole
virtual machine. Koala is being built so that a page's entire client state
(DOM, styles, layout, and JavaScript heap) forks in milliseconds: try an
action, look at the result, keep it or throw it away, and run many branches
side by side, each rendering the same way every time.

The fork interface does not exist yet. A prototype that forks a process
holding a loaded Wikipedia page and a live JavaScript heap takes 1 to 11 ms,
against 170 ms to load the page again. Today Koala parses, styles, lays out,
and paints pages without WebKit, Blink, or Gecko, and renders them to PNG
from the command line or in the desktop browser. JavaScript runs on
[Boa](https://github.com/boa-dev/boa).

## Who needs this

- **Agents that plan by searching.** Instead of committing to its first
  guess, an agent can try several actions from the same page and keep the
  best. Today going back means relaunching the browser and replaying every
  step: one study measured 33 minutes per task, and in 40% of tasks the
  replayed page no longer matched the original
  ([ExACT](https://arxiv.org/abs/2410.02052)).
- **Training agents with reinforcement learning.** Training runs many
  attempts from the same starting state. When resetting the browser is
  slow, the GPUs wait for it; cheap branching keeps them busy.
- **Evaluating agents.** Every run starts from an identical page, and a
  failed run can be replayed exactly to see where it went wrong.

Rewinding covers what the browser holds: the page, its scripts, and their
state. It cannot take back what the agent already sent to a server, such
as an order placed or an email sent, so it fits exploring, reading, and
self-hosted test sites, not undoing purchases on live ones.

## Contents

- [Who needs this](#who-needs-this)
- [Roadmap](#roadmap)
- [Status](#status)
- [Getting started](#getting-started)
- [Architecture](#architecture)
- [Development](#development)
- [License](#license)

## Roadmap

1. Render the server-rendered apps that web-agent benchmarks run: WebShop,
   and WebArena's store, forum, classifieds, and wiki.
2. Interaction: scripts that keep running after load, form input and
   submission, and navigation.
3. Forking: one page per single-threaded process, forked on request, behind
   `reset`, `step`, and `fork`.
4. A measured result: a tree-search agent on WebShop, timed with forking
   against replaying its actions.

Agents will drive Koala through the Chrome DevTools Protocol, so tools built
for Chrome, such as `chrome-devtools-mcp`, work unchanged; forking is the one
thing Koala adds.

## Status

| Area | Supported |
|---|---|
| HTML | WHATWG tokenizer and tree builder, including all 2,231 named character references, tables, and forms |
| CSS | Type, class, ID, attribute, and combinator selectors; cascade; custom properties; shorthands |
| Layout | Block, inline, inline-block, flexbox, grid, tables, floats, margin collapsing, replaced elements |
| Positioning | `static`, `relative`, `absolute`, `fixed`, `sticky`; stacking contexts; `opacity`; overflow clipping |
| Painting | Text with weight, style, and decoration; backgrounds; borders with radius; box shadows; images |
| JavaScript | DOM bindings (`querySelector`, `addEventListener`, element and text APIs), timers, WPT `testharness.js` |
| Network | HTTP(S), `file:`, and `data:` URLs; per-destination `Accept` headers; the server's page on HTTP errors |

Not supported: media queries, pseudo-elements, `z-index`, transforms,
animations, and font fallback.

## Getting started

Koala needs a Rust toolchain with 2024 edition support. The browser's UI is
[Slint](https://slint.dev), a Rust crate, so Cargo builds everything. The
recipes below use [`just`](https://github.com/casey/just).

Open the browser:

```bash
just gui
```

This builds `Koala.app` and runs it under `lldb`, so a crash prints a
backtrace. The bundle is macOS-only; elsewhere, run
`cargo run --bin koala-ui`.

Render a page from the command line:

```bash
cargo run --bin koala -- -S out.png https://example.com   # screenshot
cargo run --bin koala -- https://example.com              # DOM tree
cargo run --bin koala -- --layout https://example.com     # layout tree
cargo run --bin koala -- --html '<h1>Hello</h1>' --layout # inline HTML
```

The default viewport is 1280×720; `--width` and `--height` change it.

## Architecture

A page moves through the engine top to bottom. When JavaScript mutates the
DOM, the page goes back through the cascade and everything after it.

```text
bytes ─► HTML tokenizer ─► tree builder ─► DOM ◄──── JavaScript (Boa)
                                            │
CSS ───► CSS tokenizer ──► parser ──► cascade ─► computed styles
                                            │
                                   layout ─► box tree
                                            │
                                    paint ─► display list
                                            │
                                   raster ─► pixels
```

```text
koala/
├── crates/
│   ├── koala-common/   URLs, images, warnings, allocation counting
│   ├── koala-fetch/    WHATWG Fetch: requests, destinations, senders
│   ├── koala-dom/      arena-allocated DOM tree
│   ├── koala-html/     HTML tokenizer and tree builder
│   ├── koala-css/      CSS parser, cascade, layout, paint
│   ├── koala-js/       Boa runtime and DOM bindings
│   ├── koala-browser/  document pipeline and software rasterizer
│   ├── koala-wpt/      web-platform-tests glue
│   ├── koala-debug/    diagnostic probes for Boa and memory use
│   └── koala-std/      no_std collections written for koala
├── koala-cli/          the `koala` binary
├── koala-ui/           the desktop browser (Slint)
├── tools/koala-lab/    performance comparisons between builds
└── res/                fonts, fixtures, and test pages
```

[`CLAUDE.md`](CLAUDE.md) describes the spec-commenting conventions every
engine crate follows.

## Development

```bash
cargo test                  # all Rust tests
cargo test -p koala-css     # one crate
cargo clippy --workspace
just py-check               # ruff, mypy, and pytest for the Python tools
```

### Measuring performance

`just lab compare` builds two commits, loads each page of a recorded corpus
in fresh processes, and reports which stages changed and by how much. With no
arguments it compares the working tree against where the branch left
`master`:

```bash
just lab corpus record        # record the corpus pages once
just lab compare              # this branch vs. master
just lab compare HEAD~3 HEAD  # any two commits
```

Recorded pages replay without the network, so both builds see the same bytes.
A change counts only when it passes a Mann-Whitney test across seven
processes per build, with a Holm correction across stages.

### Conformance

```bash
just wpt-setup                      # once: create the wptrunner venv
just wpt /css/CSS2/visudet/         # run a WPT directory
just dashboard                      # browse recorded runs
```

### Debugging layout

```bash
cargo run --bin koala --features layout-trace -- -S out.png <url> 2> trace.txt
```

The trace tags each line by subsystem: `[FLEX]`, `[INLINE]`, `[BLOCK STEP`,
`[MEASURE]`, `[LAYOUT DEPTH]`.

## License

[MIT](LICENSE)
