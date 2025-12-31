use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, text, text_input, Column,
    TextInput,
};
use iced::{Alignment, Border, Color, Element, Length, Padding, Shadow, Theme};
use std::fs::read_to_string;

use crate::lib_dom::{Node, NodeType};
use crate::lib_html::html_parser::parser::HTMLParser;
use crate::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

// ============================================================================
// Color Palette - Minimal, clean design
// ============================================================================

mod colors {
    use iced::Color;

    // Backgrounds
    pub const TOOLBAR_BG: Color = Color::from_rgb(0.96, 0.96, 0.97); // Light gray
    pub const CONTENT_BG: Color = Color::WHITE;
    pub const URL_BAR_BG: Color = Color::WHITE;

    // Borders
    pub const BORDER_LIGHT: Color = Color::from_rgb(0.88, 0.88, 0.90);
    pub const BORDER_FOCUS: Color = Color::from_rgb(0.4, 0.6, 0.9);

    // Text
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.15, 0.15, 0.15);
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.45, 0.45, 0.50);
    pub const TEXT_MUTED: Color = Color::from_rgb(0.65, 0.65, 0.70);

    // Buttons
    pub const BUTTON_TEXT: Color = Color::from_rgb(0.35, 0.35, 0.40);
    pub const BUTTON_HOVER_BG: Color = Color::from_rgb(0.92, 0.92, 0.94);
    pub const BUTTON_DISABLED: Color = Color::from_rgb(0.75, 0.75, 0.78);

    // Security indicators
    pub const SECURE: Color = Color::from_rgb(0.2, 0.7, 0.3); // Green
    pub const LOCAL: Color = Color::from_rgb(0.5, 0.5, 0.55); // Gray
}

// ============================================================================
// Browser State
// ============================================================================

#[derive(Default)]
pub struct Browser {
    url: String,
    document: Option<Node>,
    error: Option<String>,
    is_loading: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    UrlChanged(String),
    Go,
    Back,
    Forward,
    Refresh,
}

impl Browser {
    pub fn title(&self) -> String {
        "Koala".to_string()
    }

