//! Koala Browser CLI - HTML/CSS parsing and debugging tool
//!
//! Usage:
//!   koala <file>              Parse and display DOM tree with computed styles
//!   koala <file> --json       Output DOM as JSON with computed styles
//!   koala <file> --tokens     Show HTML tokens
//!   koala <file> --css        Show extracted CSS and parsed rules
//!   koala <file> --verbose    Show all debugging information
//!   koala <file> --no-color   Disable colored output
//!
//! Examples:
//!   koala res/simple.html
//!   koala res/simple.html --json
//!   koala --html '<h1>Hello</h1>'

use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};

use koala::lib_css::css_cascade::compute_styles;
use koala::lib_css::css_parser::parser::{CSSParser, Rule};
use koala::lib_css::css_tokenizer::tokenizer::CSSTokenizer;
use koala::lib_css::extract_style_content;
use koala::lib_dom::{Node, NodeType};
use koala::lib_html::html_parser::parser::HTMLParser;
use koala::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

// ANSI color codes
mod color {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";

    // Foreground colors
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    pub const GRAY: &str = "\x1b[90m";

    // Bright colors
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
}

struct Colors {
    enabled: bool,
}

impl Colors {
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn wrap(&self, text: &str, color: &str) -> String {
        if self.enabled {
            format!("{}{}{}", color, text, color::RESET)
        } else {
            text.to_string()
        }
    }

    fn tag(&self, text: &str) -> String {
        self.wrap(text, color::BRIGHT_BLUE)
    }

    fn attr_name(&self, text: &str) -> String {
        self.wrap(text, color::CYAN)
    }

    fn attr_value(&self, text: &str) -> String {
        self.wrap(text, color::YELLOW)
    }

    fn text(&self, text: &str) -> String {
        self.wrap(text, color::GREEN)
    }

    fn comment(&self, text: &str) -> String {
        self.wrap(text, color::GRAY)
    }

    fn style(&self, text: &str) -> String {
        self.wrap(text, color::BRIGHT_MAGENTA)
    }

    fn css_prop(&self, text: &str) -> String {
        self.wrap(text, color::CYAN)
    }

    fn css_value(&self, text: &str) -> String {
        self.wrap(text, color::YELLOW)
    }

    fn header(&self, text: &str) -> String {
        self.wrap(text, &format!("{}{}", color::BOLD, color::WHITE))
    }

    fn dim(&self, text: &str) -> String {
        self.wrap(text, color::DIM)
    }

    fn selector(&self, text: &str) -> String {
        self.wrap(text, color::BRIGHT_CYAN)
    }

    fn number(&self, text: &str) -> String {
        self.wrap(text, color::BRIGHT_YELLOW)
    }
}

