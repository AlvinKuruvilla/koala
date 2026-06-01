//! Koala — fast, lightweight HTML-to-image renderer
//!
//! Renders HTML/CSS to images with CSS 2.1 compliant layout.

#[cfg(feature = "bench")]
mod bench;
mod render;

// Heap accounting for `--bench` mode. Installed only under the
// `bench` feature so the shipping renderer keeps the system
// allocator untouched and pays nothing for the counters.
#[cfg(feature = "bench")]
#[global_allocator]
static GLOBAL: koala_common::alloc_count::CountingAllocator =
    koala_common::alloc_count::CountingAllocator;

mod wpt_protocol;

use anyhow::Result;
use clap::Parser;
use koala_browser::{FontProvider, LoadedDocument, load_document, parse_html_string};
use koala_css::LayoutBox;
use koala_dom::{DomTree, NodeId, NodeType};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

use crate::render::render_document_to_path;

/// Koala — fast, lightweight HTML-to-image renderer
#[derive(Parser, Debug)]
#[command(name = "koala")]
#[command(author, version, about, long_about = None)]
#[command(group = clap::ArgGroup::new("input").required(true))]
#[command(after_help = r#"EXAMPLES:
    # Parse a local file and show DOM tree
    koala ./index.html

    # Fetch a URL and show DOM tree
    koala https://example.com

    # Show layout tree for debugging CSS
    koala --layout https://example.com

    # Parse inline HTML
    koala --html '<html><body><h1>Test</h1></body></html>'

    # Parse inline HTML and show layout
    koala --html '<div style="margin: auto; width: 50vw">Centered</div>' --layout

    # Take a screenshot of a webpage
    koala -S screenshot.png https://example.com

    # Screenshot with custom viewport size
    koala --screenshot output.png --width 1920 --height 1080 https://example.com
"#)]
struct Cli {
    /// Path to HTML file or URL to fetch and parse
    #[arg(value_name = "FILE|URL", group = "input")]
    path: Option<String>,

    /// Parse HTML string directly instead of file/URL
    #[arg(long, value_name = "HTML", group = "input")]
    html: Option<String>,

    /// Show computed layout tree with dimensions instead of DOM tree.
    /// Uses a 1280x720 viewport. Useful for debugging CSS layout issues.
    #[arg(long)]
    layout: bool,

    /// Take a screenshot and save to the specified file (PNG format).
    /// Renders the page to an image for visual debugging without the GUI.
    #[arg(short = 'S', long, value_name = "FILE")]
    screenshot: Option<PathBuf>,

    /// Viewport width for screenshot (default: 1280)
    #[arg(long, default_value = "1280")]
    width: u32,

    /// Viewport height for screenshot (default: 720)
    #[arg(long, default_value = "720")]
    height: u32,

    /// Run in WPT protocol mode: read JSON-line commands from
    /// stdin, emit JSON-line events on stdout. Used by the
    /// wptrunner browser plugin to drive koala under upstream WPT.
    #[arg(
        long,
        group = "input",
        conflicts_with_all = ["layout", "screenshot", "width", "height"]
    )]
    wpt_protocol: bool,

    /// Path to a WPT-format hosts file (output of
    /// `wpt make-hosts-file`). When set, koala uses it to override
    /// DNS resolution for the listed hostnames instead of touching
    /// `/etc/hosts`. Applies in every mode, but most useful with
    /// `--wpt-protocol`.
    #[arg(long, value_name = "FILE")]
    hosts_file: Option<PathBuf>,

    /// Perf-harness mode: load the page once, render it N+warmup
    /// times, emit a per-stage timing JSON report on stdout.
    /// Requires the `bench` cargo feature (which enables the
    /// `render-trace` spans). The `just bench` recipe wires this
    /// up; ad-hoc invocations need `cargo run --release --features
    /// bench --bin koala -- --bench <path>`.
    #[arg(
        long,
        conflicts_with_all = ["html", "layout", "screenshot", "wpt_protocol"]
    )]
    bench: bool,

    /// Sample iteration count for `--bench`. Default 30 is enough
    /// for the per-stage means to stabilise below ~5 % run-to-run
    /// variance on the landing page.
    #[arg(long, default_value = "30", value_name = "N")]
    bench_iterations: u32,

    /// Discard-iteration count for `--bench`. Default 3 lets the
    /// OS page in glyph data and warms any lazy caches before the
    /// sampled iterations start.
    #[arg(long, default_value = "3", value_name = "N")]
    bench_warmup: u32,
}