    pub fn theme(&self) -> Theme {
        Theme::Light
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::UrlChanged(new_url) => {
                self.url = new_url;
            }
            Message::Go => {
                self.load_page();
            }
            Message::Refresh => {
                self.load_page();
            }
            Message::Back => {
                // TODO: History navigation
            }
            Message::Forward => {
                // TODO: History navigation
            }
        }
    }

    fn load_page(&mut self) {
        self.error = None;
        self.document = None;
        self.is_loading = true;

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

        self.is_loading = false;
    }

    pub fn view(&self) -> Element<'_, Message> {
        let toolbar = self.view_toolbar();
        let content = self.view_content();

        column![toolbar, content].into()
    }

    fn view_toolbar(&self) -> Element<'_, Message> {
        // Navigation buttons
        let back_btn = nav_button("◀", Message::Back, false);
        let forward_btn = nav_button("▶", Message::Forward, false);
        let refresh_btn = nav_button("↻", Message::Refresh, self.document.is_some());

        let nav_buttons = row![back_btn, forward_btn, refresh_btn]
            .spacing(2)
            .align_y(Alignment::Center);

        // Security indicator
        let security_icon = if self.url.starts_with("file://") || !self.url.contains("://") {
            text("📄").size(14) // Local file
        } else if self.url.starts_with("https://") {
            text("🔒").size(14).color(colors::SECURE) // Secure
        } else {
            text("ⓘ").size(14).color(colors::TEXT_MUTED) // Info/unknown
        };

        // URL bar
        let url_input: TextInput<'_, Message> =
            text_input("Enter file path or URL...", &self.url)
                .on_input(Message::UrlChanged)
                .on_submit(Message::Go)
                .padding(Padding::from([8, 12]))
                .size(14)
                .width(Length::Fill)
                .style(url_input_style);

        let url_bar = container(
            row![security_icon, url_input]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([4, 12]))
        .width(Length::Fill)
        .style(url_bar_style);

        // Go button
        let go_btn = button(text("Go").size(13).color(colors::TEXT_PRIMARY))
            .padding(Padding::from([8, 16]))
            .style(go_button_style)
            .on_press(Message::Go);

        // Assemble toolbar
        let toolbar_content = row![nav_buttons, url_bar, go_btn]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding(Padding::from([8, 12]));

        container(toolbar_content)
            .width(Length::Fill)
            .style(toolbar_style)
            .into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        if let Some(ref error) = self.error {
            // Error state - centered
            container(self.view_error(error))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(content_area_style)
                .into()
        } else if let Some(ref doc) = self.document {
            // Page content - normal browser rendering (top-left, scrollable)
            container(scrollable(
                container(render_document(doc))
                    .width(Length::Fill)
                    .padding(16),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(content_area_style)
            .into()
        } else {
            // Empty state - centered welcome screen
            container(self.view_empty_state())
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(content_area_style)
                .into()
        }
    }

    fn view_empty_state(&self) -> Element<'_, Message> {
        let icon = text("🌐").size(64);
        let title = text("Welcome to Koala")
            .size(28)
            .color(colors::TEXT_PRIMARY);
        let subtitle = text("Enter a file path to load HTML")
            .size(16)
            .color(colors::TEXT_SECONDARY);
        let hint = text("Try: res/simple.html")
            .size(13)
            .color(colors::TEXT_MUTED);

        column![icon, title, subtitle, hint]
            .spacing(16)
            .align_x(Alignment::Center)
            .into()
    }

    fn view_error(&self, error: &str) -> Element<'_, Message> {
        let icon = text("⚠").size(36).color(Color::from_rgb(0.9, 0.5, 0.2));
        let title = text("Unable to load page")
            .size(20)
            .color(colors::TEXT_PRIMARY);
        let message = text(error.to_string()).size(13).color(colors::TEXT_SECONDARY);

        column![icon, title, message]
            .spacing(12)
            .align_x(Alignment::Center)
            .padding(Padding::from([80, 40]))
            .into()
    }
}

// ============================================================================
// Custom Styles
// ============================================================================

fn toolbar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors::TOOLBAR_BG.into()),
        border: Border {
            color: colors::BORDER_LIGHT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
            offset: iced::Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..Default::default()
    }
}

fn url_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors::URL_BAR_BG.into()),
        border: Border {
            color: colors::BORDER_LIGHT,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

fn url_input_style(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: Color::TRANSPARENT,
        },
        icon: colors::TEXT_MUTED,
        placeholder: colors::TEXT_MUTED,
        value: colors::TEXT_PRIMARY,
        selection: colors::BORDER_FOCUS,
    }
}

fn content_area_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors::CONTENT_BG.into()),
        ..Default::default()
    }
}

