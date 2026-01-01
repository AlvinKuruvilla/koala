//! Koala Browser GUI - egui-based browser interface
//!
//! Run with: cargo run --bin koala
//!
//! Debug features:
//! - F12: Toggle debug panel
//! - All state changes logged to terminal

use std::collections::HashMap;
use std::fs;

use eframe::egui;
use koala_css::{canvas_background, compute_styles, extract_style_content, ComputedStyle, CSSParser, CSSTokenizer};
use koala_dom::{DomTree, NodeId, NodeType};
use koala_html::{HTMLParser, HTMLTokenizer, Token};

fn main() -> eframe::Result<()> {
    println!("[Koala GUI] Starting browser...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Koala Browser",
        options,
        Box::new(|cc| Ok(Box::new(BrowserApp::new(&cc.egui_ctx)))),
    )
}

/// Application theme
#[derive(Debug, Clone, Copy, PartialEq)]
enum Theme {
    Light,
    Dark,
}

impl Theme {
    fn toggle(&self) -> Self {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    fn visuals(&self) -> egui::Visuals {
        match self {
            Theme::Light => {
                let mut visuals = egui::Visuals::light();
                visuals.window_rounding = egui::Rounding::same(8.0);
                visuals.widgets.noninteractive.rounding = egui::Rounding::same(4.0);
                visuals.widgets.inactive.rounding = egui::Rounding::same(4.0);
                visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);
                visuals.widgets.active.rounding = egui::Rounding::same(4.0);
                visuals
            }
            Theme::Dark => {
                let mut visuals = egui::Visuals::dark();
                visuals.window_rounding = egui::Rounding::same(8.0);
                visuals.widgets.noninteractive.rounding = egui::Rounding::same(4.0);
                visuals.widgets.inactive.rounding = egui::Rounding::same(4.0);
                visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);
                visuals.widgets.active.rounding = egui::Rounding::same(4.0);
                visuals
            }
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Theme::Light => "🌙",
            Theme::Dark => "☀️",
        }
    }
}

/// Quick link for the landing page
struct QuickLink {
    name: &'static str,
    url: &'static str,
    icon: &'static str,
}

const QUICK_LINKS: &[QuickLink] = &[
    QuickLink {
        name: "Example.com",
        url: "https://example.com",
        icon: "🌐",
    },
    QuickLink {
        name: "Test Page",
        url: "res/simple.html",
        icon: "📄",
    },
    QuickLink {
        name: "Test with Styles",
        url: "res/test.html",
        icon: "🎨",
    },
];

/// Browser application state
struct BrowserApp {
    /// Current URL/path in the URL bar
    url_input: String,

    /// History of visited URLs for back/forward
    history: Vec<String>,
    history_index: usize,

    /// Current page state
    page: Option<PageState>,

    /// Is the debug panel visible?
    debug_panel_open: bool,

    /// Which debug tab is selected
    debug_tab: DebugTab,

    /// Status message shown in status bar
    status_message: String,

    /// Current theme
    theme: Theme,
}

/// Parsed page state
struct PageState {
    /// Original HTML source
    html_source: String,

    /// Path/URL this was loaded from
    source_path: String,

    /// HTML tokens (for debugging)
    tokens: Vec<Token>,

    /// Parsed DOM tree
    dom: DomTree,

    /// Extracted CSS text
    css_text: String,

    /// Computed styles per node
    styles: HashMap<NodeId, ComputedStyle>,

