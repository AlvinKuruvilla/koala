//! Koala Browser CLI
//!
//! A headless browser for testing and debugging.

use anyhow::Result;
use koala_browser::{load_document, parse_html_string, LoadedDocument};
use koala_html::print_tree;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: koala-cli <file.html | URL>");
        eprintln!("       koala-cli --html '<html>...</html>'");
        std::process::exit(1);
    }

    let doc = if args[1] == "--html" {
        if args.len() < 3 {
            eprintln!("Error: --html requires an HTML string argument");
            std::process::exit(1);
        }
        parse_html_string(&args[2])
    } else {
        load_document(&args[1]).map_err(|e| anyhow::anyhow!("{}", e))?
    };

    print_document(&doc);

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
