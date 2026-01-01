//! Koala Browser CLI
//!
//! A headless browser for testing and debugging.

use anyhow::Result;
use koala_browser::{load_document, parse_html_string, LoadedDocument};
use koala_css::LayoutBox;
use koala_html::print_tree;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: koala-cli <file.html | URL>");
        eprintln!("       koala-cli --html '<html>...</html>'");
        eprintln!("       koala-cli --layout <file.html | URL>  # Show layout tree");
        std::process::exit(1);
    }

    // Check for --layout flag
    let show_layout = args.contains(&"--layout".to_string());
    let args_filtered: Vec<&String> = args.iter()
        .filter(|a| *a != "--layout")
        .collect();

    let doc = if args_filtered.get(1) == Some(&&"--html".to_string()) {
        if args_filtered.len() < 3 {
            eprintln!("Error: --html requires an HTML string argument");
            std::process::exit(1);
        }
        parse_html_string(args_filtered[2])
    } else if let Some(path) = args_filtered.get(1) {
        load_document(path).map_err(|e| anyhow::anyhow!("{}", e))?
    } else {
        eprintln!("Error: missing file path or URL");
        std::process::exit(1);
    };

    if show_layout {
        print_layout(&doc);
    } else {
        print_document(&doc);
    }

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

    println!("=== Layout Tree (viewport: {}x{}) ===\n", viewport_width, viewport_height);

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
            } else if doc.dom.get(*node_id).map(|n| matches!(n.node_type, koala_browser::dom::NodeType::Document)).unwrap_or(false) {
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
    println!(
        "{}[{}] {:?}",
        indent,
        name,
        layout_box.display
    );

    // Print content box
    println!(
        "{}  content: x={:.1} y={:.1} w={:.1} h={:.1}",
        indent,
        dims.content.x,
        dims.content.y,
        dims.content.width,
        dims.content.height
    );

    // Print margins if non-zero
    if dims.margin.top != 0.0 || dims.margin.right != 0.0
        || dims.margin.bottom != 0.0 || dims.margin.left != 0.0 {
        println!(
            "{}  margin: t={:.1} r={:.1} b={:.1} l={:.1}",
            indent,
            dims.margin.top,
            dims.margin.right,
            dims.margin.bottom,
            dims.margin.left
        );
    }

    // Print padding if non-zero
    if dims.padding.top != 0.0 || dims.padding.right != 0.0
        || dims.padding.bottom != 0.0 || dims.padding.left != 0.0 {
        println!(
            "{}  padding: t={:.1} r={:.1} b={:.1} l={:.1}",
            indent,
            dims.padding.top,
            dims.padding.right,
            dims.padding.bottom,
            dims.padding.left
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