fn nav_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(colors::BUTTON_HOVER_BG.into()),
        button::Status::Pressed => Some(colors::BORDER_LIGHT.into()),
        _ => None,
    };

    button::Style {
        background,
        text_color: colors::BUTTON_TEXT,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn nav_button_disabled_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: colors::BUTTON_DISABLED,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn go_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(colors::BUTTON_HOVER_BG.into()),
        button::Status::Pressed => Some(colors::BORDER_LIGHT.into()),
        _ => Some(colors::TOOLBAR_BG.into()),
    };

    button::Style {
        background,
        text_color: colors::TEXT_PRIMARY,
        border: Border {
            color: colors::BORDER_LIGHT,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

// ============================================================================
// Navigation Button Helper
// ============================================================================

fn nav_button(label: &str, msg: Message, enabled: bool) -> Element<'_, Message> {
    let btn = button(
        text(label)
            .size(16)
            .color(if enabled {
                colors::BUTTON_TEXT
            } else {
                colors::BUTTON_DISABLED
            })
            .width(Length::Fixed(24.0))
            .align_x(Alignment::Center),
    )
    .padding(Padding::from([6, 4]));

    if enabled {
        btn.style(nav_button_style).on_press(msg).into()
    } else {
        btn.style(nav_button_disabled_style).into()
    }
}

// ============================================================================
// DOM Rendering
// ============================================================================

fn render_document<'a>(node: &Node) -> Element<'a, Message> {
    // Render like a normal browser - content flows from top-left
    render_node(node)
}

fn render_node<'a>(node: &Node) -> Element<'a, Message> {
    match &node.node_type {
        NodeType::Document => render_children(node),
        NodeType::Element(data) => {
            let tag = data.tag_name.as_str();
            match tag {
                // Headings
                "h1" => render_heading(node, 32, 24, 16),
                "h2" => render_heading(node, 28, 20, 14),
                "h3" => render_heading(node, 24, 18, 12),
                "h4" => render_heading(node, 20, 16, 10),
                "h5" => render_heading(node, 18, 14, 8),
                "h6" => render_heading(node, 16, 12, 6),

                // Block elements
                "html" | "body" | "div" | "article" | "section" | "main" | "header" | "footer"
                | "nav" | "aside" => render_block(node),

                // Don't render head contents
                "head" | "title" | "meta" | "link" | "script" | "style" => column![].into(),

                // Paragraph
                "p" => render_paragraph(node),

                // Inline
                "span" | "a" => render_children(node),

                // Emphasis (simulated)
                "b" | "strong" => render_text_styled(node, colors::TEXT_PRIMARY),
                "i" | "em" => render_text_styled(node, colors::TEXT_SECONDARY),

                // Void elements
                "br" => text("\n").into(),
                "hr" => render_hr(),
                "input" | "img" => column![].into(),

                // Unknown - render children
                _ => render_children(node),
            }
        }
        NodeType::Text(data) => {
            let normalized = normalize_whitespace(data);
            if normalized.is_empty() {
                column![].into()
            } else {
                text(normalized)
                    .size(16)
                    .color(colors::TEXT_PRIMARY)
                    .into()
            }
        }
        NodeType::Comment(_) => column![].into(),
    }
}

fn render_children<'a>(node: &Node) -> Element<'a, Message> {
    let children: Vec<Element<'a, Message>> = node.children.iter().map(render_node).collect();

    if children.is_empty() {
        column![].into()
    } else {
        Column::with_children(children).into()
    }
}

fn render_block<'a>(node: &Node) -> Element<'a, Message> {
    let children: Vec<Element<'a, Message>> = node.children.iter().map(render_node).collect();
    Column::with_children(children).spacing(8).into()
}

fn render_paragraph<'a>(node: &Node) -> Element<'a, Message> {
    let content = get_text_content(node);
    container(
        text(content)
            .size(16)
            .color(colors::TEXT_PRIMARY)
            .line_height(1.6),
    )
    .padding(Padding::from([8, 0]))
    .into()
}

fn render_heading<'a>(node: &Node, size: u16, top: u16, bottom: u16) -> Element<'a, Message> {
    let content = get_text_content(node);
    container(text(content).size(size).color(colors::TEXT_PRIMARY))
        .padding(Padding {
            top: top.into(),
            right: 0.0,
            bottom: bottom.into(),
            left: 0.0,
        })
        .into()
}

fn render_text_styled<'a>(node: &Node, color: Color) -> Element<'a, Message> {
    let content = get_text_content(node);
    text(content).size(16).color(color).into()
}

fn render_hr<'a>() -> Element<'a, Message> {
    container(horizontal_space())
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(colors::BORDER_LIGHT.into()),
            ..Default::default()
        })
        .padding(Padding::from([16, 0]))
        .into()
}

// ============================================================================
// Text Utilities
// ============================================================================

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

fn normalize_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut last_was_space = true;

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

    if result.ends_with(' ') {
        result.pop();
    }

    result
}
