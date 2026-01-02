//! Koala Browser CLI
//!
//! A headless browser for testing and debugging.

mod renderer;

use anyhow::Result;
use clap::Parser;
use koala_browser::{load_document, parse_html_string, LoadedDocument};
use koala_css::LayoutBox;
use koala_html::print_tree;
use std::path::PathBuf;

/// Koala Browser CLI - A headless browser for testing and debugging
#[derive(Parser, Debug)]
#[command(name = "koala-cli")]
#[command(author, version, about, long_about = None)]
#[command(group = clap::ArgGroup::new("input").required(true))]
#[command(after_help = r#"EXAMPLES:
    # Parse a local file and show DOM tree
    koala-cli ./index.html

    # Fetch a URL and show DOM tree
    koala-cli https://example.com

    # Show layout tree for debugging CSS
    koala-cli --layout https://example.com

    # Parse inline HTML
    koala-cli --html '<html><body><h1>Test</h1></body></html>'

    # Parse inline HTML and show layout
    koala-cli --html '<div style="margin: auto; width: 50vw">Centered</div>' --layout

    # Take a screenshot of a webpage
    koala-cli -S screenshot.png https://example.com

    # Screenshot with custom viewport size
    koala-cli --screenshot output.png --width 1920 --height 1080 https://example.com
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine the document source
    let doc = if let Some(html_string) = cli.html {
        parse_html_string(&html_string)
    } else if let Some(path) = cli.path {
        load_document(&path).map_err(|e| anyhow::anyhow!("{}", e))?
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
fn take_screenshot(doc: &LoadedDocument, output_path: &PathBuf, width: u32, height: u32) -> Result<()> {
    use renderer::Renderer;

    let viewport = koala_css::Rect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
    };

    // Get the layout tree and compute layout
    let layout_tree = doc
        .layout_tree
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No layout tree available"))?;

    let mut layout = layout_tree.clone();
    layout.layout(viewport, viewport);

    // Create renderer and paint
    let mut renderer = Renderer::new(width, height);
    renderer.render(&layout, doc);

    // Save to file
    renderer.save(output_path)?;

    Ok(())
}

/// Print document information to stdout.
fn print_document(doc: &LoadedDocument) {
    println!("=== DOM Tree ===");
    print_tree(&doc.dom, doc.dom.root(), 0);

    println!("\n=== Stylesheet ===");
    println!("{} rules", doc.stylesheet.rules.len());

    println!("\n=== Computed Styles ===");
    println!("{} styled elements", doc.styles.len());
    print_computed_styles(doc);

    if doc.layout_tree.is_some() {
        println!("\n=== Layout Tree ===");
        println!("Layout tree built successfully");
    }

    if !doc.parse_issues.is_empty() {
        println!("\n=== Parse Issues ===");
        for issue in &doc.parse_issues {
            println!("  - {}", issue);
        }
    }
}

/// Print layout tree with computed dimensions.
fn print_layout(doc: &LoadedDocument) {
    // Default viewport size (same as what we'd use in the GUI)
    let viewport_width = 1280.0;
    let viewport_height = 720.0;

    println!(
        "=== Layout Tree (viewport: {}x{}) ===\n",
        viewport_width, viewport_height
    );

    if let Some(ref layout_tree) = doc.layout_tree {
        // Clone and compute layout
        let mut layout = layout_tree.clone();
        let viewport = koala_css::Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_width,
            height: viewport_height,
        };
        layout.layout(viewport, viewport);

        print_layout_box(&layout, 0, doc);
    } else {
        println!("No layout tree available");
    }
}

/// Recursively print a layout box with its dimensions.
fn print_layout_box(layout_box: &LayoutBox, depth: usize, doc: &LoadedDocument) {
    let indent = "  ".repeat(depth);
    let dims = &layout_box.dimensions;

    // Get box name with tag if available
    let name = match &layout_box.box_type {
        koala_css::BoxType::Principal(node_id) => {
            // Try to get the element's tag name
            if let Some(element) = doc.dom.as_element(*node_id) {
                format!("<{}> ({:?})", element.tag_name, node_id)
            } else if doc
                .dom
                .get(*node_id)
                .map(|n| matches!(n.node_type, koala_browser::dom::NodeType::Document))
                .unwrap_or(false)
            {
                format!("Document ({:?})", node_id)
            } else {
                format!("{:?}", node_id)
            }
        }
        koala_css::BoxType::AnonymousBlock => "AnonymousBlock".to_string(),
        koala_css::BoxType::AnonymousInline(text) => {
            let preview: String = text.chars().take(30).collect();
            let suffix = if text.len() > 30 { "..." } else { "" };
            format!("Text(\"{}{}\")", preview.replace('\n', "\\n"), suffix)
        }
    };

    // Print box info with dimensions
    println!("{}[{}] {:?}", indent, name, layout_box.display);

    // Print content box
    println!(
        "{}  content: x={:.1} y={:.1} w={:.1} h={:.1}",
        indent, dims.content.x, dims.content.y, dims.content.width, dims.content.height
    );

    // Print margins if non-zero
    if dims.margin.top != 0.0
        || dims.margin.right != 0.0
        || dims.margin.bottom != 0.0
        || dims.margin.left != 0.0
    {
        println!(
            "{}  margin: t={:.1} r={:.1} b={:.1} l={:.1}",
            indent, dims.margin.top, dims.margin.right, dims.margin.bottom, dims.margin.left
        );
    }

    // Print padding if non-zero
    if dims.padding.top != 0.0
        || dims.padding.right != 0.0
        || dims.padding.bottom != 0.0
        || dims.padding.left != 0.0
    {
        println!(
            "{}  padding: t={:.1} r={:.1} b={:.1} l={:.1}",
            indent, dims.padding.top, dims.padding.right, dims.padding.bottom, dims.padding.left
        );
    }

    println!();

    // Print children
    for child in &layout_box.children {
        print_layout_box(child, depth + 1, doc);
    }
}

/// Print computed styles for each element
fn print_computed_styles(doc: &LoadedDocument) {
    for (node_id, style) in &doc.styles {
        if let Some(element) = doc.dom.as_element(*node_id) {
            let tag = &element.tag_name;
            let mut props = Vec::new();

            if let Some(ref fs) = style.font_size {
                props.push(format!("font-size: {}px", fs.to_px()));
            }
            if let Some(ref color) = style.color {
                props.push(format!("color: {}", color.to_hex_string()));
            }
            if let Some(ref bg) = style.background_color {
                props.push(format!("background: {}", bg.to_hex_string()));
            }
            if let Some(ref m) = style.margin_top {
                props.push(format!("margin-top: {}px", m.to_px()));
            }

            if !props.is_empty() {
                println!("  <{}> {{ {} }}", tag, props.join("; "));
            }
        }
    }
}
