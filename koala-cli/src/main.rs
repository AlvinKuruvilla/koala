//! Koala Browser CLI
//!
//! A headless browser for testing and debugging.

use anyhow::Result;
use clap::Parser;
use koala_browser::{LoadedDocument, load_document, parse_html_string, renderer::Renderer};
use koala_css::{LayoutBox, Painter};
use koala_dom::{DomTree, NodeId, NodeType};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

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
#[allow(clippy::cast_precision_loss)] // viewport dimensions don't need full u32 precision
fn take_screenshot(
    doc: &LoadedDocument,
    output_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
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

    // Paint: generate display list from layout tree
    let painter = Painter::new(&doc.styles);
    let display_list = painter.paint(&layout);

    // Render: execute display list to pixels
    let mut renderer = Renderer::new(width, height);
    renderer.render(&display_list);

    // Save to file
    renderer.save(output_path)?;

    Ok(())
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
        layout.layout(viewport, viewport);

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
        koala_css::BoxType::Principal(node_id) => {
            if let Some(element) = doc.dom.as_element(*node_id) {
                format!("<{}>", element.tag_name)
            } else if doc
                .dom
                .get(*node_id)
                .is_some_and(|n| matches!(n.node_type, NodeType::Document))
            {
                "Document".to_string()
            } else {
                format!("{node_id:?}")
            }
        }
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
            props.push(format_style_prop(
                "font-size",
                &format!("{}px", fs.to_px()),
            ));
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
                props.push(format_style_prop("margin-top", &format!("{}px", px)));
            }
        }
        if let Some(AutoLength::Length(ref len)) = style.margin_bottom {
            let px = len.to_px();
            if px != 0.0 {
                props.push(format_style_prop("margin-bottom", &format!("{}px", px)));
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
