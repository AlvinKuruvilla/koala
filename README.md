# Koala Browser

A from-scratch web browser implementation in Rust, built for learning and understanding.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![egui](https://img.shields.io/badge/egui-0.29-blue)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

## Overview

Koala is a learning-oriented browser that implements web standards from first principles:

- HTML tokenizer and parser ([WHATWG Living Standard](https://html.spec.whatwg.org/))
- CSS tokenizer, parser, and selector matching ([CSS Syntax Level 3](https://www.w3.org/TR/css-syntax-3/), [Selectors Level 4](https://www.w3.org/TR/selectors-4/))
- Style cascade and computed styles ([CSS Cascading Level 4](https://www.w3.org/TR/css-cascade-4/))
- egui-based cross-platform GUI

## Building

```bash
# Build
cargo build

# Run GUI browser
cargo run --bin koala

# Run CLI tool
cargo run --bin koala-cli -- res/simple.html

# Run tests
cargo test
```

## Project Structure

```
koala/
├── crates/
│   ├── koala-dom/        # Arena-based DOM tree
│   ├── koala-html/       # HTML tokenizer and parser
│   ├── koala-css/        # CSS tokenizer, parser, selector, cascade
│   └── koala-core/       # Shared browser API
├── koala-cli/            # CLI tool
├── koala-gui/            # GUI browser
└── res/                  # Test files
```

## License

MIT