#[derive(Default)]
struct Options {
    json: bool,
    tokens: bool,
    css: bool,
    verbose: bool,
    no_color: bool,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    // Parse arguments
    let mut options = Options::default();
    let mut html_source: Option<String> = None;
    let mut file_path: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" | "-j" => options.json = true,
            "--tokens" | "-t" => options.tokens = true,
            "--css" | "-c" => options.css = true,
            "--verbose" | "-v" => {
                options.verbose = true;
                options.tokens = true;
                options.css = true;
            }
            "--no-color" => options.no_color = true,
            "--html" => {
                i += 1;
                if i < args.len() {
                    html_source = Some(args[i].clone());
                } else {
                    eprintln!("Error: --html requires an HTML string argument");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                print_usage(&args[0]);
                std::process::exit(0);
            }
            arg if !arg.starts_with('-') => {
                file_path = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // Determine if colors should be enabled
    let use_colors = !options.no_color && io::stdout().is_terminal();
    let c = Colors::new(use_colors);

    // Get HTML content
    let html = if let Some(src) = html_source {
        src
    } else if let Some(ref path) = file_path {
        match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading '{}': {}", path, e);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Error: No input file or --html provided");
        print_usage(&args[0]);
        std::process::exit(1);
    };

    // Tokenize HTML
    let mut tokenizer = HTMLTokenizer::new(html.clone());
    tokenizer.run();
    let tokens = tokenizer.into_tokens();

    if options.tokens {
        println!("{}", c.header(&format!("=== HTML Tokens ({}) ===", tokens.len())));
        for (i, token) in tokens.iter().enumerate() {
            println!("  {}: {:?}", c.number(&format!("{:3}", i)), token);
        }
        println!();
    }

    // Parse HTML
    let parser = HTMLParser::new(tokens);
    let dom = parser.run();

    // Extract and parse CSS
    let css_text = extract_style_content(&dom);
    let styles = if !css_text.is_empty() {
        let mut css_tokenizer = CSSTokenizer::new(css_text.clone());
        css_tokenizer.run();
        let css_tokens = css_tokenizer.into_tokens();

        if options.css {
            println!("{}", c.header(&format!("=== CSS ({} chars) ===", css_text.len())));
            print_css_highlighted(&css_text, &c);
            println!();

            if options.verbose {
                println!("{}", c.header(&format!("=== CSS Tokens ({}) ===", css_tokens.len())));
                for (i, token) in css_tokens.iter().enumerate() {
                    println!("  {}: {:?}", c.number(&format!("{:3}", i)), token);
                }
                println!();
            }
        }

        let mut css_parser = CSSParser::new(css_tokens);
        let stylesheet = css_parser.parse_stylesheet();

        if options.css {
            println!("{}", c.header(&format!("=== CSS Rules ({}) ===", stylesheet.rules.len())));
            for (i, rule) in stylesheet.rules.iter().enumerate() {
                if let Rule::Style(sr) = rule {
                    let selectors: Vec<String> = sr
                        .selectors
                        .iter()
                        .map(|s| c.selector(&s.text))
                        .collect();
                    println!(
                        "  {} {} {{ {} }}",
                        c.dim(&format!("{}.", i)),
                        selectors.join(", "),
                        c.number(&format!("{} declarations", sr.declarations.len()))
                    );
                    if options.verbose {
                        for decl in &sr.declarations {
                            println!(
                                "       {}: {}",
                                c.css_prop(&decl.name),
                                c.css_value(&format!("{:?}", decl.value))
                            );
                        }
                    }
                }
            }
            println!();
        }

        compute_styles(&dom, &stylesheet)
    } else {
        if options.css {
            println!("{}", c.header("=== No CSS found ==="));
            println!();
        }
        std::collections::HashMap::new()
    };

    // Output
    if options.json {
        print_json(&dom, &styles);
    } else {
        println!(
            "{}",
            c.header(&format!("=== DOM Tree ({} styled) ===", styles.len()))
        );
        print_tree(&dom, &styles, 0, &c);
    }

    // Flush stdout
    let _ = io::stdout().flush();
}

fn print_usage(program: &str) {
    eprintln!("Koala Browser CLI - HTML/CSS parsing and debugging tool");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  {} <file>              Parse and display DOM tree with styles", program);
    eprintln!("  {} <file> --json       Output DOM as JSON with computed styles", program);
    eprintln!("  {} <file> --tokens     Show HTML tokens", program);
    eprintln!("  {} <file> --css        Show extracted CSS and parsed rules", program);
    eprintln!("  {} <file> --verbose    Show all debugging information", program);
    eprintln!("  {} <file> --no-color   Disable colored output", program);
    eprintln!("  {} --html '<html>'     Parse HTML string directly", program);
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} res/simple.html", program);
    eprintln!("  {} res/simple.html --json", program);
    eprintln!("  {} --html '<h1>Hello</h1>' --css", program);
}

fn print_css_highlighted(css: &str, c: &Colors) {
    // Simple CSS syntax highlighting
    let mut in_value = false;
    let mut in_selector = true;
    let mut buffer = String::new();

    for ch in css.chars() {
        match ch {
            '{' => {
                // End selector, start declarations
                print!("{}", c.selector(&buffer));
                buffer.clear();
                print!("{}", c.dim("{"));
                in_selector = false;
                in_value = false;
            }
            '}' => {
                // End value/property
                if in_value {
                    print!("{}", c.css_value(&buffer));
                } else {
                    print!("{}", c.css_prop(&buffer));
                }
                buffer.clear();
                println!("{}", c.dim("}"));
                in_selector = true;
                in_value = false;
            }
            ':' if !in_selector => {
                // Property name ends
                print!("{}", c.css_prop(&buffer));
                buffer.clear();
                print!("{}", c.dim(":"));
                in_value = true;
            }
            ';' => {
                // Value ends
                print!("{}", c.css_value(&buffer));
                buffer.clear();
                println!("{}", c.dim(";"));
                in_value = false;
            }
            '\n' if in_selector => {
                // Newline in selector area
                if !buffer.trim().is_empty() {
                    print!("{}", c.selector(&buffer));
                }
                buffer.clear();
                println!();
            }
            _ => {
                buffer.push(ch);
            }
        }
    }

    // Print any remaining buffer
    if !buffer.is_empty() {
        print!("{}", buffer);
    }
}

