//! Koala Browser GUI - egui-based browser interface
//!
//! Run with: cargo run --bin koala
//!
//! Debug features:
//! - F12: Toggle debug panel
//! - All state changes logged to terminal
//!
//! Headless mode:
//! - koala -H file.html     # Print DOM/layout info
//! - koala -S out.png URL   # Take screenshot

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;

use clap::Parser;
use eframe::egui;
use koala_browser::{load_document, parse_html_string, renderer::Renderer, LoadedDocument};
use koala_common::warning::clear_warnings;
use koala_css::{LayoutBox, Rect};
use koala_dom::{NodeId, NodeType};
use koala_html::print_tree;

/// Koala Browser - A from-scratch web browser built for learning
#[derive(Parser, Debug)]
#[command(name = "koala")]
#[command(author, version, about, long_about = None)]
#[command(after_help = r#"EXAMPLES:
    # Open browser GUI
    koala

    # Open browser with a file
    koala ./index.html

    # Headless mode: print DOM tree
    koala -H https://example.com

    # Headless mode: print layout tree
    koala -H --layout https://example.com

    # Take a screenshot
    koala -S screenshot.png https://example.com

    # Screenshot with custom viewport
    koala -S output.png --width 1920 --height 1080 https://example.com

    # Parse inline HTML
    koala --html '<h1>Test</h1>'
"#)]
struct Cli {
    /// Path to HTML file or URL to open
    #[arg(value_name = "FILE|URL")]
    path: Option<String>,

    /// Parse HTML string directly instead of file/URL
    #[arg(long, value_name = "HTML")]
    html: Option<String>,

    /// Run in headless mode (no GUI, print to terminal)
    #[arg(short = 'H', long)]
    headless: bool,

    /// Show computed layout tree with dimensions (headless mode)
    #[arg(long)]
    layout: bool,

    /// Take a screenshot and save to the specified file (PNG format)
    #[arg(short = 'S', long, value_name = "FILE")]
    screenshot: Option<PathBuf>,

    /// Viewport width for screenshot (default: 1280)
    #[arg(long, default_value = "1280")]
    width: u32,

    /// Viewport height for screenshot (default: 720)
    #[arg(long, default_value = "720")]
    height: u32,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Screenshot mode
    if let Some(ref output_path) = cli.screenshot {
        let doc = load_doc(&cli)?;
        take_screenshot(&doc, output_path, cli.width, cli.height)?;
        println!("Screenshot saved to: {}", output_path.display());
        return Ok(());
    }

    // Headless mode
    if cli.headless {
        let doc = load_doc(&cli)?;
        if cli.layout {
            print_layout(&doc);
        } else {
            print_document(&doc);
        }
        return Ok(());
    }

    // GUI mode
    run_gui(cli.path.or(cli.html))
}

/// Load document from CLI arguments
fn load_doc(cli: &Cli) -> anyhow::Result<LoadedDocument> {
    if let Some(ref html_string) = cli.html {
        Ok(parse_html_string(html_string))
    } else if let Some(ref path) = cli.path {
        load_document(path).map_err(|e| anyhow::anyhow!("{}", e))
    } else {
        anyhow::bail!("Headless/screenshot mode requires a file path, URL, or --html")
    }
}

/// Take a screenshot of the rendered page
fn take_screenshot(doc: &LoadedDocument, output_path: &PathBuf, width: u32, height: u32) -> anyhow::Result<()> {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
    };

    let layout_tree = doc
        .layout_tree
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No layout tree available"))?;

    let mut layout = layout_tree.clone();
    layout.layout(viewport, viewport);

    let mut renderer = Renderer::new(width, height);
    renderer.render(&layout, doc);
    renderer.save(output_path)?;

    Ok(())
}

