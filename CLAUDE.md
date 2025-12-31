# Koala Browser

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
├── bin/
│   ├── koala.rs          # GUI application
│   └── koala_cli.rs      # CLI for testing tokenizer
├── src/
│   ├── app/              # GUI (iced framework)
│   ├── lib_html/
│   │   ├── html_tokenizer/   # HTML5 tokenizer (spec: §13.2.5)
│   │   └── html_parser/      # Tree construction (spec: §13.2.6)
│   └── lib_dom/          # DOM node structures
└── res/                  # Test files
```

### Current Status

- **Tokenizer**: Partial — handles DOCTYPE, tags, attributes, comments, basic tag/attribute parsing. Missing: character references, script/RCDATA state switching
- **Parser**: Basic tree construction working — handles Initial, BeforeHtml, BeforeHead, InHead, AfterHead, InBody, Text, AfterBody, AfterAfterBody modes. Missing: table parsing, form elements, foster parenting, adoption agency algorithm
- **DOM**: Node, Element, Text, Comment types defined and populated by parser
- **Rendering**: Not started

### Dependencies

- **iced** — GUI framework
- **Boa** (planned) — JavaScript engine (we're not writing a JS engine from scratch)

## Code Style

- Spec comments go above the code they describe
- Use markdown hyperlinks for spec references: `/// [§ X.Y.Z Title](URL)`
- Use `// "quoted text"` for verbatim spec language inside match arms
- Panic on impossible states (indicates tokenizer bug, not bad input)
- Parse errors are logged, not fatal — HTML parsing is intentionally permissive
