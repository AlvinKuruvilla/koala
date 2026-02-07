# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build all crates
cargo build
cargo build --release

# Build specific crate
cargo build -p koala-html
cargo build -p koala-css

# Run tests
cargo test                          # Run all tests
cargo test -p koala-html            # Test specific crate
cargo test test_name                # Run a specific test

# Run the GUI browser
cargo run --bin koala

# Run the CLI tool
cargo run --bin koala-cli -- <file.html>
cargo run --bin koala-cli -- --html '<h1>Test</h1>'

# Lint
cargo clippy --workspace
cargo fmt --check
```

## GUI Debugging

The GUI (`koala`) has built-in debugging features:
- **F12**: Toggle debug panel showing DOM tree, tokens, CSS, computed styles, and source
- **Terminal logging**: All state changes print to stdout with `[Koala GUI]` prefix
- Debug panel tabs: DOM | Tokens | CSS | Styles | Source

## Project Overview

A from-scratch web browser implementation in Rust, built for learning and understanding.

## Philosophy

### The Spec is the Bible

This project follows the [WHATWG HTML Living Standard](https://html.spec.whatwg.org/) religiously. The code should read like a synthesized spec-driven implementation.

#### Spec Commenting Requirements

1. **Include section numbers** — Use `§13.2.5.1` format for traceability
2. **Quote the spec exactly** — Copy the exact language from the spec as comments
3. **Preserve the spec's structure** — If the spec uses numbered steps, use numbered comments. If it uses bullets, use bullets. Match nesting levels.
4. **Add interpretive comments where helpful** — When something requires clarification, add your own commentary clearly marked as such (e.g., "NOTE:" or "Implementation note:")
5. **Document unimplemented branches too** — Even `todo!()` branches should have full spec text explaining what they *would* do

#### Example of Good Code

```rust
/// [§ 13.2.5.8 Tag name state](https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state)
fn handle_tag_name_state(&mut self) {
    match self.current_input_character {
        // "U+0009 CHARACTER TABULATION (tab)"
        // "U+000A LINE FEED (LF)"
        // "U+000C FORM FEED (FF)"
        // "U+0020 SPACE"
        // "Switch to the before attribute name state."
        Some(c) if Self::is_whitespace_char(c) => {
            self.switch_to(TokenizerState::BeforeAttributeName);
        }
        // "U+002F SOLIDUS (/)"
        // "Switch to the self-closing start tag state."
        Some('/') => {
            self.switch_to(TokenizerState::SelfClosingStartTag);
        }
        // "U+003E GREATER-THAN SIGN (>)"
        // "Switch to the data state. Emit the current tag token."
        Some('>') => {
            self.switch_to(TokenizerState::Data);
            self.emit_token();
        }
        // ...
    }
}
```

#### Step-Level Algorithm Comments

For multi-step algorithms (like CSS layout), use numbered STEP comments that map directly to the spec. This makes it easy to:
1. Verify correctness against the spec
2. Understand what each section of code is doing
3. Know where to add code when implementing

```rust
fn calculate_block_width(&mut self, containing_block: Rect) {
    // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    //
    // "The following constraints must hold among the used values..."

    // STEP 1: Read the style values.
    // Border and padding cannot be 'auto', only margins and width can.
    let padding_left = self.padding.left;
    // ...

    // STEP 2: Handle over-constrained case.
    // [§ 10.3.3](https://www.w3.org/TR/CSS2/visudet.html#blockwidth)
    //
    // "If 'width' is not 'auto' and the total is larger than the width
    // of the containing block, then any 'auto' values for margins are
    // treated as zero."
    if !width.is_auto() {
        // ...
    }

    // STEP 3: Apply the constraint rules.
    // RULE A: "If 'width' is set to 'auto'..."
    if width.is_auto() {
        // ...
    }
    // RULE B: "If both margins are 'auto'..."
    else if margin_left.is_auto() && margin_right.is_auto() {
        // ...
    }

    // STEP 4: Store the used values.
    self.dimensions.content.width = used_width;
}
```

When implementing new algorithms:
1. First write out all the STEP comments with spec quotes (no code yet)
2. Then implement each step one at a time
3. Keep `todo!()` at the end until all steps are implemented

#### What NOT to Do

- Don't use banner-style section dividers like `// --------`
- Don't paraphrase the spec when you can quote it
- Don't skip documenting branches just because they're not implemented yet

### Earn the Understanding

This is a learning project. The goal is depth over speed. When implementing:

- Understand *why* the spec says what it says (often backwards compatibility)
- Implement things yourself before reaching for libraries (except where unreasonable, like JS engines)
- Use `todo!()` liberally for unimplemented paths — it's better than wrong behavior

### Incremental Progress over Completeness

Focus on what you're working on *now*. Don't try to implement everything in one shot.

- **`todo!()` is your friend** — Unimplemented states should crash loudly, not silently misbehave. When you hit one, you know exactly what needs work.
- **Document what's missing** — Leave TODOs in code, update this file's status section, note limitations clearly.
- **Parse errors ≠ crashes** — Per spec, HTML parsing is permissive. Log parse errors and continue. But hitting an *unimplemented* code path should panic.
- **Mechanical refactors: do completely** — If you're renaming a function or changing a pattern, update all call sites. Half-migrated code is confusing.
- **Feature implementation: do incrementally** — Implement what you need for the current test case. Add more states/handlers as you encounter them.

