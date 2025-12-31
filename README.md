# Koala Browser

A minimal web browser with a Rust parsing engine and Swift UI.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![Swift](https://img.shields.io/badge/Swift-FA7343?style=flat&logo=swift&logoColor=white)
![macOS](https://img.shields.io/badge/macOS-13%2B-blue)
![License](https://img.shields.io/badge/License-MIT-green)

## Overview

Koala is an experimental browser that combines:
- **Rust** - HTML tokenizer, parser, and CSS tokenizer
- **Swift/SwiftUI** - Native macOS user interface
- **C FFI** - Bridge between Rust and Swift

## Table of Contents

- [Building](#building)
- [Usage](#usage)
- [Project Structure](#project-structure)
- [Architecture](#architecture)

## Building

### Prerequisites

- Rust 1.70+
- Swift 5.9+
- macOS 13+

### Build and Run

```bash
cd KoalaBrowser
./run.sh
```

This builds the Rust static library, compiles the Swift app, and launches the browser.

### CLI Tools

```bash
# Parse HTML and print tokens
cargo run --bin koala_cli -- path/to/file.html

# Parse HTML and output formatted JSON
cargo run --bin json_dump -- path/to/file.html
```

## Usage

Enter an absolute file path in the URL bar to load an HTML file:

```
/path/to/your/file.html
```

A test file is included at `res/test.html`.

## Project Structure

```
koala/
├── src/
│   ├── lib.rs              # Library root
│   ├── ffi/                # C FFI for Swift interop
│   ├── lib_html/           # HTML tokenizer and parser
│   ├── lib_css/            # CSS tokenizer and parser
│   └── lib_dom/            # DOM node types
├── bin/
│   ├── koala_cli.rs        # CLI for testing parser
│   └── json_dump.rs        # JSON output tool
├── KoalaBrowser/
│   ├── Package.swift
│   ├── run.sh
│   └── Sources/
│       ├── KoalaBrowser/   # SwiftUI app
│       └── KoalaCore/      # Swift FFI wrapper
└── res/                    # Test files
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Swift UI (SwiftUI)                   │
│                   BrowserView, NodeView                 │
└─────────────────────────┬───────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────┐
│                  KoalaCore (Swift)                      │
│              KoalaParser, DOMNode wrapper               │
└─────────────────────────┬───────────────────────────────┘
                          │ C FFI
┌─────────────────────────▼───────────────────────────────┐
│                    libkoala (Rust)                      │
│         HTMLTokenizer → HTMLParser → DOM Tree           │
└─────────────────────────────────────────────────────────┘
```

## License

MIT
