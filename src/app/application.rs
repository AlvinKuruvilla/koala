use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, svg, text, text_input, Column,
    Svg, TextInput,
};
use iced::{Alignment, Border, Color, Element, Length, Padding, Shadow, Theme};
use std::fs::read_to_string;

// ============================================================================
// SVG Icons
// ============================================================================

mod icons {
    use iced::widget::svg;

    pub fn back() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/back.svg").as_slice())
    }

    pub fn forward() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/forward.svg").as_slice())
    }

    pub fn refresh() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/refresh.svg").as_slice())
    }

    pub fn lock() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/lock.svg").as_slice())
    }

    pub fn file() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/file.svg").as_slice())
    }

    pub fn globe() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/globe.svg").as_slice())
    }

    pub fn sun() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/sun.svg").as_slice())
    }

    pub fn moon() -> svg::Handle {
        svg::Handle::from_memory(include_bytes!("../../res/icons/moon.svg").as_slice())
    }
}

use crate::lib_dom::{Node, NodeType};
use crate::lib_html::html_parser::parser::HTMLParser;
use crate::lib_html::html_tokenizer::tokenizer::HTMLTokenizer;

// ============================================================================
// Color Palette - Light and Dark themes
// ============================================================================

#[derive(Clone, Copy)]
pub struct ColorPalette {
    // Backgrounds
    pub toolbar_bg: Color,
    pub content_bg: Color,
    pub url_bar_bg: Color,

    // Borders
    pub border: Color,
    pub border_focus: Color,

    // Text
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,

    // Buttons
    pub button_text: Color,
    pub button_hover_bg: Color,
    pub button_disabled: Color,

    // Security indicators
    pub secure: Color,
    pub local: Color,
}

impl ColorPalette {
    pub const fn light() -> Self {
        Self {
            toolbar_bg: Color::from_rgb(0.96, 0.96, 0.97),
            content_bg: Color::WHITE,
            url_bar_bg: Color::WHITE,
            border: Color::from_rgb(0.88, 0.88, 0.90),
            border_focus: Color::from_rgb(0.4, 0.6, 0.9),
            text_primary: Color::from_rgb(0.15, 0.15, 0.15),
            text_secondary: Color::from_rgb(0.45, 0.45, 0.50),
            text_muted: Color::from_rgb(0.65, 0.65, 0.70),
            button_text: Color::from_rgb(0.35, 0.35, 0.40),
            button_hover_bg: Color::from_rgb(0.92, 0.92, 0.94),
            button_disabled: Color::from_rgb(0.75, 0.75, 0.78),
            secure: Color::from_rgb(0.2, 0.7, 0.3),
            local: Color::from_rgb(0.5, 0.5, 0.55),
        }
    }

    pub const fn dark() -> Self {
        Self {
            toolbar_bg: Color::from_rgb(0.12, 0.12, 0.14),
            content_bg: Color::from_rgb(0.08, 0.08, 0.10),
            url_bar_bg: Color::from_rgb(0.18, 0.18, 0.20),
            border: Color::from_rgb(0.25, 0.25, 0.28),
            border_focus: Color::from_rgb(0.4, 0.6, 0.9),
            text_primary: Color::from_rgb(0.92, 0.92, 0.94),
            text_secondary: Color::from_rgb(0.70, 0.70, 0.75),
            text_muted: Color::from_rgb(0.50, 0.50, 0.55),
            button_text: Color::from_rgb(0.80, 0.80, 0.85),
            button_hover_bg: Color::from_rgb(0.22, 0.22, 0.25),
            button_disabled: Color::from_rgb(0.40, 0.40, 0.45),
            secure: Color::from_rgb(0.3, 0.8, 0.4),
            local: Color::from_rgb(0.55, 0.55, 0.60),
        }
    }
}

// ============================================================================
// Browser State
// ============================================================================

pub struct Browser {
    url: String,
    document: Option<Node>,
    error: Option<String>,
    is_loading: bool,
    dark_mode: bool,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            url: String::new(),
            document: None,
            error: None,
            is_loading: false,
            dark_mode: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    UrlChanged(String),
    Go,
    Back,
    Forward,
    Refresh,
    ToggleDarkMode,
}

impl Browser {
    pub fn title(&self) -> String {
        "Koala".to_string()
    }

