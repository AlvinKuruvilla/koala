use iced::widget::{column, container, row, scrollable, text, Button, TextInput};
use iced::{Alignment, Color, Element, Length};
use std::fs::read_to_string;

use crate::lib_dom::{Node, NodeType};
use crate::lib_html::html_parser::parser::HTMLParser;
use crate::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

#[derive(Default)]
pub struct Browser {
    url: String,
    document: Option<Node>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    UrlChanged(String),
    Go,
    Back,
    Forward,
}

impl Browser {
    pub fn title(&self) -> String {
        "Koala Browser".to_string()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::UrlChanged(new_url) => {
                self.url = new_url;
            }
            Message::Go => {
                self.load_page();
            }
            Message::Back => {
                println!("Back button clicked");
            }
            Message::Forward => {
                println!("Forward button clicked");
            }
        }
    }

    fn load_page(&mut self) {
        self.error = None;
        self.document = None;

        // For now, treat URL as a file path
        let path = if self.url.starts_with("file://") {
            self.url.strip_prefix("file://").unwrap_or(&self.url)
        } else {
            &self.url
        };

        match read_to_string(path) {
            Ok(html) => {
                let mut tokenizer = HTMLTokenizer::new(html);
                tokenizer.run();
                let parser = HTMLParser::new(tokenizer.into_tokens());
                self.document = Some(parser.run());
            }
            Err(e) => {
                self.error = Some(format!("Failed to load: {}", e));
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let back_button = Button::new("←").on_press(Message::Back).padding(10);
        let forward_button = Button::new("→").on_press(Message::Forward).padding(10);
        let go_button = Button::new("Go").on_press(Message::Go).padding(10);

        let url_input = TextInput::new("Enter file path...", &self.url)
            .on_input(Message::UrlChanged)
            .on_submit(Message::Go)
            .padding(10)
            .size(16)
            .width(Length::Fill);

        let toolbar = row![back_button, forward_button, url_input, go_button]
            .spacing(5)
            .align_y(Alignment::Center)
            .padding(10);

        // Render content area
        let content: Element<'_, Message> = if let Some(ref error) = self.error {
            text(error).color(Color::from_rgb(0.8, 0.2, 0.2)).into()
        } else if let Some(ref doc) = self.document {
            render_node(doc)
        } else {
            text("Enter a file path and press Go to load HTML")
                .color(Color::from_rgb(0.5, 0.5, 0.5))
                .into()
        };

        let content_area = container(scrollable(
            container(content).width(Length::Fill).padding(20),
        ))
        .width(Length::Fill)
        .height(Length::Fill);

        column![toolbar, content_area].into()
    }
}

/// Render a DOM node as an iced Element
fn render_node<'a>(node: &Node) -> Element<'a, Message> {
    match &node.node_type {
        NodeType::Document => {
            // Render all children
            render_children(node)
        }
        NodeType::Element(data) => {
            let tag = data.tag_name.as_str();
            match tag {
                // Headings with different sizes
                "h1" => render_heading(node, 32.0),
                "h2" => render_heading(node, 28.0),
                "h3" => render_heading(node, 24.0),
                "h4" => render_heading(node, 20.0),
                "h5" => render_heading(node, 18.0),
                "h6" => render_heading(node, 16.0),

                // Block elements
                "html" | "body" | "div" | "article" | "section" | "main" | "header" | "footer"
                | "nav" | "aside" => render_block(node),

                "head" => {
                    // Don't render head contents visually (except title for now)
                    column![].into()
                }

                "title" => {
                    // Could update window title, but for now skip
                    column![].into()
                }

                "p" => render_paragraph(node),

                // Inline elements - just render children
                "span" | "a" => render_children(node),

                // Bold/strong
                "b" | "strong" => render_bold(node),

                // Italic/emphasis
                "i" | "em" => render_italic(node),

                // Line break
                "br" => text("\n").into(),

                // Horizontal rule
                "hr" => container(text(""))
                    .width(Length::Fill)
                    .height(1)
                    .style(|_| container::Style {
                        background: Some(Color::from_rgb(0.7, 0.7, 0.7).into()),
                        ..Default::default()
                    })
                    .into(),

                // Void elements that don't display content
                "meta" | "link" | "script" | "style" | "input" | "img" => column![].into(),

                // Unknown elements - just render children
                _ => render_children(node),
            }
        }
        NodeType::Text(data) => {
            // Collapse whitespace for display (basic normalization)
            let normalized = normalize_whitespace(data);
            if normalized.is_empty() {
                column![].into()
            } else {
                text(normalized).into()
            }
        }
        NodeType::Comment(_) => {
            // Don't render comments
            column![].into()
        }
    }
}

/// Render children of a node in a column
fn render_children<'a>(node: &Node) -> Element<'a, Message> {
    let children: Vec<Element<'a, Message>> = node
        .children
        .iter()
        .map(render_node)
        .collect();

    if children.is_empty() {
        column![].into()
    } else {
        column(children).into()
    }
}

/// Render a block-level element
fn render_block<'a>(node: &Node) -> Element<'a, Message> {
    let children: Vec<Element<'a, Message>> = node
        .children
        .iter()
        .map(render_node)
        .collect();

    column(children).spacing(5).into()
}

/// Render a paragraph with margin
fn render_paragraph<'a>(node: &Node) -> Element<'a, Message> {
    let content = render_children(node);
    container(content).padding([10, 0]).into()
}

/// Render a heading with specific size
fn render_heading<'a>(node: &Node, size: f32) -> Element<'a, Message> {
    let text_content = get_text_content(node);
    container(text(text_content).size(size as u16))
        .padding([10, 0])
        .into()
}

/// Render bold text
fn render_bold<'a>(node: &Node) -> Element<'a, Message> {
    let text_content = get_text_content(node);
    // iced doesn't have built-in bold, but we can use a slightly larger size
    // or different color to indicate emphasis
    text(text_content)
        .color(Color::from_rgb(0.1, 0.1, 0.1))
        .into()
}

/// Render italic text
fn render_italic<'a>(node: &Node) -> Element<'a, Message> {
    let text_content = get_text_content(node);
    // iced doesn't have built-in italic, use a slightly different color
    text(text_content)
        .color(Color::from_rgb(0.3, 0.3, 0.3))
        .into()
}

/// Get concatenated text content of a node
fn get_text_content(node: &Node) -> String {
    let mut result = String::new();
    collect_text(node, &mut result);
    normalize_whitespace(&result)
}

fn collect_text(node: &Node, result: &mut String) {
    match &node.node_type {
        NodeType::Text(data) => result.push_str(data),
        _ => {
            for child in &node.children {
                collect_text(child, result);
            }
        }
    }
}

/// Normalize whitespace: collapse runs of whitespace into single spaces
fn normalize_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut last_was_space = true; // Start true to trim leading whitespace

    for c in s.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    // Trim trailing space
    if result.ends_with(' ') {
        result.pop();
    }

    result
}
