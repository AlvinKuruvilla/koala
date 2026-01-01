//! Koala Browser CLI
//!
//! A headless browser for testing and debugging.

use anyhow::Result;
use koala_browser::parse_document;
use koala_dom::DomTree;
use koala_html::print_tree;
use std::env;
use std::fs;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: koala-cli <file.html>");
        eprintln!("       koala-cli --html '<html>...</html>'");
        std::process::exit(1);
    }

    let html = if args[1] == "--html" {
        if args.len() < 3 {
            eprintln!("Error: --html requires an HTML string argument");
            std::process::exit(1);
        }
        args[2].clone()
    } else {
        fs::read_to_string(&args[1])?
    };

    let (tree, stylesheet, styles) = parse_document(&html);

    println!("=== DOM Tree ===");
    print_tree(&tree, tree.root(), 0);

    println!("\n=== Stylesheet ===");
    println!("{} rules", stylesheet.rules.len());

    println!("\n=== Computed Styles ===");
    println!("{} styled elements", styles.len());
    print_computed_styles(&tree, &styles);

    Ok(())
}

/// Print computed styles for each element
fn print_computed_styles(
    tree: &DomTree,
    styles: &std::collections::HashMap<koala_dom::NodeId, koala_css::style::ComputedStyle>,
) {
    for (node_id, style) in styles {
        if let Some(element) = tree.as_element(*node_id) {
            let tag = &element.tag_name;
            let mut props = Vec::new();

            if let Some(ref fs) = style.font_size {
                props.push(format!("font-size: {}px", fs.to_px()));
            }
            if let Some(ref color) = style.color {
                props.push(format!("color: #{:02x}{:02x}{:02x}", color.r, color.g, color.b));
            }
            if let Some(ref bg) = style.background_color {
                props.push(format!("background: #{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b));
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