fn main() -> Result<()> {
    #[cfg(feature = "layout-trace")]
    {
        let stack_marker: u8 = 0;
        let stack_addr = &stack_marker as *const u8 as usize;
        #[allow(unsafe_code)]
        // SAFETY: pthread_self and stack info functions are safe to call
        unsafe {
            let thread = libc::pthread_self();
            let stack_base = libc::pthread_get_stackaddr_np(thread) as usize;
            let stack_size = libc::pthread_get_stacksize_np(thread);
            let stack_bottom = stack_base - stack_size;
            let remaining = stack_addr.saturating_sub(stack_bottom);
            eprintln!(
                "[STACK] main(): addr=0x{stack_addr:x} base=0x{stack_base:x} size={stack_size} ({:.1}MB) bottom=0x{stack_bottom:x} remaining={remaining} ({:.1}KB)",
                stack_size as f64 / 1024.0 / 1024.0,
                remaining as f64 / 1024.0
            );
        }
    }
    let cli = Cli::parse();

    // Install host overrides before any HTTP fetch can happen. Done
    // for every mode so the same setup applies to both CLI usage
    // and protocol-driven runs.
    if let Some(ref path) = cli.hosts_file {
        koala_browser::hosts::set_from_file(path)
            .map_err(|e| anyhow::anyhow!("failed to load --hosts-file '{}': {e}", path.display()))?;
    }

    // Protocol mode owns its own input loop and rendering pipeline;
    // dispatch before any CLI-style argument validation runs.
    if cli.wpt_protocol {
        return wpt_protocol::run();
    }

    // Bench mode similarly owns its own pipeline (no screenshot
    // save, repeated render). Dispatch before screenshot
    // validation since it doesn't take an output path.
    if cli.bench {
        let path = cli
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--bench requires a path/URL argument"))?;
        #[cfg(feature = "bench")]
        {
            return bench::run(
                path,
                cli.width,
                cli.height,
                cli.bench_iterations,
                cli.bench_warmup,
            );
        }
        #[cfg(not(feature = "bench"))]
        {
            let _ = path;
            anyhow::bail!(
                "--bench requires the `bench` cargo feature. \
                 Run `just bench`, or rebuild with \
                 `cargo run --release --features bench --bin koala -- ...`."
            );
        }
    }

    // Validate screenshot output path before doing any expensive work.
    if let Some(ref output_path) = cli.screenshot {
        let supported = [
            "png", "jpg", "jpeg", "gif", "bmp", "tiff", "tga", "ico", "pnm", "webp",
        ];
        match output_path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if supported.contains(&ext.to_ascii_lowercase().as_str()) => {}
            Some(ext) => {
                anyhow::bail!(
                    "Unsupported screenshot format '.{ext}' for '{}'.\n\
                     Supported formats: {}",
                    output_path.display(),
                    supported.join(", ")
                );
            }
            None => {
                anyhow::bail!(
                    "Screenshot path '{}' has no file extension.\n\
                     Please add one of the supported formats: {}",
                    output_path.display(),
                    supported.join(", ")
                );
            }
        }
    }

    // Determine the document source
    let doc = if let Some(html_string) = cli.html {
        parse_html_string(&html_string)
    } else if let Some(path) = cli.path {
        load_document(&path)?
    } else {
        // clap should prevent this, but just in case
        anyhow::bail!("Either a file/URL path or --html must be provided");
    };

    // Handle screenshot mode
    if let Some(ref output_path) = cli.screenshot {
        take_screenshot(&doc, output_path, cli.width, cli.height)?;
        println!("Screenshot saved to: {}", output_path.display());
        return Ok(());
    }

    if cli.layout {
        print_layout(&doc);
    } else {
        print_document(&doc);
    }

    Ok(())
}

/// Take a screenshot of the rendered page and save to file.
fn take_screenshot(
    doc: &LoadedDocument,
    output_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    let font_provider = FontProvider::load();
    render_document_to_path(doc, output_path, width, height, &font_provider)
}

/// Print a section header with formatting.
fn print_header(title: &str) {
    println!();
    println!("{}", format!("─── {title} ───").cyan().bold());
}

/// Print a sub-header for sections.
fn print_subheader(text: &str) {
    println!("    {}", text.dimmed());
}