    /// Any parse errors/warnings
    parse_issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DebugTab {
    Dom,
    Tokens,
    Css,
    Styles,
    Source,
}

impl BrowserApp {
    fn new(ctx: &egui::Context) -> Self {
        let theme = Theme::Dark;
        ctx.set_visuals(theme.visuals());
        println!("[Koala GUI] Browser initialized with {:?} theme", theme);

        Self {
            url_input: String::new(),
            history: Vec::new(),
            history_index: 0,
            page: None,
            debug_panel_open: false,
            debug_tab: DebugTab::Dom,
            status_message: "Welcome to Koala Browser".to_string(),
            theme,
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.theme = theme;
        ctx.set_visuals(theme.visuals());
        println!("[Koala GUI] Theme changed to {:?}", theme);
    }

    /// Navigate to a URL/path
    fn navigate(&mut self, path: &str) {
        println!("[Koala GUI] Navigating to: {}", path);
        self.status_message = format!("Loading {}...", path);

        match self.load_page(path) {
            Ok(page) => {
                println!("[Koala GUI] Page loaded successfully");
                println!("[Koala GUI]   - {} tokens", page.tokens.len());
                println!("[Koala GUI]   - {} DOM nodes", page.dom.len());
                println!("[Koala GUI]   - {} styled nodes", page.styles.len());
                println!("[Koala GUI]   - {} bytes CSS", page.css_text.len());

                if !page.parse_issues.is_empty() {
                    println!("[Koala GUI]   - {} parse issues:", page.parse_issues.len());
                    for issue in &page.parse_issues {
                        println!("[Koala GUI]     ! {}", issue);
                    }
                }

                // Update history
                if self.history_index < self.history.len() {
                    self.history.truncate(self.history_index);
                }
                self.history.push(path.to_string());
                self.history_index = self.history.len();

                self.url_input = path.to_string();
                self.page = Some(page);
                self.status_message = format!("Loaded: {}", path);
            }
            Err(e) => {
                println!("[Koala GUI] ERROR loading page: {}", e);
                self.status_message = format!("Error: {}", e);
                self.page = None;
            }
        }
    }

    /// Load and parse a page from a file path or URL
    fn load_page(&self, path: &str) -> Result<PageState, String> {
        let html_source = if path.starts_with("http://") || path.starts_with("https://") {
            self.fetch_url(path)?
        } else {
            fs::read_to_string(path).map_err(|e| format!("Failed to read '{}': {}", path, e))?
        };

        println!("[Koala GUI] Parsing {} bytes of HTML", html_source.len());

        // Tokenize
        let mut tokenizer = HTMLTokenizer::new(html_source.clone());
        tokenizer.run();
        let tokens = tokenizer.into_tokens();
        println!("[Koala GUI] Tokenized: {} tokens", tokens.len());

        // Parse
        let parser = HTMLParser::new(tokens.clone());
        let (dom, issues) = parser.run_with_issues();
        let parse_issues: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();
        println!("[Koala GUI] Parsed: {} nodes", dom.len());

        // Extract and parse CSS
        let css_text = extract_style_content(&dom);
        let styles = if !css_text.is_empty() {
            println!("[Koala GUI] Parsing {} bytes of CSS", css_text.len());
            let mut css_tokenizer = CSSTokenizer::new(css_text.clone());
            css_tokenizer.run();
            let mut css_parser = CSSParser::new(css_tokenizer.into_tokens());
            let stylesheet = css_parser.parse_stylesheet();
            println!("[Koala GUI] CSS: {} rules", stylesheet.rules.len());
            compute_styles(&dom, &stylesheet)
        } else {
            HashMap::new()
        };

        Ok(PageState {
            html_source,
            source_path: path.to_string(),
            tokens,
            dom,
            css_text,
            styles,
            parse_issues,
        })
    }

    /// Fetch HTML from a URL using curl
    fn fetch_url(&self, url: &str) -> Result<String, String> {
        println!("[Koala GUI] Fetching URL: {}", url);
        let output = std::process::Command::new("curl")
            .args(["-sL", "--max-time", "10", url])
            .output()
            .map_err(|e| format!("Failed to run curl: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "curl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {}", e))
    }

    fn go_back(&mut self) {
        if self.history_index > 1 {
            self.history_index -= 1;
            let path = self.history[self.history_index - 1].clone();
            println!("[Koala GUI] Going back to: {}", path);
            self.url_input = path.clone();
            if let Ok(page) = self.load_page(&path) {
                self.page = Some(page);
                self.status_message = format!("Loaded: {}", path);
            }
        }
    }

    fn go_forward(&mut self) {
        if self.history_index < self.history.len() {
            self.history_index += 1;
            let path = self.history[self.history_index - 1].clone();
            println!("[Koala GUI] Going forward to: {}", path);
            self.url_input = path.clone();
            if let Ok(page) = self.load_page(&path) {
                self.page = Some(page);
                self.status_message = format!("Loaded: {}", path);
            }
        }
    }

    fn refresh(&mut self) {
        if let Some(page) = &self.page {
            let path = page.source_path.clone();
            println!("[Koala GUI] Refreshing: {}", path);
            self.navigate(&path);
        }
    }

    fn go_home(&mut self) {
        self.page = None;
        self.url_input.clear();
        self.status_message = "Welcome to Koala Browser".to_string();
        println!("[Koala GUI] Returned to home");
    }

    fn can_go_back(&self) -> bool {
        self.history_index > 1
    }

    fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len()
    }
}

impl eframe::App for BrowserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.debug_panel_open = !self.debug_panel_open;
            println!(
                "[Koala GUI] Debug panel: {}",
                if self.debug_panel_open { "OPEN" } else { "CLOSED" }
            );
        }

