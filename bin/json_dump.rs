//! CLI tool to dump parsed HTML as formatted JSON

use std::env;
use std::fs;

use koala::lib_html::html_parser::parser::HTMLParser;
use koala::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;
use koala::lib_dom::{Node, NodeType};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <html_file>", args[0]);
        eprintln!("       {} --html '<html>...'", args[0]);
        std::process::exit(1);
    }

    let html = if args[1] == "--html" {
        if args.len() < 3 {
            eprintln!("Error: --html requires an HTML string argument");
            std::process::exit(1);
        }
        args[2].clone()
    } else {
        match fs::read_to_string(&args[1]) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", args[1], e);
                std::process::exit(1);
            }
        }
    };

    // Parse HTML
    let mut tokenizer = HTMLTokenizer::new(html);
    tokenizer.run();
    let tokens = tokenizer.into_tokens();
    let parser = HTMLParser::new(tokens);
    let root = parser.run();

    // Convert to formatted JSON
    let json = node_to_json_pretty(&root, 0);
    println!("{}", json);
}

fn node_to_json_pretty(node: &Node, indent: usize) -> String {
    let spaces = "  ".repeat(indent);
    let mut json = format!("{}{{\n", spaces);

    match &node.node_type {
        NodeType::Document => {
            json.push_str(&format!("{}  \"type\": \"document\"", spaces));
        }
        NodeType::Element(data) => {
            json.push_str(&format!("{}  \"type\": \"element\",\n", spaces));
            json.push_str(&format!("{}  \"tagName\": \"{}\",\n", spaces, escape_string(&data.tag_name)));

            // Attributes
            json.push_str(&format!("{}  \"attributes\": {{", spaces));
            if data.attrs.is_empty() {
                json.push('}');
            } else {
                json.push('\n');
                let attrs: Vec<String> = data.attrs.iter()
                    .map(|(k, v)| format!("{}    \"{}\": \"{}\"", spaces, escape_string(k), escape_string(v)))
                    .collect();
                json.push_str(&attrs.join(",\n"));
                json.push_str(&format!("\n{}  }}", spaces));
            }
        }
        NodeType::Text(text) => {
            json.push_str(&format!("{}  \"type\": \"text\",\n", spaces));
            json.push_str(&format!("{}  \"content\": \"{}\"", spaces, escape_string(text)));
        }
        NodeType::Comment(text) => {
            json.push_str(&format!("{}  \"type\": \"comment\",\n", spaces));
            json.push_str(&format!("{}  \"content\": \"{}\"", spaces, escape_string(text)));
        }
    }

    // Children
    if !node.children.is_empty() {
        json.push_str(",\n");
        json.push_str(&format!("{}  \"children\": [\n", spaces));
        let children: Vec<String> = node.children.iter()
            .map(|child| node_to_json_pretty(child, indent + 2))
            .collect();
        json.push_str(&children.join(",\n"));
        json.push_str(&format!("\n{}  ]", spaces));
    }

    json.push_str(&format!("\n{}}}", spaces));
    json
}

fn escape_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}