/// Print document information to stdout.
fn print_document(doc: &LoadedDocument) {
    print_header("DOM Tree");
    print_dom_tree(&doc.dom, doc.dom.root(), 0);

    print_header("Stylesheet");
    let rule_count = doc.stylesheet.rules.len();
    if rule_count == 0 {
        print_subheader("No CSS rules");
    } else {
        print_subheader(&format!(
            "{} rule{}",
            rule_count,
            if rule_count == 1 { "" } else { "s" }
        ));
    }

    print_header("Computed Styles");
    let style_count = doc.styles.len();
    if style_count == 0 {
        print_subheader("No styled elements");
    } else {
        print_subheader(&format!(
            "{} styled element{}",
            style_count,
            if style_count == 1 { "" } else { "s" }
        ));
        println!();
        print_computed_styles(doc);
    }

    if doc.layout_tree.is_some() {
        print_header("Layout");
        print_subheader("Layout tree built successfully");
    }

    if !doc.parse_issues.is_empty() {
        print_header("Parse Issues");
        for issue in &doc.parse_issues {
            println!("    {} {}", "!".yellow(), issue);
        }
    }

    println!();
}

/// Print colorized DOM tree.
fn print_dom_tree(tree: &DomTree, id: NodeId, indent: usize) {
    let prefix = "  ".repeat(indent);
    let Some(node) = tree.get(id) else { return };

    match &node.node_type {
        NodeType::Document => {
            println!("{}{}", prefix, "Document".blue().bold());
        }
        NodeType::Element(data) => {
            if data.attrs.is_empty() {
                println!("{}{}", prefix, format!("<{}>", data.tag_name).green());
            } else {
                let tag_open = format!("<{}", data.tag_name);
                print!("{}{}", prefix, tag_open.green());
                for (key, value) in &data.attrs {
                    if value.is_empty() {
                        print!(" {}", key.yellow());
                    } else {
                        print!(
                            " {}{}{}{}{}",
                            key.yellow(),
                            "=".dimmed(),
                            "\"".dimmed(),
                            value.magenta(),
                            "\"".dimmed()
                        );
                    }
                }
                println!("{}", ">".green());
            }
        }
        NodeType::Text(data) => {
            let display = format_text_content(data);
            if !display.trim().is_empty() || display.contains("\\n") {
                println!(
                    "{}{}{}{}",
                    prefix,
                    "\"".dimmed(),
                    display.white(),
                    "\"".dimmed()
                );
            }
        }
        NodeType::Comment(data) => {
            println!(
                "{}{}{}{}",
                prefix,
                "<!-- ".dimmed(),
                data.dimmed().italic(),
                " -->".dimmed()
            );
        }
    }

    for &child_id in tree.children(id) {
        print_dom_tree(tree, child_id, indent + 1);
    }
}

/// Format text content for display, showing whitespace characters.
fn format_text_content(text: &str) -> String {
    text.replace('\n', "\\n").replace(' ', "\u{00B7}")
}

/// Print layout tree with computed dimensions.
fn print_layout(doc: &LoadedDocument) {
    let viewport_width = 1280.0;
    let viewport_height = 720.0;

    print_header("Layout Tree");
    print_subheader(&format!("viewport: {viewport_width}x{viewport_height}"));
    println!();

    if let Some(ref layout_tree) = doc.layout_tree {
        let mut layout = layout_tree.clone();
        let viewport = koala_css::Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_width,
            height: viewport_height,
        };
        let font_provider = FontProvider::load();
        let font_metrics = font_provider.metrics();
        layout.layout(viewport, viewport, &*font_metrics, viewport);

        print_layout_box(&layout, 0, doc);
    } else {
        println!("    {}", "No layout tree available".dimmed());
    }

    println!();
}

