use std::fs::read_to_string;

pub use anyhow::Result;
use koala::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;
pub fn main() -> Result<()> {
    let html_input: String = read_to_string("res/simple.html")?;
    let mut tokenizer = HTMLTokenizer::new(html_input.clone());
    println!("HTML is: {}", html_input);
    tokenizer.run();
    Ok(())
}
