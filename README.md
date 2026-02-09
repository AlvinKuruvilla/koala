# Koala

A fast, lightweight HTML-to-image renderer in Rust.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

## What is Koala?

Koala renders HTML and CSS to images without any browser dependency. It includes a from-scratch HTML parser, CSS engine, and layout engine — no WebKit, Blink, or Gecko. Give it HTML (a file, URL, or string) and get back a PNG.

## Use Cases

- **OG images** — Generate social preview cards from HTML templates
- **Email previews** — Render HTML emails to image thumbnails
- **Visual regression testing** — Screenshot pages for pixel-level diffing
- **Serverless rendering** — No headless browser binary needed

## Quick Start

```bash
# Render a URL to PNG
koala -S output.png https://example.com

# Render a local HTML file
koala -S output.png ./page.html

# Render inline HTML
koala --html '<h1 style="color: red">Hello</h1>' -S output.png

# Custom viewport size
koala -S output.png --width 1920 --height 1080 https://example.com
```

## Installation

```bash
# From source
cargo install --path koala-cli
```

## CSS Support

| Supported | Not Yet |
|-----------|---------|
| Block layout (CSS 2.1 § 9-10) | Media queries |
| Inline formatting context | Pseudo-elements (::before, ::after) |
| Flexbox (CSS Flexbox Level 1) | Float layout |
| Grid (CSS Grid Level 1) | Absolute/fixed positioning |
| Table layout (CSS 2.1 § 17) | CSS animations/transitions |
| Inline-block | @font-face |
| Margin collapsing (§ 8.3.1) | |
| Colors (hex, named, rgb) | |
| Font weight/style/size | |
| Text decoration | |
| Border radius | |
| Box shadow | |
| Images (`<img>`) | |
| External stylesheets | |
| CSS variables (custom properties) | |
| Overflow clipping | |
| Viewport units (vw, vh) | |

## Why not Puppeteer / wkhtmltoimage?

| | Koala | Puppeteer | wkhtmltoimage |
|---|---|---|---|
| Binary size | ~10 MB | ~300 MB (Chromium) | ~50 MB (Qt WebKit) |
| Cold start | Instant | 1-3s (browser launch) | 0.5-1s |
| Runtime deps | None | Chrome/Chromium | Qt libraries |
| Serverless-friendly | Yes | Requires layers/containers | Requires system libs |

Koala trades full web compatibility for simplicity and speed. If you need JavaScript execution or pixel-perfect Chrome rendering, use Puppeteer. If you need fast, predictable HTML-to-image conversion with known CSS, use Koala.

## Architecture

```
HTML string ──→ Tokenizer ──→ Parser ──→ DOM Tree
                                            │
CSS string  ──→ Tokenizer ──→ Parser ──→ Stylesheet
                                            │
                              Cascade ──→ Computed Styles
                                            │
                              Layout  ──→ Box Tree + Dimensions
                                            │
                              Paint   ──→ Display List
                                            │
                              Render  ──→ PNG
```

## Project Structure

```
koala/
├── crates/
│   ├── koala-common/     # Shared utilities (warnings, networking, URL resolution)
│   ├── koala-dom/        # Arena-based DOM tree
│   ├── koala-html/       # WHATWG-compliant HTML tokenizer and parser
│   ├── koala-css/        # CSS tokenizer, parser, layout engine, and cascade
│   ├── koala-js/         # JavaScript engine integration (Boa)
│   └── koala-browser/    # Document loading and rendering pipeline
├── koala-cli/            # Primary binary: HTML-to-image renderer
├── koala-gui/            # Development GUI (egui-based renderer inspector)
└── res/                  # Test HTML files and fonts
```

## Development

```bash
# Build all crates
cargo build

# Run all tests
cargo test

# Render HTML to image (primary binary)
cargo run --bin koala -- -S output.png https://example.com

# Run the development GUI
cargo run --bin koala-gui

# Lint
cargo clippy --workspace
```

The development GUI (`koala-gui`) includes a debug panel (F12) showing DOM tree, tokens, CSS, computed styles, and HTML source.

## License

[MIT](LICENSE)
