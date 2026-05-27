// Slint compiles `.slint` markup to Rust at build time. The
// generated module is then included into `main.rs` via
// `slint::include_modules!()`. `ui/main.slint` is the entry
// point — it will grow imports for sub-components as the
// chrome lands in Phase 2 and beyond.

fn main() {
    slint_build::compile("ui/main.slint").expect("compile ui/main.slint");
}