/// Recursively print a layout box with its dimensions.
fn print_layout_box(layout_box: &LayoutBox, depth: usize, doc: &LoadedDocument) {
    let indent = "  ".repeat(depth);
    let dims = &layout_box.dimensions;

    // Get box name with tag if available
    let name = match &layout_box.box_type {
        koala_css::BoxType::Principal(node_id) => doc.dom.as_element(*node_id).map_or_else(
            || {
                if doc
                    .dom
                    .get(*node_id)
                    .is_some_and(|n| matches!(n.node_type, NodeType::Document))
                {
                    "Document".to_string()
                } else {
                    format!("{node_id:?}")
                }
            },
            |element| format!("<{}>", element.tag_name),
        ),
        koala_css::BoxType::AnonymousBlock => "AnonymousBlock".to_string(),
        koala_css::BoxType::AnonymousInline(text) => {
            let preview: String = text.chars().take(25).collect();
            let suffix = if text.len() > 25 { "..." } else { "" };
            format!("\"{}{}\"", preview.replace('\n', "\\n"), suffix)
        }
    };

    // Format display type
    let display_str = format!("{:?}", layout_box.display);

    // Print box header
    match &layout_box.box_type {
        koala_css::BoxType::Principal(_) => {
            println!(
                "{}{}  {}",
                indent,
                name.green().bold(),
                display_str.dimmed()
            );
        }
        koala_css::BoxType::AnonymousBlock => {
            println!("{}{}  {}", indent, name.blue(), display_str.dimmed());
        }
        koala_css::BoxType::AnonymousInline(_) => {
            println!(
                "{}{}  {}",
                indent,
                name.white().dimmed(),
                display_str.dimmed()
            );
        }
    }

    // Print content box dimensions
    println!(
        "{}  {} {}{} {}{} {}{} {}{}",
        indent,
        "content:".cyan(),
        "x=".dimmed(),
        format!("{:.0}", dims.content.x).yellow(),
        "y=".dimmed(),
        format!("{:.0}", dims.content.y).yellow(),
        "w=".dimmed(),
        format!("{:.0}", dims.content.width).yellow(),
        "h=".dimmed(),
        format!("{:.0}", dims.content.height).yellow(),
    );

    // Print margins if non-zero
    if dims.margin.top != 0.0
        || dims.margin.right != 0.0
        || dims.margin.bottom != 0.0
        || dims.margin.left != 0.0
    {
        println!(
            "{}  {} [{} {} {} {}]",
            indent,
            "margin:".cyan(),
            format!("{:.0}", dims.margin.top).magenta(),
            format!("{:.0}", dims.margin.right).magenta(),
            format!("{:.0}", dims.margin.bottom).magenta(),
            format!("{:.0}", dims.margin.left).magenta(),
        );
    }

    // Print padding if non-zero
    if dims.padding.top != 0.0
        || dims.padding.right != 0.0
        || dims.padding.bottom != 0.0
        || dims.padding.left != 0.0
    {
        println!(
            "{}  {} [{} {} {} {}]",
            indent,
            "padding:".cyan(),
            format!("{:.0}", dims.padding.top).magenta(),
            format!("{:.0}", dims.padding.right).magenta(),
            format!("{:.0}", dims.padding.bottom).magenta(),
            format!("{:.0}", dims.padding.left).magenta(),
        );
    }

    // Print children
    for child in &layout_box.children {
        print_layout_box(child, depth + 1, doc);
    }
}

/// Print computed styles for each element
fn print_computed_styles(doc: &LoadedDocument) {
    use koala_css::AutoLength;

    for (node_id, style) in &doc.styles {
        let Some(element) = doc.dom.as_element(*node_id) else {
            continue;
        };

        let tag = &element.tag_name;
        let mut props = Vec::new();

        // Collect style properties
        if let Some(ref fs) = style.font_size {
            props.push(format_style_prop("font-size", &format!("{}px", fs.to_px())));
        }
        if let Some(ref color) = style.color {
            props.push(format_style_prop("color", &color.to_hex_string()));
        }
        if let Some(ref bg) = style.background_color {
            props.push(format_style_prop("background", &bg.to_hex_string()));
        }
        if let Some(AutoLength::Length(ref len)) = style.margin_top {
            let px = len.to_px();
            if px != 0.0 {
                props.push(format_style_prop("margin-top", &format!("{px}px")));
            }
        }
        if let Some(AutoLength::Length(ref len)) = style.margin_bottom {
            let px = len.to_px();
            if px != 0.0 {
                props.push(format_style_prop("margin-bottom", &format!("{px}px")));
            }
        }
        if let Some(ref d) = style.display {
            let display_str = format!("{:?}/{:?}", d.outer, d.inner);
            props.push(format_style_prop("display", &display_str.to_lowercase()));
        }

        if !props.is_empty() {
            println!(
                "    {} {} {} {}",
                format!("<{tag}>").green(),
                "{".dimmed(),
                props.join(&format!("{} ", ";".dimmed())),
                "}".dimmed()
            );
        }
    }
}

/// Format a single style property with colorization.
fn format_style_prop(name: &str, value: &str) -> String {
    format!("{}{} {}", name.cyan(), ":".dimmed(), value.yellow())
}
