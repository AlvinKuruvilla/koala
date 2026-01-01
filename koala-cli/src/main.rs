//! Koala Browser CLI
//!
//! A headless browser for testing and debugging.

use anyhow::Result;
use koala_browser::parse_document;
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

    Ok(())
}