    pub fn theme(&self) -> Theme {
        if self.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn palette(&self) -> ColorPalette {
        if self.dark_mode {
            ColorPalette::dark()
        } else {
            ColorPalette::light()
        }
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
            Message::ToggleDarkMode => {
                self.dark_mode = !self.dark_mode;
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
        let p = self.palette();

        // Navigation buttons with SVG icons
        let back_btn = nav_button_svg(icons::back(), Message::Back, false, p);
        let forward_btn = nav_button_svg(icons::forward(), Message::Forward, false, p);
        let refresh_btn = nav_button_svg(icons::refresh(), Message::Refresh, self.document.is_some(), p);

        let nav_buttons = row![back_btn, forward_btn, refresh_btn]
            .spacing(4)
            .align_y(Alignment::Center);

        // Security indicator with SVG
        let security_color = if self.url.starts_with("file://") || !self.url.contains("://") {
            p.local
        } else if self.url.starts_with("https://") {
            p.secure
        } else {
            p.text_muted
        };

        let security_handle = if self.url.starts_with("file://") || !self.url.contains("://") {
            icons::file()
        } else if self.url.starts_with("https://") {
            icons::lock()
        } else {
            icons::globe()
        };

        let security_icon: Element<'_, Message> = svg(security_handle)
            .width(14)
            .height(14)
            .style(move |_theme, _status| svg::Style {
                color: Some(security_color),
            })
            .into();

        // URL bar - pill shaped, takes full width
        let url_input: TextInput<'_, Message> =
            text_input("Enter file path or URL...", &self.url)
                .on_input(Message::UrlChanged)
                .on_submit(Message::Go)
                .padding(Padding::from([6, 8]))
                .size(13)
                .width(Length::Fill)
                .style(move |theme, status| url_input_style(theme, status, p));

        let url_bar = container(
            row![security_icon, url_input]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([6, 14]))
        .width(Length::Fill)
        .style(move |_theme| url_bar_style(p));

        // Dark mode toggle
        let theme_icon = if self.dark_mode {
            icons::sun()
        } else {
            icons::moon()
        };
        let theme_btn = nav_button_svg(theme_icon, Message::ToggleDarkMode, true, p);

        // Assemble toolbar - 48px height standard
        let toolbar_content = row![nav_buttons, url_bar, theme_btn]
            .spacing(12)
            .align_y(Alignment::Center)
            .padding(Padding::from([8, 16]));

        container(toolbar_content)
            .width(Length::Fill)
            .height(48)
            .style(move |_theme| toolbar_style(p))
            .into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        let p = self.palette();

        if let Some(ref error) = self.error {
            // Error state - centered
            container(self.view_error(error))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_theme| content_area_style(p))
                .into()
        } else if let Some(ref doc) = self.document {
            // Page content - normal browser rendering (top-left, scrollable)
            container(scrollable(
                container(render_document(doc, p))
                    .width(Length::Fill)
                    .padding(16),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme| content_area_style(p))
            .into()
        } else {
            // Empty state - centered welcome screen
            container(self.view_empty_state())
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_theme| content_area_style(p))
                .into()
        }
    }

    fn view_empty_state(&self) -> Element<'_, Message> {
        let p = self.palette();

        let icon: Element<'_, Message> = svg(icons::globe())
            .width(64)
            .height(64)
            .style(move |_theme, _status| svg::Style {
                color: Some(p.text_secondary),
            })
            .into();
        let title = text("Welcome to Koala")
            .size(28)
            .color(p.text_primary);
        let subtitle = text("Enter a file path to load HTML")
            .size(16)
            .color(p.text_secondary);
        let hint = text("Try: res/simple.html")
            .size(13)
            .color(p.text_muted);

        column![icon, title, subtitle, hint]
            .spacing(16)
            .align_x(Alignment::Center)
            .into()
    }

    fn view_error(&self, error: &str) -> Element<'_, Message> {
        let p = self.palette();