        // Top panel: Navigation bar
        let _ = egui::TopBottomPanel::top("nav_bar")
            .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(12.0, 8.0)))
            .show(ctx, |ui| {
                let _ = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Navigation buttons with consistent styling
                    let button_size = egui::vec2(32.0, 28.0);

                    if ui
                        .add_enabled(
                            self.can_go_back(),
                            egui::Button::new("◀").min_size(button_size),
                        )
                        .on_hover_text("Back")
                        .clicked()
                    {
                        self.go_back();
                    }

                    if ui
                        .add_enabled(
                            self.can_go_forward(),
                            egui::Button::new("▶").min_size(button_size),
                        )
                        .on_hover_text("Forward")
                        .clicked()
                    {
                        self.go_forward();
                    }

                    if ui
                        .add_enabled(
                            self.page.is_some(),
                            egui::Button::new("↻").min_size(button_size),
                        )
                        .on_hover_text("Refresh")
                        .clicked()
                    {
                        self.refresh();
                    }

                    if ui
                        .add(egui::Button::new("🏠").min_size(button_size))
                        .on_hover_text("Home")
                        .clicked()
                    {
                        self.go_home();
                    }

                    ui.add_space(8.0);

                    // URL bar with rounded frame
                    let url_bar_width = ui.available_width() - 100.0;
                    let _ = egui::Frame::none()
                        .fill(ui.visuals().extreme_bg_color)
                        .rounding(egui::Rounding::same(14.0))
                        .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_width(url_bar_width);
                            let response = ui.add_sized(
                                [url_bar_width - 24.0, 20.0],
                                egui::TextEdit::singleline(&mut self.url_input)
                                    .hint_text("Enter file path or URL...")
                                    .frame(false)
                                    .font(egui::TextStyle::Body),
                            );

                            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let path = self.url_input.clone();
                                self.navigate(&path);
                            }
                        });

                    ui.add_space(8.0);

                    // Theme toggle
                    if ui
                        .add(egui::Button::new(self.theme.icon()).min_size(button_size))
                        .on_hover_text("Toggle theme")
                        .clicked()
                    {
                        let new_theme = self.theme.toggle();
                        self.set_theme(ctx, new_theme);
                    }

                    // Debug toggle
                    let debug_button = if self.debug_panel_open {
                        egui::Button::new("🔧").fill(ui.visuals().selection.bg_fill)
                    } else {
                        egui::Button::new("🔧")
                    };
                    if ui
                        .add(debug_button.min_size(button_size))
                        .on_hover_text("Toggle debug panel (F12)")
                        .clicked()
                    {
                        self.debug_panel_open = !self.debug_panel_open;
                        println!(
                            "[Koala GUI] Debug panel: {}",
                            if self.debug_panel_open { "OPEN" } else { "CLOSED" }
                        );
                    }
                });
            });

        // Bottom panel: Status bar
        let _ = egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(12.0, 4.0)))
            .show(ctx, |ui| {
                let _ = ui.horizontal(|ui| {
                    let _ = ui.label(
                        egui::RichText::new(&self.status_message)
                            .size(12.0)
                            .color(ui.visuals().text_color().gamma_multiply(0.7)),
                    );
                    let _ = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(page) = &self.page {
                            let _ = ui.label(
                                egui::RichText::new(format!(
                                    "{} nodes • {} styled",
                                    page.dom.len(),
                                    page.styles.len()
                                ))
                                .size(12.0)
                                .color(ui.visuals().text_color().gamma_multiply(0.7)),
                            );
                        }
                    });
                });
            });

        // Right panel: Debug panel (if open)
        if self.debug_panel_open {
            let _ = egui::SidePanel::right("debug_panel")
                .min_width(350.0)
                .default_width(400.0)
                .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::same(12.0)))
                .show(ctx, |ui| {
                    let _ = ui.heading("Debug Panel");
                    ui.add_space(8.0);

                    // Tab bar
                    let _ = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (tab, label) in [
                            (DebugTab::Dom, "DOM"),
                            (DebugTab::Tokens, "Tokens"),
                            (DebugTab::Css, "CSS"),
                            (DebugTab::Styles, "Styles"),
                            (DebugTab::Source, "Source"),
                        ] {
                            if ui.selectable_label(self.debug_tab == tab, label).clicked() {
                                self.debug_tab = tab;
                            }
                        }
                    });
                    let _ = ui.separator();
                    ui.add_space(4.0);

                    // Tab content
                    let _ = egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Some(page) = &self.page {
                            match self.debug_tab {
                                DebugTab::Dom => self.render_debug_dom(ui, page),
                                DebugTab::Tokens => self.render_debug_tokens(ui, page),
                                DebugTab::Css => self.render_debug_css(ui, page),
                                DebugTab::Styles => self.render_debug_styles(ui, page),
                                DebugTab::Source => self.render_debug_source(ui, page),
                            }
                        } else {
                            let _ = ui.colored_label(
                                ui.visuals().text_color().gamma_multiply(0.5),
                                "No page loaded",
                            );
                        }
                    });
                });
        }

        // Central panel: Page content or landing page
        let _ = egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| {
                if let Some(page) = &self.page {
                    // Page content
                    // [§ 2.11.2 The Canvas Background](https://www.w3.org/TR/css-backgrounds-3/#special-backgrounds)
                    let fill_color = canvas_background(&page.dom, &page.styles)
                        .map(|c| egui::Color32::from_rgb(c.r, c.g, c.b))
                        .unwrap_or_else(|| {
                            if self.theme == Theme::Dark {
                                egui::Color32::from_rgb(28, 28, 30)
                            } else {
                                egui::Color32::WHITE
                            }
                        });
                    let _ = egui::Frame::none()
                        .fill(fill_color)
                        .inner_margin(egui::Margin::same(24.0))
                        .show(ui, |ui| {
                            let _ = egui::ScrollArea::vertical().show(ui, |ui| {
                                self.render_page_content(ui, page);
                            });
                        });
                } else {
                    // Landing page
                    self.render_landing_page(ui);
                }
            });
    }
}

