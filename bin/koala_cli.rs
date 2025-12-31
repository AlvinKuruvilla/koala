use std::fs::read_to_string;

pub use anyhow::Result;
use koala::lib_html::html_parser::parser::{print_tree, HTMLParser};
use koala::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

pub fn main() -> Result<()> {
    let html_input: String = read_to_string("res/simple.html")?;
    println!("=== HTML Input ===");
    println!("{}", html_input);

    println!("\n=== Tokenizing ===");
    let mut tokenizer = HTMLTokenizer::new(html_input);
    tokenizer.run();

    println!("\n=== Parsing ===");
    let tokens = tokenizer.into_tokens();
    let parser = HTMLParser::new(tokens);
    let document = parser.run();

    println!("\n=== DOM Tree ===");
    print_tree(&document, 0);

    Ok(())
}