fn print_tree(
    node: &Node,
    styles: &std::collections::HashMap<*const Node, koala::lib_css::css_style::ComputedStyle>,
    depth: usize,
    c: &Colors,
) {
    let indent = "  ".repeat(depth);

    match &node.node_type {
        NodeType::Document => {
            println!("{}{}", indent, c.dim("#document"));
        }
        NodeType::Element(data) => {
            // Build tag with attributes
            let mut tag_str = format!("<{}", data.tag_name);

            if let Some(id) = data.attrs.get("id") {
                tag_str.push_str(&format!(
                    " {}={}",
                    c.attr_name("id"),
                    c.attr_value(&format!("\"{}\"", id))
                ));
            }
            if let Some(class) = data.attrs.get("class") {
                tag_str.push_str(&format!(
                    " {}={}",
                    c.attr_name("class"),
                    c.attr_value(&format!("\"{}\"", class))
                ));
            }
            tag_str.push('>');

            print!("{}{}", indent, c.tag(&tag_str));

            // Show computed styles
            if let Some(style) = styles.get(&(node as *const Node)) {
                let mut parts = Vec::new();

                if let Some(ref clr) = style.color {
                    parts.push(format!("color:#{:02x}{:02x}{:02x}", clr.r, clr.g, clr.b));
                }
                if let Some(ref bg) = style.background_color {
                    parts.push(format!("bg:#{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b));
                }
                if let Some(ref fs) = style.font_size {
                    parts.push(format!("font:{}px", fs.to_px()));
                }
                if let Some(ref p) = style.padding_top {
                    parts.push(format!("pad:{}px", p.to_px()));
                }
                if let Some(ref m) = style.margin_top {
                    parts.push(format!("margin:{}px", m.to_px()));
                }
                if style.border_top.is_some() {
                    parts.push("border".to_string());
                }
                if let Some(lh) = style.line_height {
                    parts.push(format!("lh:{:.1}", lh));
                }

                if !parts.is_empty() {
                    print!(" {}", c.style(&format!("[{}]", parts.join(" "))));
                }
            }
            println!();
        }
        NodeType::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let preview = if trimmed.len() > 60 {
                    format!("{}...", &trimmed[..60])
                } else {
                    trimmed.to_string()
                };
                println!("{}{}", indent, c.text(&format!("\"{}\"", preview)));
            }
        }
        NodeType::Comment(text) => {
            let preview = if text.len() > 30 {
                format!("{}...", &text[..30])
            } else {
                text.to_string()
            };
            println!("{}{}", indent, c.comment(&format!("<!-- {} -->", preview)));
        }
    }

    for child in &node.children {
        print_tree(child, styles, depth + 1, c);
    }
}

fn print_json(
    node: &Node,
    styles: &std::collections::HashMap<*const Node, koala::lib_css::css_style::ComputedStyle>,
) {
    let json = node_to_json(node, styles);
    println!(
        "{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
    );
}

fn node_to_json(
    node: &Node,
    styles: &std::collections::HashMap<*const Node, koala::lib_css::css_style::ComputedStyle>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();

    match &node.node_type {
        NodeType::Document => {
            obj.insert("type".to_string(), serde_json::json!("document"));
        }
        NodeType::Element(data) => {
            obj.insert("type".to_string(), serde_json::json!("element"));
            obj.insert("tagName".to_string(), serde_json::json!(data.tag_name));

            let attrs: serde_json::Map<String, serde_json::Value> = data
                .attrs
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect();
            obj.insert("attributes".to_string(), serde_json::Value::Object(attrs));

            if let Some(style) = styles.get(&(node as *const Node)) {
                if let Ok(style_json) = serde_json::to_value(style) {
                    obj.insert("computedStyle".to_string(), style_json);
                }
            }
        }
        NodeType::Text(text) => {
            obj.insert("type".to_string(), serde_json::json!("text"));
            obj.insert("content".to_string(), serde_json::json!(text));
        }
        NodeType::Comment(text) => {
            obj.insert("type".to_string(), serde_json::json!("comment"));
            obj.insert("content".to_string(), serde_json::json!(text));
        }
    }

    if !node.children.is_empty() {
        let children: Vec<serde_json::Value> = node
            .children
            .iter()
            .map(|child| node_to_json(child, styles))
            .collect();
        obj.insert("children".to_string(), serde_json::Value::Array(children));
    }

    serde_json::Value::Object(obj)
}
