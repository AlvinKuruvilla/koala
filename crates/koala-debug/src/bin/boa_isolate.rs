//! Minimal-repro harness for "does this JS file blow up in Boa
//! on its own?" Loads one or more files and runs each through
//! [`JsRuntime::execute`] in order, sharing a single runtime
//! bound to an empty DOM, with wall-time + peak-RSS reporting
//! per script.
//!
//! Useful when [`oom-probe`](crate) has narrowed an issue to a
//! particular script and you want to see whether the script
//! misbehaves in vacuum vs. only after some earlier-script
//! state setup.
//!
//! Run with:
//!
//! ```sh
//! just probe-boa <file>                   # via the justfile recipe
//! cargo run --release --bin boa-isolate -- <file> [<file> …]
//! ```

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use koala_dom::DomTree;
use koala_js::JsRuntime;

#[allow(unsafe_code)]
fn peak_rss_bytes() -> u64 {
    // SAFETY: getrusage with RUSAGE_SELF and a zeroed rusage out-pointer is
    // the documented contract.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &raw mut ru) };
    if rc != 0 {
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        ru.ru_maxrss as u64
    }
    #[cfg(not(target_os = "macos"))]
    {
        (ru.ru_maxrss as u64).saturating_mul(1024)
    }
}

fn fmt_mb(n: u64) -> String {
    format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: boa-isolate <path-to-js> [path-to-js …]");
        std::process::exit(2);
    }

    let dom = Rc::new(RefCell::new(DomTree::new()));
    let mut runtime = JsRuntime::new(dom);
    eprintln!(
        "[boa_isolate] runtime ready, rss={}",
        fmt_mb(peak_rss_bytes())
    );

    for path in &paths {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[boa_isolate] could not read {path}: {e}");
                std::process::exit(2);
            }
        };
        eprintln!(
            "[boa_isolate] >> exec {path} ({} bytes), rss={}",
            source.len(),
            fmt_mb(peak_rss_bytes())
        );
        let start = Instant::now();
        let result = runtime.execute(&source);
        let elapsed = start.elapsed();
        match result {
            Ok(_) => eprintln!(
                "[boa_isolate] << ok in {elapsed:?}, rss={}",
                fmt_mb(peak_rss_bytes())
            ),
            Err(e) => eprintln!(
                "[boa_isolate] << error in {elapsed:?}, rss={}: {e}",
                fmt_mb(peak_rss_bytes())
            ),
        }
    }
}
