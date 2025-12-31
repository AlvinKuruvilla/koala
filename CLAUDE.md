# Koala Browser

A from-scratch web browser implementation in Rust, built for learning and understanding.

## Philosophy

### The Spec is the Bible

This project follows the [WHATWG HTML Living Standard](https://html.spec.whatwg.org/) religiously. Every piece of parsing logic should:

1. **Link to the spec** — Include the URL to the relevant spec section
2. **Quote the spec verbatim** — Copy the exact language from the spec as comments
3. **Match the spec's structure** — State machines, token types, and algorithms should mirror the spec's organization

Example of good code:
```rust
// Spec: https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state
fn handle_tag_name_state(&mut self) {
    match self.current_input_character {
        // Spec: "U+003E GREATER-THAN SIGN (>) - Switch to the data state.
        // Emit the current tag token."
        Some('>') => {
            self.switch_to(TokenizerState::Data);
            self.emit_token();
        }
        // ...
    }
}
```

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
├── bin/
│   ├── koala.rs          # GUI application
│   └── koala_cli.rs      # CLI for testing tokenizer
├── src/
│   ├── app/              # GUI (iced framework)
│   ├── lib_html/
│   │   ├── html_tokenizer/   # HTML5 tokenizer (spec: §13.2.5)
│   │   └── html_parser/      # Tree construction (spec: §13.2.6) [not started]
│   └── lib_dom/          # DOM node structures
└── res/                  # Test files
```

### Current Status

- **Tokenizer**: Partial — handles DOCTYPE, tags, attributes, comments. Missing: character references, script/RCDATA modes
- **Parser**: Not started (empty file)
- **DOM**: Structures defined, not populated
- **Rendering**: Not started

### Dependencies

- **iced** — GUI framework
- **Boa** (planned) — JavaScript engine (we're not writing a JS engine from scratch)

## Code Style

- Spec comments go above the code they describe
- Use `// Spec: "quoted text"` for verbatim spec language
- Use `// Spec: URL` for section links
- Panic on impossible states (indicates tokenizer bug, not bad input)
- Parse errors are logged, not fatal — HTML parsing is intentionally permissive
