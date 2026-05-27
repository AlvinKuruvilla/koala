// Landing page shown in every new tab and new window.
//
// The HTML + CSS live in `koala-ui/res/landing.html` so they can
// be edited as a real `.html` file (with HTML syntax highlighting,
// LSP, and formatter support) instead of a raw string inside Rust.
// `include_str!` embeds the file contents at compile time, so there
// is no runtime I/O, no CWD dependency, and no missing-file error
// path. Touching the HTML only rebuilds `koala-ui` — the engine
// crates are unaffected.

pub const LANDING_HTML: &str = include_str!("../res/landing.html");