/// Print document information to stdout (headless mode)
fn print_document(doc: &LoadedDocument) {
    println!("=== DOM Tree ===");
    print_tree(&doc.dom, doc.dom.root(), 0);

    println!("\n=== Stylesheet ===");
    println!("{} rules", doc.stylesheet.rules.len());

    println!("\n=== Computed Styles ===");
    println!("{} styled elements", doc.styles.len());

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

/// Print layout tree with computed dimensions (headless mode)
fn print_layout(doc: &LoadedDocument) {
    let viewport_width = 1280.0;
    let viewport_height = 720.0;

    println!(
        "=== Layout Tree (viewport: {}x{}) ===\n",
        viewport_width, viewport_height
    );

    if let Some(ref layout_tree) = doc.layout_tree {
        let mut layout = layout_tree.clone();
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_width,
            height: viewport_height,
        };
        layout.layout(viewport, viewport);
        print_layout_box(&layout, 0, doc);
    } else {
        println!("No layout tree available");
    }
}

/// Recursively print a layout box with its dimensions
fn print_layout_box(layout_box: &LayoutBox, depth: usize, doc: &LoadedDocument) {
    let indent = "  ".repeat(depth);
    let dims = &layout_box.dimensions;

    let name = match &layout_box.box_type {
        koala_css::BoxType::Principal(node_id) => {
            if let Some(element) = doc.dom.as_element(*node_id) {
                format!("<{}> ({:?})", element.tag_name, node_id)
            } else if doc
                .dom
                .get(*node_id)
                .map(|n| matches!(n.node_type, NodeType::Document))
                .unwrap_or(false)
            {
                format!("Document ({:?})", node_id)
            } else {
                format!("{:?}", node_id)
            }
        }
        koala_css::BoxType::AnonymousBlock => "AnonymousBlock".to_string(),
        koala_css::BoxType::AnonymousInline(text) => {
            let preview: String = text.chars().take(30).collect();
            let suffix = if text.len() > 30 { "..." } else { "" };
            format!("Text(\"{}{}\")", preview.replace('\n', "\\n"), suffix)
        }
    };

    println!("{}[{}] {:?}", indent, name, layout_box.display);
    println!(
        "{}  content: x={:.1} y={:.1} w={:.1} h={:.1}",
        indent, dims.content.x, dims.content.y, dims.content.width, dims.content.height
    );

    if dims.margin.top != 0.0
        || dims.margin.right != 0.0
        || dims.margin.bottom != 0.0
        || dims.margin.left != 0.0
    {
        println!(
            "{}  margin: t={:.1} r={:.1} b={:.1} l={:.1}",
            indent, dims.margin.top, dims.margin.right, dims.margin.bottom, dims.margin.left
        );
    }

    if dims.padding.top != 0.0
        || dims.padding.right != 0.0
        || dims.padding.bottom != 0.0
        || dims.padding.left != 0.0
    {
        println!(
            "{}  padding: t={:.1} r={:.1} b={:.1} l={:.1}",
            indent, dims.padding.top, dims.padding.right, dims.padding.bottom, dims.padding.left
        );
    }

    println!();

    for child in &layout_box.children {
        print_layout_box(child, depth + 1, doc);
    }
}