impl BrowserApp {
    /// Render the landing page
    fn render_landing_page(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();

        // Center content vertically
        ui.add_space((available.y / 2.0 - 150.0).max(50.0));

        let _ = ui.vertical_centered(|ui| {
            // Logo/Title
            let _ = ui.heading(
                egui::RichText::new("🐨")
                    .size(80.0),
            );
            ui.add_space(16.0);

            let _ = ui.heading(
                egui::RichText::new("Koala Browser")
                    .size(32.0)
                    .strong(),
            );
            ui.add_space(8.0);

            let _ = ui.label(
                egui::RichText::new("A from-scratch browser built for learning")
                    .size(16.0)
                    .color(ui.visuals().text_color().gamma_multiply(0.6)),
            );

            ui.add_space(32.0);

            // Search/URL box
            let _ = egui::Frame::none()
                .fill(ui.visuals().extreme_bg_color)
                .rounding(egui::Rounding::same(24.0))
                .inner_margin(egui::Margin::symmetric(20.0, 12.0))
                .show(ui, |ui| {
                    ui.set_width(400.0);
                    let response = ui.add_sized(
                        [380.0, 24.0],
                        egui::TextEdit::singleline(&mut self.url_input)
                            .hint_text("Enter file path or URL...")
                            .frame(false)
                            .font(egui::FontId::proportional(16.0)),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let path = self.url_input.clone();
                        if !path.is_empty() {
                            self.navigate(&path);
                        }
                    }
                });

            ui.add_space(40.0);

            // Quick links
            let _ = ui.label(
                egui::RichText::new("Quick Links")
                    .size(14.0)
                    .color(ui.visuals().text_color().gamma_multiply(0.5)),
            );
            ui.add_space(16.0);

            let _ = ui.horizontal(|ui| {
                ui.add_space((available.x / 2.0 - 200.0).max(0.0));
                ui.spacing_mut().item_spacing.x = 16.0;

                for link in QUICK_LINKS {
                    let button = egui::Button::new(
                        egui::RichText::new(format!("{} {}", link.icon, link.name)).size(14.0),
                    )
                    .min_size(egui::vec2(120.0, 40.0))
                    .rounding(egui::Rounding::same(8.0));

                    if ui.add(button).on_hover_text(link.url).clicked() {
                        let url = link.url.to_string();
                        self.navigate(&url);
                    }
                }
            });

            ui.add_space(60.0);

            // Keyboard shortcuts hint
            let _ = ui.label(
                egui::RichText::new("Press F12 for debug panel • Click 🌙/☀️ to toggle theme")
                    .size(12.0)
                    .color(ui.visuals().text_color().gamma_multiply(0.4)),
            );
        });
    }