        let icon = text("⚠").size(36).color(Color::from_rgb(0.9, 0.5, 0.2));
        let title = text("Unable to load page")
            .size(20)
            .color(p.text_primary);
        let message = text(error.to_string()).size(13).color(p.text_secondary);

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

fn toolbar_style(p: ColorPalette) -> container::Style {
    container::Style {
        background: Some(p.toolbar_bg.into()),
        border: Border {
            color: p.border,
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

fn url_bar_style(p: ColorPalette) -> container::Style {
    container::Style {
        background: Some(p.url_bar_bg.into()),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 18.0.into(), // Pill shape
        },
        ..Default::default()
    }
}

fn url_input_style(_theme: &Theme, _status: text_input::Status, p: ColorPalette) -> text_input::Style {
    text_input::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: Color::TRANSPARENT,
        },
        icon: p.text_muted,
        placeholder: p.text_muted,
        value: p.text_primary,
        selection: p.border_focus,
    }
}

fn content_area_style(p: ColorPalette) -> container::Style {
    container::Style {
        background: Some(p.content_bg.into()),
        ..Default::default()
    }
}

fn nav_button_style(status: button::Status, p: ColorPalette) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(p.button_hover_bg.into()),
        button::Status::Pressed => Some(p.border.into()),
        _ => None,
    };

    button::Style {
        background,
        text_color: p.button_text,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn nav_button_disabled_style(p: ColorPalette) -> button::Style {
    button::Style {
        background: None,
        text_color: p.button_disabled,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ============================================================================
// Navigation Button Helper
// ============================================================================

fn nav_button_svg(icon: svg::Handle, msg: Message, enabled: bool, p: ColorPalette) -> Element<'static, Message> {
    let icon_color = if enabled {
        p.button_text
    } else {
        p.button_disabled
    };

    let icon_widget: Svg<'static, Theme> = svg(icon).width(18).height(18).style(
        move |_theme: &Theme, _status| svg::Style {
            color: Some(icon_color),
        },
    );

    let btn = button(
        container(icon_widget)
            .width(28)
            .height(28)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .padding(4);

    if enabled {
        btn.style(move |_theme, status| nav_button_style(status, p))
            .on_press(msg)
            .into()
    } else {
        btn.style(move |_theme, _status| nav_button_disabled_style(p))
            .into()
    }
}

// ============================================================================
// DOM Rendering
// ============================================================================

fn render_document<'a>(node: &Node, p: ColorPalette) -> Element<'a, Message> {
    // Render like a normal browser - content flows from top-left
    render_node(node, p)
}

fn render_node<'a>(node: &Node, p: ColorPalette) -> Element<'a, Message> {
    match &node.node_type {
        NodeType::Document => render_children(node, p),
        NodeType::Element(data) => {
            let tag = data.tag_name.as_str();
            match tag {
                // Headings
                "h1" => render_heading(node, 32, 24, 16, p),
                "h2" => render_heading(node, 28, 20, 14, p),
                "h3" => render_heading(node, 24, 18, 12, p),
                "h4" => render_heading(node, 20, 16, 10, p),
                "h5" => render_heading(node, 18, 14, 8, p),
                "h6" => render_heading(node, 16, 12, 6, p),

                // Block elements
                "html" | "body" | "div" | "article" | "section" | "main" | "header" | "footer"
                | "nav" | "aside" => render_block(node, p),

                // Don't render head contents
                "head" | "title" | "meta" | "link" | "script" | "style" => column![].into(),

                // Paragraph
                "p" => render_paragraph(node, p),

                // Inline
                "span" | "a" => render_children(node, p),

                // Emphasis (simulated)
                "b" | "strong" => render_text_styled(node, p.text_primary),
                "i" | "em" => render_text_styled(node, p.text_secondary),

                // Void elements
                "br" => text("\n").into(),
                "hr" => render_hr(p),
                "input" | "img" => column![].into(),

                // Unknown - render children
                _ => render_children(node, p),
            }
        }
        NodeType::Text(data) => {
            let normalized = normalize_whitespace(data);
            if normalized.is_empty() {
                column![].into()
            } else {
                text(normalized)
                    .size(16)
                    .color(p.text_primary)
                    .into()
            }
        }
        NodeType::Comment(_) => column![].into(),
    }
}

fn render_children<'a>(node: &Node, p: ColorPalette) -> Element<'a, Message> {
    let children: Vec<Element<'a, Message>> = node.children.iter().map(|n| render_node(n, p)).collect();

    if children.is_empty() {
        column![].into()
    } else {
        Column::with_children(children).into()
    }
}

fn render_block<'a>(node: &Node, p: ColorPalette) -> Element<'a, Message> {
    let children: Vec<Element<'a, Message>> = node.children.iter().map(|n| render_node(n, p)).collect();
    Column::with_children(children).spacing(8).into()
}

fn render_paragraph<'a>(node: &Node, p: ColorPalette) -> Element<'a, Message> {
    let content = get_text_content(node);
    container(
        text(content)
            .size(16)
            .color(p.text_primary)
            .line_height(1.6),
    )
    .padding(Padding::from([8, 0]))
    .into()
}

fn render_heading<'a>(node: &Node, size: u16, top: u16, bottom: u16, p: ColorPalette) -> Element<'a, Message> {
    let content = get_text_content(node);
    container(text(content).size(size).color(p.text_primary))
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

fn render_hr<'a>(p: ColorPalette) -> Element<'a, Message> {
    container(horizontal_space())
        .width(Length::Fill)
        .height(1)
        .style(move |_| container::Style {
            background: Some(p.border.into()),
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