/// Run the GUI browser
fn run_gui(initial_url: Option<String>) -> anyhow::Result<()> {
    println!("[Koala GUI] Starting browser...");

    if let Some(ref url) = initial_url {
        println!("[Koala GUI] Will open: {}", url);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Koala Browser",
        options,
        Box::new(move |cc| Ok(Box::new(BrowserApp::new(&cc.egui_ctx, initial_url)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
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

    /// CSS properties we've warned about - (property, tag) pairs
    /// Cleared on each page load to avoid spam
    css_warnings_logged: RefCell<HashSet<(String, String)>>,

    /// URL to navigate to on first update (from command-line arg)
    pending_navigation: Option<String>,
}

/// Parsed page state - wraps LoadedDocument with GUI-specific fields
struct PageState {
    /// The loaded document from koala-browser
    doc: LoadedDocument,

    /// Last viewport size used for layout (to detect when relayout is needed)
    last_layout_viewport: Option<(f32, f32)>,
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
    fn new(ctx: &egui::Context, initial_url: Option<String>) -> Self {
        let theme = Theme::Dark;
        ctx.set_visuals(theme.visuals());
        println!("[Koala GUI] Browser initialized with {:?} theme", theme);

        Self {
            url_input: initial_url.clone().unwrap_or_default(),
            history: Vec::new(),
            history_index: 0,
            page: None,
            debug_panel_open: false,
            debug_tab: DebugTab::Dom,
            status_message: "Welcome to Koala Browser".to_string(),
            theme,
            css_warnings_logged: RefCell::new(HashSet::new()),
            pending_navigation: initial_url,
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.theme = theme;
        ctx.set_visuals(theme.visuals());
        println!("[Koala GUI] Theme changed to {:?}", theme);
    }

    /// Navigate to a URL/path
    fn navigate(&mut self, path: &str) {
        // Clear CSS warnings for the new page
        clear_warnings();
        self.css_warnings_logged.borrow_mut().clear();

        println!("[Koala GUI] Navigating to: {}", path);
        self.status_message = format!("Loading {}...", path);

        match self.load_page(path) {
            Ok(page) => {
                println!("[Koala GUI] Page loaded successfully");
                println!("[Koala GUI]   - {} tokens", page.doc.tokens.len());
                println!("[Koala GUI]   - {} DOM nodes", page.doc.dom.len());
                println!("[Koala GUI]   - {} styled nodes", page.doc.styles.len());
                println!("[Koala GUI]   - {} bytes CSS", page.doc.css_text.len());

                if !page.doc.parse_issues.is_empty() {
                    println!("[Koala GUI]   - {} parse issues:", page.doc.parse_issues.len());
                    for issue in &page.doc.parse_issues {
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
    ///
    /// Uses koala_browser::load_document for the actual loading/parsing.
    fn load_page(&self, path: &str) -> Result<PageState, String> {
        let doc = load_document(path).map_err(|e| e.to_string())?;

        println!("[Koala GUI] Parsing {} bytes of HTML", doc.html_source.len());
        println!("[Koala GUI] Tokenized: {} tokens", doc.tokens.len());
        println!("[Koala GUI] Parsed: {} nodes", doc.dom.len());
        if !doc.css_text.is_empty() {
            println!("[Koala GUI] Parsing {} bytes of CSS", doc.css_text.len());
            println!("[Koala GUI] CSS: {} rules", doc.stylesheet.rules.len());
        }
        if doc.layout_tree.is_some() {
            println!("[Koala GUI] Layout tree built (layout pending)");
        }

        Ok(PageState {
            doc,
            last_layout_viewport: None, // Will be set on first render
        })
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
            let path = page.doc.source_path.clone();
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
        // Handle pending navigation from command-line argument
        if let Some(url) = self.pending_navigation.take() {
            self.navigate(&url);
        }

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
                                    page.doc.dom.len(),
                                    page.doc.styles.len()
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
                if self.page.is_some() {
                    // Get actual viewport size from egui
                    // [§ 9.1.1 The viewport](https://www.w3.org/TR/CSS2/visuren.html#viewport)
                    let available = ui.available_size();
                    let viewport_size = (available.x, available.y);

                    // Recompute layout if viewport changed (mutable borrow scope)
                    {
                        let page = self.page.as_mut().unwrap();
                        if page.last_layout_viewport != Some(viewport_size) {
                            if let Some(ref mut root) = page.doc.layout_tree {
                                // [§ 9.1.2 Containing blocks](https://www.w3.org/TR/CSS2/visuren.html#containing-block)
                                //
                                // "The containing block in which the root element lives is a
                                // rectangle called the initial containing block. For continuous
                                // media, it has the dimensions of the viewport..."
                                let initial_containing_block = Rect {
                                    x: 0.0,
                                    y: 0.0,
                                    width: viewport_size.0,
                                    height: viewport_size.1,
                                };
                                // [§ 5.1.2 Viewport-percentage lengths](https://www.w3.org/TR/css-values-4/#viewport-relative-lengths)
                                //
                                // "The viewport-percentage lengths are relative to the size
                                // of the initial containing block."
                                let viewport = initial_containing_block;
                                root.layout(initial_containing_block, viewport);
                                page.last_layout_viewport = Some(viewport_size);
                                println!(
                                    "[Koala GUI] Layout computed for viewport {}x{}",
                                    viewport_size.0 as u32, viewport_size.1 as u32
                                );
                            }
                        }
                    }

                    // Now borrow immutably for rendering
                    let page = self.page.as_ref().unwrap();

                    // Page content
                    // [§ 2.11.2 The Canvas Background](https://www.w3.org/TR/css-backgrounds-3/#special-backgrounds)
                    //
                    // "The background of the root element becomes the background of the canvas
                    // and covers the entire canvas"
                    //
                    // We use a neutral background for the canvas, then paint the body's
                    // background at its computed layout position.
                    let canvas_color = if self.theme == Theme::Dark {
                        egui::Color32::from_rgb(28, 28, 30)
                    } else {
                        egui::Color32::WHITE
                    };
                    let _ = egui::Frame::none()
                        .fill(canvas_color)
                        .inner_margin(egui::Margin::ZERO)
                        .show(ui, |ui| {
                            // Use auto_shrink(false) to prevent ScrollArea from adding scrollbar
                            // space that would make the content area narrower than the layout viewport.
                            let _ = egui::ScrollArea::vertical()
                                .auto_shrink(false)
                                .show(ui, |ui| {
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
        // Use layout tree if available, otherwise fall back to DOM walking
        if let Some(ref layout_tree) = page.doc.layout_tree {
            self.render_layout_tree(ui, page, layout_tree);
        } else {
            self.render_node_content(ui, page, page.doc.dom.root(), 0);
        }
    }

    /// Render content using the computed layout tree with absolute positioning
    ///
    /// [§ 9.1 The viewport](https://www.w3.org/TR/CSS2/visuren.html#viewport)
    /// "User agents for continuous media generally offer users a viewport
    /// through which users consult a document."
    fn render_layout_tree(&self, ui: &mut egui::Ui, page: &PageState, layout_box: &LayoutBox) {
        // [§ 9.1.2 Containing blocks](https://www.w3.org/TR/CSS2/visuren.html#containing-block)
        //
        // Since we're using painter() for absolute positioning, we need to tell the
        // ScrollArea about the total content height. We allocate this space upfront
        // as an invisible rect so scrolling works correctly.
        let total_height = layout_box.dimensions.margin_box().height;
        let total_width = layout_box.dimensions.margin_box().width;
        let (_response, _painter) = ui.allocate_painter(
            egui::vec2(total_width, total_height),
            egui::Sense::hover(),
        );

        // The origin is where document (0,0) maps to on screen
        let origin = ui.clip_rect().min;
        self.render_layout_box_absolute(ui, page, layout_box, origin);
    }

    /// Render a layout box using absolute positioning based on computed dimensions
    fn render_layout_box_absolute(
        &self,
        ui: &mut egui::Ui,
        page: &PageState,
        layout_box: &LayoutBox,
        origin: egui::Pos2,
    ) {
        use koala_css::BoxType;

        // Get the node ID for this box (if it's a principal box)
        let node_id = match layout_box.box_type {
            BoxType::Principal(id) => Some(id),
            _ => None,
        };

        // Get computed style and node info
        let (tag, style) = if let Some(id) = node_id {
            let style = page.doc.styles.get(&id);
            let tag = page.doc.dom.get(id).and_then(|n| {
                if let NodeType::Element(data) = &n.node_type {
                    Some(data.tag_name.to_lowercase())
                } else {
                    None
                }
            });
            (tag, style)
        } else {
            (None, None)
        };

        // Skip non-visual elements
        if let Some(ref t) = tag {
            match t.as_str() {
                "head" | "meta" | "title" | "link" | "script" | "style" => return,
                _ => {}
            }
        }

        // Get the layout dimensions
        let dims = &layout_box.dimensions;

        // Convert layout coordinates to screen coordinates
        // The layout box's content rect position is relative to the viewport origin
        let content_rect = egui::Rect::from_min_size(
            egui::pos2(origin.x + dims.content.x, origin.y + dims.content.y),
            egui::vec2(dims.content.width, dims.content.height),
        );

        // Paint background if this element has one
        // [§ 2.1 background-color](https://www.w3.org/TR/css-backgrounds-3/#background-color)
        if let Some(s) = style {
            if let Some(ref bg) = s.background_color {
                let bg_color = egui::Color32::from_rgb(bg.r, bg.g, bg.b);
                // Paint at the padding box (content + padding)
                let padding_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        origin.x + dims.content.x - dims.padding.left,
                        origin.y + dims.content.y - dims.padding.top,
                    ),
                    egui::vec2(
                        dims.content.width + dims.padding.left + dims.padding.right,
                        dims.content.height + dims.padding.top + dims.padding.bottom,
                    ),
                );
                let _ = ui.painter().rect_filled(padding_rect, 0.0, bg_color);
            }
        }

        // Determine text formatting from style
        let font_size = style
            .and_then(|s| s.font_size.as_ref())
            .map(|fs| fs.to_px())
            .unwrap_or_else(|| {
                match tag.as_deref() {
                    Some("h1") => 32.0,
                    Some("h2") => 24.0,
                    Some("h3") => 18.72,
                    Some("h4") => 16.0,
                    Some("h5") => 13.28,
                    Some("h6") => 10.72,
                    _ => 16.0,
                }
            });

        let text_color = style
            .and_then(|s| s.color.as_ref())
            .map(|c| egui::Color32::from_rgb(c.r, c.g, c.b))
            .unwrap_or(ui.visuals().text_color());

        // Render text content for this node at the computed position
        if let Some(id) = node_id {
            let mut text_y = content_rect.min.y;
            for &child_id in page.doc.dom.children(id) {
                if let Some(child_node) = page.doc.dom.get(child_id) {
                    if let NodeType::Text(text) = &child_node.node_type {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            // Paint text at the computed position
                            let text_pos = egui::pos2(content_rect.min.x, text_y);
                            let galley = ui.painter().layout(
                                trimmed.to_string(),
                                egui::FontId::new(font_size as f32, egui::FontFamily::Proportional),
                                text_color,
                                content_rect.width(),
                            );
                            ui.painter().galley(text_pos, galley.clone(), text_color);
                            text_y += galley.rect.height() + 4.0; // Line spacing
                        }
                    }
                }
            }
        }

        // Recursively render children from the layout tree
        for child in &layout_box.children {
            self.render_layout_box_absolute(ui, page, child, origin);
        }

        // Note: We don't need to call allocate_space() because we're using
        // painter().rect_filled() which draws directly without affecting layout.
        // The ScrollArea will size based on the content naturally.
    }

    /// Recursively render a DOM node's content
    fn render_node_content(&self, ui: &mut egui::Ui, page: &PageState, id: NodeId, _depth: usize) {
        let Some(node) = page.doc.dom.get(id) else {
            return;
        };

        match &node.node_type {
            NodeType::Document => {
                for &child_id in page.doc.dom.children(id) {
                    self.render_node_content(ui, page, child_id, 0);
                }
            }
            NodeType::Element(data) => {
                let tag = data.tag_name.to_lowercase();
                let style = page.doc.styles.get(&id);

                // Skip non-visual elements
                match tag.as_str() {
                    "head" | "meta" | "title" | "link" | "script" | "style" => return,
                    _ => {}
                }

                // Warn about CSS properties we parse but don't render
                if let Some(s) = style {
                    // background-color only supported on body (via canvas_background)
                    if tag != "body" {
                        if let Some(bg) = &s.background_color {
                            self.warn_unsupported_css(
                                "background-color",
                                &tag,
                                &bg.to_hex_string(),
                            );
                        }
                    }

                    // font-family is parsed but not applied
                    if let Some(ff) = &s.font_family {
                        self.warn_unsupported_css("font-family", &tag, ff);
                    }

                    // margin/padding are parsed but we use hardcoded spacing
                    if s.margin_top.is_some()
                        || s.margin_right.is_some()
                        || s.margin_bottom.is_some()
                        || s.margin_left.is_some()
                    {
                        self.warn_unsupported_css("margin", &tag, "(set)");
                    }
                    if s.padding_top.is_some()
                        || s.padding_right.is_some()
                        || s.padding_bottom.is_some()
                        || s.padding_left.is_some()
                    {
                        self.warn_unsupported_css("padding", &tag, "(set)");
                    }
                }

                // Determine text formatting
                // [§ 15.7 Font size](https://www.w3.org/TR/CSS21/fonts.html#font-size-props)
                // CSS font-size takes precedence; fall back to user-agent defaults
                let _is_heading_or_bold = matches!(
                    tag.as_str(),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "strong" | "b"
                );
                let font_size = style
                    .and_then(|s| s.font_size.as_ref())
                    .map(|fs| fs.to_px())
                    .unwrap_or_else(|| {
                        // User-agent default sizes when no CSS specified
                        // [§ 15.3.1 HTML headings](https://html.spec.whatwg.org/#sections-and-headings)
                        match tag.as_str() {
                            "h1" => 32.0,
                            "h2" => 24.0,
                            "h3" => 18.72,
                            "h4" => 16.0,
                            "h5" => 13.28,
                            "h6" => 10.72,
                            _ => 16.0, // Default body text size
                        }
                    });

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

                for &child_id in page.doc.dom.children(id) {
                    let child = page.doc.dom.get(child_id);
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

    /// Log a warning about an unsupported CSS property, but only once per (property, tag) pair
    fn warn_unsupported_css(&self, property: &str, tag: &str, value: &str) {
        let key = (property.to_string(), tag.to_string());
        let mut logged = self.css_warnings_logged.borrow_mut();
        if !logged.contains(&key) {
            println!(
                "[Koala CSS] Ignoring {}: {} on <{}> (not yet implemented)",
                property, value, tag
            );
            let _ = logged.insert(key);
        }
    }

    fn render_debug_dom(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("DOM Tree ({} nodes)", page.doc.dom.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);
        self.render_debug_node(ui, page, page.doc.dom.root(), 0);
    }

    fn render_debug_node(&self, ui: &mut egui::Ui, page: &PageState, id: NodeId, depth: usize) {
        let Some(node) = page.doc.dom.get(id) else {
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

                let has_style = page.doc.styles.contains_key(&id);
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

        for &child_id in page.doc.dom.children(id) {
            self.render_debug_node(ui, page, child_id, depth + 1);
        }
    }

    fn render_debug_tokens(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("HTML Tokens ({})", page.doc.tokens.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        for (i, token) in page.doc.tokens.iter().enumerate() {
            let _ = ui.monospace(format!("{:4}: {:?}", i, token));
        }
    }

    fn render_debug_css(&self, ui: &mut egui::Ui, page: &PageState) {
        if page.doc.css_text.is_empty() {
            let _ = ui.colored_label(
                ui.visuals().text_color().gamma_multiply(0.5),
                "No CSS found in document",
            );
            return;
        }

        let _ = ui.label(
            egui::RichText::new(format!("CSS ({} bytes)", page.doc.css_text.len()))
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
                    egui::TextEdit::multiline(&mut page.doc.css_text.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
    }

    fn render_debug_styles(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("Computed Styles ({} elements)", page.doc.styles.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        for (node_id, style) in &page.doc.styles {
            if let Some(node) = page.doc.dom.get(*node_id) {
                if let NodeType::Element(data) = &node.node_type {
                    let _ =
                        ui.collapsing(format!("<{}> (node {})", data.tag_name, node_id.0), |ui| {
                            if let Some(ref color) = style.color {
                                let _ = ui.monospace(format!("color: {}", color.to_hex_string()));
                            }
                            if let Some(ref bg) = style.background_color {
                                let _ = ui.monospace(format!("background-color: {}", bg.to_hex_string()));
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
            egui::RichText::new(format!("HTML Source ({} bytes)", page.doc.html_source.len()))
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
                    egui::TextEdit::multiline(&mut page.doc.html_source.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
    }
}