### Architecture

```
koala/
├── crates/
│   ├── koala-common/     # Shared utilities (warnings, error types)
│   ├── koala-dom/        # Arena-based DOM tree with parent/sibling links
│   ├── koala-html/       # HTML tokenizer and parser (WHATWG spec)
│   ├── koala-css/        # CSS tokenizer, parser, selector, cascade
│   └── koala-browser/    # High-level browser API
├── koala-cli/            # CLI tool for parsing/debugging
├── koala-gui/            # egui-based GUI browser
└── res/                  # Test HTML files
```

**Crate Dependencies:**
```
koala-common       (no deps - shared utilities)
koala-dom          (no deps)
    ↑
koala-html         (depends on koala-dom)
koala-css          (depends on koala-common, koala-dom)
    ↑
koala-browser      (depends on koala-common, koala-dom, koala-html, koala-css)
    ↑
koala-cli/koala-gui (depends on koala-common, koala-browser, etc.)
```

**Data Flow:**
```
HTML String → HTMLTokenizer → Tokens → HTMLParser → DomTree
                                                        ↓
CSS String  → CSSTokenizer  → Tokens → CSSParser  → Stylesheet
                                                        ↓
                                      css_cascade::compute_styles()
                                                        ↓
                                              HashMap<NodeId, ComputedStyle>
```

The `DomTree` uses arena-based allocation with `NodeId` indices for O(1) traversal. Parent, child, and sibling relationships are stored as indices, avoiding borrow checker issues.

### Current Status

- **Tokenizer**: Partial — handles DOCTYPE, tags, attributes, comments, basic tag/attribute parsing, RCDATA/RAWTEXT states, character references (named + numeric, § 13.2.5.72-80, all 2,231 named entities). Missing: script data states
- **Parser**: Basic tree construction working — handles Initial, BeforeHtml, BeforeHead, InHead, AfterHead, InBody, Text, AfterBody, AfterAfterBody modes. CSS combinator matching works. Missing: table parsing, form elements, foster parenting, adoption agency algorithm
- **DOM**: Arena-based tree with Node, Element, Text, Comment types. O(1) parent/sibling traversal.
- **CSS**: Tokenizer, parser, selector matching (including combinators), cascade, and computed styles working
- **GUI**: egui-based browser with URL bar, navigation, content rendering, and debug panel (F12)
- **Rendering**: Basic — renders headings, paragraphs, text nodes with computed styles. Missing: full layout engine, styled text (bold/italic fonts)

### Dependencies

- **egui/eframe** — Cross-platform GUI framework
- **serde** — Serialization for computed styles
- **strum/strum_macros** — Enum utilities for tokenizer states
- **anyhow** — Error handling
- **Boa** (planned) — JavaScript engine

## Debugging Layout Issues

### Layout Trace Feature Flag

Verbose layout tracing is built into `koala-css` and `koala-cli` behind a cargo feature flag. Enable it to print detailed step-by-step trace of layout, flex, inline, and measure operations:

```bash
# Run CLI with layout tracing enabled
cargo run --bin koala-cli --features layout-trace -- --screenshot out.png https://example.com

# Capture trace to file for analysis
cargo run --bin koala-cli --features layout-trace -- --screenshot out.png https://example.com 2> /tmp/trace.txt

# Filter trace by subsystem
grep '\[FLEX\]' /tmp/trace.txt      # Flex layout steps
grep '\[INLINE\]' /tmp/trace.txt    # Inline formatting context
grep '\[LAYOUT DEPTH\]' /tmp/trace.txt  # Layout recursion depth + stack addresses
grep '\[BLOCK STEP' /tmp/trace.txt  # Block layout steps (anon boxes, child dispatch)
grep '\[MEASURE\]' /tmp/trace.txt   # Content size measurement
grep '\[STACK\]' /tmp/trace.txt     # Thread stack bounds (main only)
```

### Debugging Stack Overflows

Rust's built-in stack overflow handler does **not** produce a backtrace — even with `RUST_BACKTRACE=full`. For stack overflows, use `lldb` to get the actual crash backtrace:

```bash
# Write an lldb script
cat > /tmp/lldb_cmds << 'EOF'
settings set auto-confirm true
process launch -e /dev/null -- --screenshot /tmp/test.png https://www.google.com
bt 50
thread info
register read sp
quit
EOF

# Run under lldb
lldb -s /tmp/lldb_cmds target/debug/koala-cli
```

This gives the definitive backtrace at the crash point, showing exactly which function chain caused the overflow.

**Key lesson**: Stack overflow symptoms can be misleading. Trace output may show only shallow recursion while the actual overflow happens in a completely different function (e.g., `InlineLayout::add_text()` infinite recursion appeared as a layout depth issue). Always get the real backtrace before theorizing.

## Code Style

- Spec comments go above the code they describe
- Use markdown hyperlinks for spec references: `/// [§ X.Y.Z Title](URL)`
- Use `// "quoted text"` for verbatim spec language inside match arms
- Panic on impossible states (indicates tokenizer bug, not bad input)
- Parse errors are logged, not fatal — HTML parsing is intentionally permissive
