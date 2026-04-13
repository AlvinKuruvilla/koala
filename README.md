# Koala

An experimental Rust browser engine, built from scratch — no WebKit, Blink, or Gecko. Koala is a research project exploring what a browser looks like when the primary consumer is an LLM agent, and the human UI is the viewport onto what the agent sees.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

## The experiment

Every "AI that browses the web" product today runs Chromium in a box and pixel-scrapes it. The browser was designed for humans; agents kludge on top. It's slow, flaky, and hostile.

Koala inverts that. The engine is being built to be driven by an agent, with a Qt shell as a window onto what the agent is doing.

The long-term bets:

- **The render tree as a typed API.** Agents consume structured layout — boxes, semantic roles, reading order — not screenshots. *"Click the primary action in the checkout form"* resolves through the layout tree, not pixel coordinates.
- **Every page as an MCP-shaped tool.** The browser extracts a page's action surface (forms, links, buttons, regions) and hands it to an LLM as typed tools. Every site becomes a tool server for free.
- **Deterministic, replayable sessions.** Agent runs are reproducible scripts of `(URL, DOM state, action, resulting render tree)`. Diffable, shareable, debuggable.
- **Spec-faithful by construction.** The engine is a synthesized implementation of the WHATWG HTML standard and CSS 2.1+, with section numbers and spec text quoted inline next to the code. Correctness over velocity.

**None of the agent API exists yet.** Koala today is an HTML/CSS engine, a CLI, and a Qt browser shell. This README describes the direction, not the current reality.

## What works today

- **HTML parsing** — WHATWG tokenizer and tree builder: DOCTYPE, tags, attributes, comments, RCDATA/RAWTEXT, all 2,231 named character references, tables, forms.
- **CSS engine** — tokenizer, parser, selector matching (type/class/ID/combinator/attribute), cascade, computed styles, custom properties, shorthand expansion.
- **Layout** — block (CSS 2.1 § 9–10), inline formatting context, flexbox, grid, tables, inline-block, margin collapsing, replaced elements (`<img>`), overflow clipping.
- **Rendering** — software rasterizer producing PNG/JPG/BMP: text with font weight/style/decoration, backgrounds, borders with radius, box shadows, images.
- **`koala-qt`** — Qt6 browser shell over a `cxx` bridge into the engine. Tabs, location bar, landing page.
- **`koala` CLI** — parse HTML, dump DOM and layout trees, render any URL to a PNG.

**Notable gaps:** no JavaScript execution wired through the DOM yet (Boa is integrated but idle), no media queries, no pseudo-elements, no floats, no absolute positioning, no animations, no agent API. Expect rough edges.

## Try it

```bash
# Render a URL to a PNG
cargo run --bin koala -- -S out.png https://example.com

# Dump the DOM tree
cargo run --bin koala -- https://example.com

# Dump the computed layout tree (1280x720 viewport)
cargo run --bin koala -- --layout https://example.com

# Parse inline HTML
cargo run --bin koala -- --html '<h1>Hello</h1>' --layout

# Launch the Qt browser shell (requires Qt6 — `brew install qt` on macOS)
cargo run --bin koala-qt
```

## Architecture

```
HTML ──→ Tokenizer ──→ Parser ──→ DOM Tree
                                      │
CSS  ──→ Tokenizer ──→ Parser ──→ Stylesheet
                                      │
                        Cascade ──→ Computed Styles
                                      │
                        Layout  ──→ Box Tree
                                      │
                        Paint   ──→ Display List
                                      │
                        Raster  ──→ Pixels
```

```
koala/
├── crates/
│   ├── koala-common/     # Shared utilities (URL, fetching, images)
│   ├── koala-dom/        # Arena-based DOM tree
│   ├── koala-html/       # WHATWG tokenizer and parser
│   ├── koala-css/        # CSS parser, cascade, layout, paint
│   ├── koala-js/         # Boa-backed JavaScript runtime
│   └── koala-browser/    # Document pipeline + software rasterizer
├── koala-cli/            # CLI: parse, inspect, screenshot
├── koala-qt/             # Qt6 browser shell (cxx bridge)
└── res/                  # Test HTML and fonts
```

Every algorithm is implemented alongside its WHATWG or CSS section number, with the spec text quoted inline. See [`CLAUDE.md`](CLAUDE.md) for the project's spec-commenting conventions.

## Development

```bash
cargo build                                  # Build workspace
cargo test                                   # Full test suite
cargo test -p koala-css                      # Single crate
cargo clippy --workspace
cargo fmt --check

# Verbose layout tracing (block/inline/flex/grid/measure)
cargo run --bin koala --features layout-trace -- -S out.png <url>
```

## License

[MIT](LICENSE)