    /// Render the parsed page content
    fn render_page_content(&self, ui: &mut egui::Ui, page: &PageState) {
        self.render_node_content(ui, page, page.dom.root(), 0);
    }

    /// Recursively render a DOM node's content
    fn render_node_content(&self, ui: &mut egui::Ui, page: &PageState, id: NodeId, _depth: usize) {
        let Some(node) = page.dom.get(id) else {
            return;
        };

        match &node.node_type {
            NodeType::Document => {
                for &child_id in page.dom.children(id) {
                    self.render_node_content(ui, page, child_id, 0);
                }
            }
            NodeType::Element(data) => {
                let tag = data.tag_name.to_lowercase();
                let style = page.styles.get(&id);

                // Skip non-visual elements
                match tag.as_str() {
                    "head" | "meta" | "title" | "link" | "script" | "style" => return,
                    _ => {}
                }

                // Determine text formatting
                let (font_size, _is_bold) = match tag.as_str() {
                    "h1" => (32.0, true),
                    "h2" => (26.0, true),
                    "h3" => (22.0, true),
                    "h4" => (18.0, true),
                    "h5" => (16.0, true),
                    "h6" => (14.0, true),
                    "strong" | "b" => (14.0, true),
                    _ => {
                        let size = style
                            .and_then(|s| s.font_size.as_ref())
                            .map(|fs| fs.to_px())
                            .unwrap_or(14.0);
                        (size, false)
                    }
                };

                let text_color = style
                    .and_then(|s| s.color.as_ref())
                    .map(|c| egui::Color32::from_rgb(c.r, c.g, c.b))
                    .unwrap_or(ui.visuals().text_color());

                let is_block = matches!(
                    tag.as_str(),
                    "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "li"
                        | "article" | "section" | "header" | "footer" | "main" | "nav"
                        | "aside" | "blockquote" | "pre"
                );

                if is_block {
                    ui.add_space(6.0);
                }

                for &child_id in page.dom.children(id) {
                    let child = page.dom.get(child_id);
                    if let Some(child_node) = child {
                        match &child_node.node_type {
                            NodeType::Text(text) => {
                                let trimmed = text.trim();
                                if !trimmed.is_empty() {
                                    let job = egui::text::LayoutJob::simple_singleline(
                                        trimmed.to_string(),
                                        egui::FontId::new(font_size as f32, egui::FontFamily::Proportional),
                                        text_color,
                                    );
                                    let _ = ui.label(job);
                                }
                            }
                            _ => {
                                self.render_node_content(ui, page, child_id, _depth + 1);
                            }
                        }
                    }
                }

                if is_block {
                    ui.add_space(6.0);
                }
            }
            NodeType::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let _ = ui.label(trimmed);
                }
            }
            NodeType::Comment(_) => {}
        }
    }

    fn render_debug_dom(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("DOM Tree ({} nodes)", page.dom.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);
        self.render_debug_node(ui, page, page.dom.root(), 0);
    }

    fn render_debug_node(&self, ui: &mut egui::Ui, page: &PageState, id: NodeId, depth: usize) {
        let Some(node) = page.dom.get(id) else {
            return;
        };

        let indent = "  ".repeat(depth);

        match &node.node_type {
            NodeType::Document => {
                let _ = ui.monospace(format!("{}#document", indent));
            }
            NodeType::Element(data) => {
                let mut label = format!("{}<{}", indent, data.tag_name);
                if let Some(id_attr) = data.attrs.get("id") {
                    label.push_str(&format!(" id=\"{}\"", id_attr));
                }
                if let Some(class) = data.attrs.get("class") {
                    label.push_str(&format!(" class=\"{}\"", class));
                }
                label.push('>');

                let has_style = page.styles.contains_key(&id);
                if has_style {
                    let _ = ui.horizontal(|ui| {
                        let _ = ui.monospace(&label);
                        let _ = ui.colored_label(egui::Color32::from_rgb(100, 200, 100), "●");
                    });
                } else {
                    let _ = ui.monospace(label);
                }
            }
            NodeType::Text(text) => {
                let preview = if text.len() > 40 {
                    format!("{}...", &text.trim()[..40.min(text.trim().len())])
                } else {
                    text.trim().to_string()
                };
                if !preview.is_empty() {
                    let _ = ui.colored_label(
                        ui.visuals().text_color().gamma_multiply(0.6),
                        format!("{}\"{}\"", indent, preview),
                    );
                }
            }
            NodeType::Comment(text) => {
                let preview = if text.len() > 30 {
                    format!("{}...", &text[..30])
                } else {
                    text.clone()
                };
                let _ = ui.colored_label(
                    ui.visuals().text_color().gamma_multiply(0.4),
                    format!("{}<!-- {} -->", indent, preview),
                );
            }
        }

        for &child_id in page.dom.children(id) {
            self.render_debug_node(ui, page, child_id, depth + 1);
        }
    }

    fn render_debug_tokens(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("HTML Tokens ({})", page.tokens.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        for (i, token) in page.tokens.iter().enumerate() {
            let _ = ui.monospace(format!("{:4}: {:?}", i, token));
        }
    }

    fn render_debug_css(&self, ui: &mut egui::Ui, page: &PageState) {
        if page.css_text.is_empty() {
            let _ = ui.colored_label(
                ui.visuals().text_color().gamma_multiply(0.5),
                "No CSS found in document",
            );
            return;
        }

        let _ = ui.label(
            egui::RichText::new(format!("CSS ({} bytes)", page.css_text.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        let _ = egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .rounding(egui::Rounding::same(4.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                let _ = ui.add(
                    egui::TextEdit::multiline(&mut page.css_text.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
    }

    fn render_debug_styles(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("Computed Styles ({} elements)", page.styles.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        for (node_id, style) in &page.styles {
            if let Some(node) = page.dom.get(*node_id) {
                if let NodeType::Element(data) = &node.node_type {
                    let _ =
                        ui.collapsing(format!("<{}> (node {})", data.tag_name, node_id.0), |ui| {
                            if let Some(ref color) = style.color {
                                let _ = ui.monospace(format!(
                                    "color: #{:02x}{:02x}{:02x}",
                                    color.r, color.g, color.b
                                ));
                            }
                            if let Some(ref bg) = style.background_color {
                                let _ = ui.monospace(format!(
                                    "background-color: #{:02x}{:02x}{:02x}",
                                    bg.r, bg.g, bg.b
                                ));
                            }
                            if let Some(ref fs) = style.font_size {
                                let _ = ui.monospace(format!("font-size: {}px", fs.to_px()));
                            }
                            if let Some(ref m) = style.margin_top {
                                let _ = ui.monospace(format!("margin-top: {}px", m.to_px()));
                            }
                            if let Some(ref p) = style.padding_top {
                                let _ = ui.monospace(format!("padding-top: {}px", p.to_px()));
                            }
                        });
                }
            }
        }
    }

    fn render_debug_source(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("HTML Source ({} bytes)", page.html_source.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        let _ = egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .rounding(egui::Rounding::same(4.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                let _ = ui.add(
                    egui::TextEdit::multiline(&mut page.html_source.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
    }
}
