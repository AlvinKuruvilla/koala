//! Koala development GUI — egui-based renderer inspector
//!
//! Run with: cargo run --bin koala-gui
//!
//! Debug features:
//! - F12: Toggle debug panel
//! - All state changes logged to terminal
//!
//! Headless mode:
//! - koala -H file.html     # Print DOM/layout info
//! - koala -S out.png URL   # Take screenshot

mod theme;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use clap::Parser;
use eframe::egui;
use koala_browser::{
    FontProvider, LoadedDocument, load_document, parse_html_string, renderer::Renderer,
};
use koala_common::warning::clear_warnings;
use koala_css::Painter;
use koala_css::{AutoLength, LayoutBox, Rect};
use koala_dom::{NodeId, NodeType};
use koala_html::print_tree;

use theme::{ColorPalette, Theme};

/// Koala GUI — development debugging interface for the Koala renderer
#[derive(Parser, Debug)]
#[command(name = "koala-gui")]
#[command(author, version, about, long_about = None)]
#[command(after_help = r#"EXAMPLES:
    # Open development GUI
    koala-gui

    # Open GUI with a file
    koala-gui ./index.html

    # Headless mode: print DOM tree
    koala-gui -H https://example.com

    # Headless mode: print layout tree
    koala-gui -H --layout https://example.com

    # Take a screenshot
    koala-gui -S screenshot.png https://example.com

    # Screenshot with custom viewport
    koala-gui -S output.png --width 1920 --height 1080 https://example.com

    # Parse inline HTML
    koala-gui --html '<h1>Test</h1>'
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
fn take_screenshot(
    doc: &LoadedDocument,
    output_path: &Path,
    width: u32,
    height: u32,
) -> anyhow::Result<()> {
    #[allow(clippy::cast_precision_loss)]
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
    let font_provider = FontProvider::load();
    let font_metrics = font_provider.metrics();
    layout.layout(viewport, viewport, &*font_metrics, viewport);

    // Paint: generate display list from layout tree
    let painter = Painter::new(&doc.styles);
    let display_list = painter.paint(&layout);

    // Render: execute display list to pixels
    let mut renderer = Renderer::new(width, height, doc.images.clone());
    renderer.render(&display_list);
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
            println!("  - {issue}");
        }
    }
}

/// Print layout tree with computed dimensions (headless mode)
fn print_layout(doc: &LoadedDocument) {
    let viewport_width = 1280.0;
    let viewport_height = 720.0;

    println!("=== Layout Tree (viewport: {viewport_width}x{viewport_height}) ===\n");

    if let Some(ref layout_tree) = doc.layout_tree {
        let mut layout = layout_tree.clone();
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_width,
            height: viewport_height,
        };
        let font_provider = FontProvider::load();
        let font_metrics = font_provider.metrics();
        layout.layout(viewport, viewport, &*font_metrics, viewport);
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
        koala_css::BoxType::Principal(node_id) => doc.dom.as_element(*node_id).map_or_else(
            || {
                if doc
                    .dom
                    .get(*node_id)
                    .is_some_and(|n| matches!(n.node_type, NodeType::Document))
                {
                    format!("Document ({node_id:?})")
                } else {
                    format!("{node_id:?}")
                }
            },
            |element| format!("<{}> ({:?})", element.tag_name, node_id),
        ),
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
        println!("[Koala GUI] Will open: {url}");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Koala (Dev)",
        options,
        Box::new(move |cc| Ok(Box::new(BrowserApp::new(&cc.egui_ctx, initial_url)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
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

    /// Semantic color palette derived from the current theme.
    palette: ColorPalette,

    /// CSS properties we've warned about - (property, tag) pairs
    /// Cleared on each page load to avoid spam
    css_warnings_logged: RefCell<HashSet<(String, String)>>,

    /// URL to navigate to on first update (from command-line arg)
    pending_navigation: Option<String>,

    /// Font provider for real text measurement during layout.
    ///
    /// [§ 10.8 Line height calculations](https://www.w3.org/TR/CSS2/visudet.html#line-height)
    font_provider: FontProvider,

    /// Cached egui texture handles for loaded images, keyed by `src`.
    ///
    /// [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
    ///
    /// Textures are lazily created from `LoadedImage` RGBA data and cached
    /// for the lifetime of the current page. Cleared on navigation.
    image_textures: RefCell<HashMap<String, egui::TextureHandle>>,
}

/// Parsed page state - wraps `LoadedDocument` with GUI-specific fields
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
        theme::setup_fonts(ctx);

        let theme = Theme::Dark;
        theme.apply(ctx);
        println!("[Koala GUI] Browser initialized with {theme:?} theme");

        Self {
            url_input: initial_url.clone().unwrap_or_default(),
            history: Vec::new(),
            history_index: 0,
            page: None,
            debug_panel_open: false,
            debug_tab: DebugTab::Dom,
            status_message: "Welcome to Koala".to_string(),
            theme,
            palette: theme.palette(),
            css_warnings_logged: RefCell::new(HashSet::new()),
            pending_navigation: initial_url,
            font_provider: FontProvider::load(),
            image_textures: RefCell::new(HashMap::new()),
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.theme = theme;
        self.palette = theme.palette();
        theme.apply(ctx);
        println!("[Koala GUI] Theme changed to {theme:?}");
    }

    /// Navigate to a URL/path
    fn navigate(&mut self, path: &str) {
        // Clear CSS warnings and image textures for the new page
        clear_warnings();
        self.css_warnings_logged.borrow_mut().clear();
        self.image_textures.borrow_mut().clear();

        println!("[Koala GUI] Navigating to: {path}");
        self.status_message = format!("Loading {path}...");

        match self.load_page(path) {
            Ok(page) => {
                println!("[Koala GUI] Page loaded successfully");
                println!("[Koala GUI]   - {} tokens", page.doc.tokens.len());
                println!("[Koala GUI]   - {} DOM nodes", page.doc.dom.len());
                println!("[Koala GUI]   - {} styled nodes", page.doc.styles.len());
                println!("[Koala GUI]   - {} bytes CSS", page.doc.css_text.len());

                if !page.doc.parse_issues.is_empty() {
                    println!(
                        "[Koala GUI]   - {} parse issues:",
                        page.doc.parse_issues.len()
                    );
                    for issue in &page.doc.parse_issues {
                        println!("[Koala GUI]     ! {issue}");
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
                self.status_message = format!("Loaded: {path}");
            }
            Err(e) => {
                println!("[Koala GUI] ERROR loading page: {e}");
                self.status_message = format!("Error: {e}");
                self.page = None;
            }
        }
    }

    /// Load and parse a page from a file path or URL
    ///
    /// Uses `koala_browser::load_document` for the actual loading/parsing.
    #[allow(clippy::unused_self)]
    fn load_page(&self, path: &str) -> Result<PageState, String> {
        let doc = load_document(path).map_err(|e| e.to_string())?;

        println!(
            "[Koala GUI] Parsing {} bytes of HTML",
            doc.html_source.len()
        );
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
            println!("[Koala GUI] Going back to: {path}");
            self.url_input.clone_from(&path);
            if let Ok(page) = self.load_page(&path) {
                self.page = Some(page);
                self.status_message = format!("Loaded: {path}");
            }
        }
    }

    fn go_forward(&mut self) {
        if self.history_index < self.history.len() {
            self.history_index += 1;
            let path = self.history[self.history_index - 1].clone();
            println!("[Koala GUI] Going forward to: {path}");
            self.url_input.clone_from(&path);
            if let Ok(page) = self.load_page(&path) {
                self.page = Some(page);
                self.status_message = format!("Loaded: {path}");
            }
        }
    }

    fn refresh(&mut self) {
        if let Some(page) = &self.page {
            let path = page.doc.source_path.clone();
            println!("[Koala GUI] Refreshing: {path}");
            self.navigate(&path);
        }
    }

    fn go_home(&mut self) {
        self.page = None;
        self.url_input.clear();
        self.status_message = "Welcome to Koala".to_string();
        println!("[Koala GUI] Returned to home");
    }

    const fn can_go_back(&self) -> bool {
        self.history_index > 1
    }

    const fn can_go_forward(&self) -> bool {
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
                if self.debug_panel_open {
                    "OPEN"
                } else {
                    "CLOSED"
                }
            );
        }

        // Top panel: Navigation bar
        let nav_response = egui::TopBottomPanel::top("nav_bar")
            .frame(
                egui::Frame::none()
                    .fill(self.palette.bg_surface)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                let _ = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Navigation buttons — ghost style (transparent fill, visible on hover)
                    let button_size = egui::vec2(32.0, 28.0);

                    if ui
                        .add_enabled(
                            self.can_go_back(),
                            egui::Button::new("◀")
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(button_size),
                        )
                        .on_hover_text("Back")
                        .clicked()
                    {
                        self.go_back();
                    }

                    if ui
                        .add_enabled(
                            self.can_go_forward(),
                            egui::Button::new("▶")
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(button_size),
                        )
                        .on_hover_text("Forward")
                        .clicked()
                    {
                        self.go_forward();
                    }

                    if ui
                        .add_enabled(
                            self.page.is_some(),
                            egui::Button::new("↻")
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(button_size),
                        )
                        .on_hover_text("Refresh")
                        .clicked()
                    {
                        self.refresh();
                    }

                    if ui
                        .add(
                            egui::Button::new("🏠")
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(button_size),
                        )
                        .on_hover_text("Home")
                        .clicked()
                    {
                        self.go_home();
                    }

                    ui.add_space(8.0);

                    // URL bar with rounded frame and focus-aware border
                    let url_bar_width = ui.available_width() - 100.0;

                    // Check if the URL text edit had focus last frame
                    let url_id = ui.id().with("url_input");
                    let url_focused = ui.memory(|m| m.has_focus(url_id));
                    let url_border = if url_focused {
                        self.palette.accent
                    } else {
                        self.palette.border_subtle
                    };

                    let _ = egui::Frame::none()
                        .fill(self.palette.bg_input)
                        .rounding(egui::Rounding::same(14.0))
                        .stroke(egui::Stroke::new(1.0, url_border))
                        .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_width(url_bar_width);
                            let response = ui.add_sized(
                                [url_bar_width - 24.0, 20.0],
                                egui::TextEdit::singleline(&mut self.url_input)
                                    .id(url_id)
                                    .hint_text("Enter file path or URL...")
                                    .frame(false)
                                    .font(egui::TextStyle::Body),
                            );

                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                let path = self.url_input.clone();
                                self.navigate(&path);
                            }
                        });

                    ui.add_space(8.0);

                    // Theme toggle
                    if ui
                        .add(
                            egui::Button::new(self.theme.icon())
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(button_size),
                        )
                        .on_hover_text("Toggle theme")
                        .clicked()
                    {
                        let new_theme = self.theme.toggle();
                        self.set_theme(ctx, new_theme);
                    }

                    // Debug toggle — accent tint when open
                    let debug_button = if self.debug_panel_open {
                        egui::Button::new("🔧").fill(self.palette.accent_muted)
                    } else {
                        egui::Button::new("🔧").fill(egui::Color32::TRANSPARENT)
                    };
                    if ui
                        .add(debug_button.min_size(button_size))
                        .on_hover_text("Toggle debug panel (F12)")
                        .clicked()
                    {
                        self.debug_panel_open = !self.debug_panel_open;
                        println!(
                            "[Koala GUI] Debug panel: {}",
                            if self.debug_panel_open {
                                "OPEN"
                            } else {
                                "CLOSED"
                            }
                        );
                    }
                });
            });

        // Paint bottom separator line for the nav bar
        {
            let r = nav_response.response.rect;
            let _ = ctx.layer_painter(egui::LayerId::background()).hline(
                r.x_range(),
                r.bottom(),
                egui::Stroke::new(1.0, self.palette.border_subtle),
            );
        }

        // Bottom panel: Status bar
        let status_response = egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::none()
                    .fill(self.palette.bg_surface)
                    .inner_margin(egui::Margin::symmetric(12.0, 4.0)),
            )
            .show(ctx, |ui| {
                let _ = ui.horizontal(|ui| {
                    let _ = ui.label(
                        egui::RichText::new(&self.status_message)
                            .size(12.0)
                            .color(self.palette.text_tertiary),
                    );
                    let _ =
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(page) = &self.page {
                                let _ = ui.label(
                                    egui::RichText::new(format!(
                                        "{} nodes \u{2022} {} styled",
                                        page.doc.dom.len(),
                                        page.doc.styles.len()
                                    ))
                                    .size(12.0)
                                    .color(self.palette.text_tertiary),
                                );
                            }
                        });
                });
            });

        // Paint top separator line for the status bar
        {
            let r = status_response.response.rect;
            let _ = ctx.layer_painter(egui::LayerId::background()).hline(
                r.x_range(),
                r.top(),
                egui::Stroke::new(1.0, self.palette.border_subtle),
            );
        }

        // Right panel: Debug panel (if open)
        if self.debug_panel_open {
            let debug_response = egui::SidePanel::right("debug_panel")
                .min_width(350.0)
                .default_width(400.0)
                .frame(
                    egui::Frame::none()
                        .fill(self.palette.bg_surface)
                        .inner_margin(egui::Margin::same(12.0)),
                )
                .show(ctx, |ui| {
                    let _ = ui.label(
                        egui::RichText::new("Debug Panel")
                            .size(16.0)
                            .color(self.palette.text_primary)
                            .strong(),
                    );
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
                            let _ = ui.colored_label(self.palette.text_secondary, "No page loaded");
                        }
                    });
                });

            // Paint left separator line for the debug panel
            {
                let r = debug_response.response.rect;
                let _ = ctx.layer_painter(egui::LayerId::background()).vline(
                    r.left(),
                    r.y_range(),
                    egui::Stroke::new(1.0, self.palette.border_subtle),
                );
            }
        }

        // Central panel: Page content or landing page
        let _ = egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(self.palette.bg_base)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                if self.page.is_some() {
                    // Get actual viewport size from egui
                    // [§ 9.1.1 The viewport](https://www.w3.org/TR/CSS2/visuren.html#viewport)
                    let available = ui.available_size();
                    let viewport_size = (available.x, available.y);

                    // Recompute layout if viewport changed (mutable borrow scope)
                    {
                        let page = self.page.as_mut().unwrap();
                        if page.last_layout_viewport != Some(viewport_size)
                            && let Some(ref mut root) = page.doc.layout_tree
                        {
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
                            let font_metrics = self.font_provider.metrics();
                            root.layout(
                                initial_containing_block,
                                viewport,
                                &*font_metrics,
                                viewport,
                            );
                            page.last_layout_viewport = Some(viewport_size);
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            {
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
                    let _ = egui::Frame::none()
                        .fill(self.palette.bg_base)
                        .inner_margin(egui::Margin::ZERO)
                        .show(ui, |ui| {
                            // Use auto_shrink(false) to prevent ScrollArea from adding scrollbar
                            // space that would make the content area narrower than the layout
                            // viewport.
                            let _ =
                                egui::ScrollArea::vertical()
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
            let _ = ui.heading(egui::RichText::new("\u{1f428}").size(80.0));
            ui.add_space(16.0);

            let _ = ui.heading(
                egui::RichText::new("Koala")
                    .size(36.0)
                    .color(self.palette.text_primary)
                    .strong(),
            );
            ui.add_space(8.0);

            let _ = ui.label(
                egui::RichText::new("Fast, lightweight HTML-to-image renderer")
                    .size(15.0)
                    .color(self.palette.text_secondary),
            );

            ui.add_space(32.0);

            // Search/URL box with focus-aware border
            let landing_url_id = ui.id().with("landing_url_input");
            let landing_focused = ui.memory(|m| m.has_focus(landing_url_id));
            let landing_border = if landing_focused {
                self.palette.accent
            } else {
                self.palette.border_subtle
            };

            let _ = egui::Frame::none()
                .fill(self.palette.bg_elevated)
                .rounding(egui::Rounding::same(24.0))
                .stroke(egui::Stroke::new(1.0, landing_border))
                .inner_margin(egui::Margin::symmetric(20.0, 12.0))
                .show(ui, |ui| {
                    ui.set_width(400.0);
                    let response = ui.add_sized(
                        [380.0, 24.0],
                        egui::TextEdit::singleline(&mut self.url_input)
                            .id(landing_url_id)
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
                    .color(self.palette.text_tertiary),
            );
            ui.add_space(16.0);

            let _ = ui.horizontal(|ui| {
                ui.add_space((available.x / 2.0 - 200.0).max(0.0));
                ui.spacing_mut().item_spacing.x = 16.0;

                for link in QUICK_LINKS {
                    let button = egui::Button::new(
                        egui::RichText::new(format!("{} {}", link.icon, link.name)).size(14.0),
                    )
                    .fill(self.palette.bg_elevated)
                    .stroke(egui::Stroke::new(1.0, self.palette.border_subtle))
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
                egui::RichText::new(
                    "Press F12 for debug panel \u{2022} Click \u{1f319}/\u{2600}\u{fe0f} to toggle theme",
                )
                .size(12.0)
                .color(self.palette.text_tertiary),
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
        let (_response, _painter) =
            ui.allocate_painter(egui::vec2(total_width, total_height), egui::Sense::hover());

        // The origin is where document (0,0) maps to on screen
        let origin = ui.clip_rect().min;
        self.render_layout_box_absolute(ui, page, layout_box, origin);
    }

    /// Render a layout box using absolute positioning based on computed dimensions
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
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
        let (tag, style) = node_id.map_or((None, None), |id| {
            let style = page.doc.styles.get(&id);
            let tag = page.doc.dom.get(id).and_then(|n| {
                if let NodeType::Element(data) = &n.node_type {
                    Some(data.tag_name.to_lowercase())
                } else {
                    None
                }
            });
            (tag, style)
        });

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

        // Compute border-box rect (used for shadows and background)
        let border_rect = egui::Rect::from_min_size(
            egui::pos2(
                origin.x + dims.content.x - dims.padding.left - dims.border.left,
                origin.y + dims.content.y - dims.padding.top - dims.border.top,
            ),
            egui::vec2(
                dims.content.width
                    + dims.padding.left
                    + dims.padding.right
                    + dims.border.left
                    + dims.border.right,
                dims.content.height
                    + dims.padding.top
                    + dims.padding.bottom
                    + dims.border.top
                    + dims.border.bottom,
            ),
        );

        // [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
        //
        // Outer shadows are painted before the background.
        // Painted in reverse order (last in list = furthest back).
        for shadow in layout_box.box_shadow.iter().rev() {
            if !shadow.inset {
                let shadow_color =
                    egui::Color32::from_rgba_unmultiplied(
                        shadow.color.r,
                        shadow.color.g,
                        shadow.color.b,
                        shadow.color.a,
                    );
                let shadow_rect = border_rect
                    .expand(shadow.spread_radius)
                    .translate(egui::vec2(shadow.offset_x, shadow.offset_y));
                if shadow.blur_radius > 0.0 {
                    // Approximate blur with multiple expanding translucent layers
                    let layers = shadow.blur_radius.ceil() as u32;
                    for layer in 0..layers {
                        let expand = layer as f32;
                        let alpha_frac = 1.0 - (layer as f32 / layers as f32);
                        let layer_color = shadow_color.linear_multiply(alpha_frac);
                        let layer_rect = shadow_rect.expand(expand);
                        let sbr = &layout_box.border_radius;
                        let srounding = egui::Rounding {
                            nw: sbr.top_left,
                            ne: sbr.top_right,
                            sw: sbr.bottom_left,
                            se: sbr.bottom_right,
                        };
                        let _ = ui.painter().rect_filled(layer_rect, srounding, layer_color);
                    }
                } else {
                    let sbr = &layout_box.border_radius;
                    let srounding = egui::Rounding {
                        nw: sbr.top_left,
                        ne: sbr.top_right,
                        sw: sbr.bottom_left,
                        se: sbr.bottom_right,
                    };
                    let _ = ui.painter().rect_filled(shadow_rect, srounding, shadow_color);
                }
            }
        }

        // Paint background if this element has one
        // [CSS Backgrounds § 3.7](https://www.w3.org/TR/css-backgrounds-3/#background-painting-area)
        //
        // "The initial value of 'background-clip' is 'border-box', meaning
        // the background is painted within the border box."
        if let Some(s) = style
            && let Some(ref bg) = s.background_color
        {
            let bg_color = egui::Color32::from_rgba_unmultiplied(bg.r, bg.g, bg.b, bg.a);
            let br = &layout_box.border_radius;
            let rounding = egui::Rounding {
                nw: br.top_left,
                ne: br.top_right,
                sw: br.bottom_left,
                se: br.bottom_right,
            };
            let _ = ui.painter().rect_filled(border_rect, rounding, bg_color);
        }

        // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
        // "When overflow is not 'visible', content is clipped to the padding edge."
        let old_clip = if style.is_some_and(|s| {
            s.overflow
                .is_some_and(|o| o != koala_css::style::computed::Overflow::Visible)
        }) {
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
            let prev = ui.clip_rect();
            ui.set_clip_rect(prev.intersect(padding_rect));
            Some(prev)
        } else {
            None
        };

        // [CSS 2.1 Appendix E.2 Step 5](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the replaced content of replaced inline-level elements"
        //
        // If this is a replaced element (e.g., <img>), paint the image.
        if layout_box.is_replaced
            && let Some(ref src) = layout_box.replaced_src
            && let Some(page_ref) = &self.page
            && let Some(loaded_img) = page_ref.doc.images.get(src)
        {
            let mut textures = self.image_textures.borrow_mut();
            let texture = textures.entry(src.clone()).or_insert_with(|| {
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [loaded_img.width() as usize, loaded_img.height() as usize],
                    loaded_img.rgba_data(),
                );
                ui.ctx()
                    .load_texture(src.clone(), color_image, egui::TextureOptions::LINEAR)
            });
            let img_rect = content_rect;
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            let _ = ui
                .painter()
                .image(texture.id(), img_rect, uv, egui::Color32::WHITE);
        }

        // Determine text formatting from style
        let font_size = style.and_then(|s| s.font_size.as_ref()).map_or_else(
            || match tag.as_deref() {
                Some("h1") => 32.0,
                Some("h2") => 24.0,
                Some("h3") => 18.72,
                // h4 = 16.0 (same as default body text size)
                Some("h5") => 13.28,
                Some("h6") => 10.72,
                _ => 16.0,
            },
            koala_css::LengthValue::to_px,
        );

        let text_color = style.and_then(|s| s.color.as_ref()).map_or_else(
            || ui.visuals().text_color(),
            |c| egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a),
        );

        // Render text content.
        //
        // If this box has line_boxes (established an inline formatting
        // context), paint text from the positioned line fragments.
        // Otherwise fall back to walking DOM children directly.
        if !layout_box.line_boxes.is_empty() {
            for line_box in &layout_box.line_boxes {
                for fragment in &line_box.fragments {
                    if let koala_css::layout::inline::FragmentContent::Text(text_run) =
                        &fragment.content
                    {
                        let frag_color = egui::Color32::from_rgb(
                            text_run.color.r,
                            text_run.color.g,
                            text_run.color.b,
                        );
                        let text_pos =
                            egui::pos2(origin.x + fragment.bounds.x, origin.y + fragment.bounds.y);

                        // Select font family based on weight and style.
                        let is_bold = text_run.font_weight >= 700;
                        let is_italic = text_run.font_style != koala_css::FontStyle::Normal;
                        let font_family = match (is_bold, is_italic) {
                            (true, true) => egui::FontFamily::Name("inter-bold-italic".into()),
                            (true, false) => egui::FontFamily::Name("inter-bold".into()),
                            (false, true) => egui::FontFamily::Name("inter-italic".into()),
                            (false, false) => egui::FontFamily::Proportional,
                        };

                        let galley = ui.painter().layout(
                            text_run.text.clone(),
                            egui::FontId::new(text_run.font_size, font_family),
                            frag_color,
                            fragment.bounds.width.max(content_rect.width()),
                        );
                        ui.painter().galley(text_pos, galley, frag_color);

                        // [§ 3 Text Decoration Lines](https://www.w3.org/TR/css-text-decoration-3/#text-decoration-line-property)
                        //
                        // Draw text decoration lines after the text.
                        let td = text_run.text_decoration;
                        if td.underline || td.overline || td.line_through {
                            let stroke =
                                egui::Stroke::new(1.0, frag_color);
                            let left_x = text_pos.x;
                            let right_x = text_pos.x + fragment.bounds.width;

                            if td.underline {
                                let ly = text_run.font_size.mul_add(0.9, text_pos.y);
                                let _ = ui.painter().line_segment(
                                    [egui::pos2(left_x, ly), egui::pos2(right_x, ly)],
                                    stroke,
                                );
                            }
                            if td.line_through {
                                let ly = text_run.font_size.mul_add(0.55, text_pos.y);
                                let _ = ui.painter().line_segment(
                                    [egui::pos2(left_x, ly), egui::pos2(right_x, ly)],
                                    stroke,
                                );
                            }
                            if td.overline {
                                let ly = text_run.font_size.mul_add(0.1, text_pos.y);
                                let _ = ui.painter().line_segment(
                                    [egui::pos2(left_x, ly), egui::pos2(right_x, ly)],
                                    stroke,
                                );
                            }
                        }
                    }
                }
            }
        } else if let Some(id) = node_id {
            // Select font family from computed style for the fallback path.
            let is_bold = style.and_then(|s| s.font_weight).unwrap_or(400) >= 700;
            let is_italic = style
                .and_then(|s| s.font_style)
                .is_some_and(|s| s != koala_css::FontStyle::Normal);
            let fallback_family = match (is_bold, is_italic) {
                (true, true) => egui::FontFamily::Name("inter-bold-italic".into()),
                (true, false) => egui::FontFamily::Name("inter-bold".into()),
                (false, true) => egui::FontFamily::Name("inter-italic".into()),
                (false, false) => egui::FontFamily::Proportional,
            };

            let mut text_y = content_rect.min.y;
            for &child_id in page.doc.dom.children(id) {
                if let Some(child_node) = page.doc.dom.get(child_id)
                    && let NodeType::Text(text) = &child_node.node_type
                {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        // Paint text at the computed position
                        let text_pos = egui::pos2(content_rect.min.x, text_y);
                        let galley = ui.painter().layout(
                            trimmed.to_string(),
                            #[allow(clippy::cast_possible_truncation)]
                            egui::FontId::new(font_size as f32, fallback_family.clone()),
                            text_color,
                            content_rect.width(),
                        );
                        ui.painter().galley(text_pos, galley.clone(), text_color);
                        text_y += galley.rect.height() + 4.0; // Line spacing
                    }
                }
            }
        }

        // Recursively render children from the layout tree
        for child in &layout_box.children {
            self.render_layout_box_absolute(ui, page, child, origin);
        }

        // Restore previous clip rect if we pushed one for overflow: hidden
        if let Some(prev) = old_clip {
            ui.set_clip_rect(prev);
        }

        // Note: We don't need to call allocate_space() because we're using
        // painter().rect_filled() which draws directly without affecting layout.
        // The ScrollArea will size based on the content naturally.
    }

    /// Recursively render a DOM node's content
    #[allow(clippy::only_used_in_recursion)]
    fn render_node_content(&self, ui: &mut egui::Ui, page: &PageState, id: NodeId, depth: usize) {
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
                    if tag != "body"
                        && let Some(bg) = &s.background_color
                    {
                        self.warn_unsupported_css("background-color", &tag, &bg.to_hex_string());
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
                let font_size = style.and_then(|s| s.font_size.as_ref()).map_or_else(
                    || {
                        // User-agent default sizes when no CSS specified
                        // [§ 15.3.1 HTML headings](https://html.spec.whatwg.org/#sections-and-headings)
                        match tag.as_str() {
                            "h1" => 32.0,
                            "h2" => 24.0,
                            "h3" => 18.72,
                            // h4 = 16.0 (same as default body text size)
                            "h5" => 13.28,
                            "h6" => 10.72,
                            _ => 16.0, // Default body text size
                        }
                    },
                    koala_css::LengthValue::to_px,
                );

                let text_color = style.and_then(|s| s.color.as_ref()).map_or_else(
                    || ui.visuals().text_color(),
                    |c| egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a),
                );

                let is_block = matches!(
                    tag.as_str(),
                    "div"
                        | "p"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "ul"
                        | "ol"
                        | "li"
                        | "article"
                        | "section"
                        | "header"
                        | "footer"
                        | "main"
                        | "nav"
                        | "aside"
                        | "blockquote"
                        | "pre"
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
                                    #[allow(clippy::cast_possible_truncation)]
                                    let job = egui::text::LayoutJob::simple_singleline(
                                        trimmed.to_string(),
                                        egui::FontId::new(
                                            font_size as f32,
                                            egui::FontFamily::Proportional,
                                        ),
                                        text_color,
                                    );
                                    let _ = ui.label(job);
                                }
                            }
                            _ => {
                                self.render_node_content(ui, page, child_id, depth + 1);
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
            println!("[Koala CSS] Ignoring {property}: {value} on <{tag}> (not yet implemented)");
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
                let _ = ui.monospace(format!("{indent}#document"));
            }
            NodeType::Element(data) => {
                let mut label = format!("{}<{}", indent, data.tag_name);
                if let Some(id_attr) = data.attrs.get("id") {
                    use std::fmt::Write;
                    let _ = write!(label, " id=\"{id_attr}\"");
                }
                if let Some(class) = data.attrs.get("class") {
                    use std::fmt::Write;
                    let _ = write!(label, " class=\"{class}\"");
                }
                label.push('>');

                let has_style = page.doc.styles.contains_key(&id);
                if has_style {
                    let _ = ui.horizontal(|ui| {
                        let _ = ui.monospace(&label);
                        let _ = ui.colored_label(self.palette.success, "\u{25cf}");
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
                        self.palette.text_secondary,
                        format!("{indent}\"{preview}\""),
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
                    self.palette.text_tertiary,
                    format!("{indent}<!-- {preview} -->"),
                );
            }
        }

        for &child_id in page.doc.dom.children(id) {
            self.render_debug_node(ui, page, child_id, depth + 1);
        }
    }

    #[allow(clippy::unused_self)]
    fn render_debug_tokens(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!("HTML Tokens ({})", page.doc.tokens.len()))
                .strong()
                .size(13.0),
        );
        ui.add_space(8.0);

        for (i, token) in page.doc.tokens.iter().enumerate() {
            let _ = ui.monospace(format!("{i:4}: {token:?}"));
        }
    }

    fn render_debug_css(&self, ui: &mut egui::Ui, page: &PageState) {
        if page.doc.css_text.is_empty() {
            let _ = ui.colored_label(self.palette.text_secondary, "No CSS found in document");
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
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                let _ = ui.add(
                    egui::TextEdit::multiline(&mut page.doc.css_text.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
    }

    #[allow(clippy::unused_self)]
    fn render_debug_styles(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!(
                "Computed Styles ({} elements)",
                page.doc.styles.len()
            ))
            .strong()
            .size(13.0),
        );
        ui.add_space(8.0);

        for (node_id, style) in &page.doc.styles {
            if let Some(node) = page.doc.dom.get(*node_id)
                && let NodeType::Element(data) = &node.node_type
            {
                let _ = ui.collapsing(format!("<{}> (node {})", data.tag_name, node_id.0), |ui| {
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
                        match m {
                            AutoLength::Length(len) => {
                                let _ = ui.monospace(format!("margin-top: {}px", len.to_px()));
                            }
                            AutoLength::Auto => {
                                let _ = ui.monospace("margin-top: auto");
                            }
                        }
                    }
                    if let Some(ref p) = style.padding_top {
                        let _ = ui.monospace(format!("padding-top: {}px", p.to_px()));
                    }
                });
            }
        }
    }

    #[allow(clippy::unused_self)]
    fn render_debug_source(&self, ui: &mut egui::Ui, page: &PageState) {
        let _ = ui.label(
            egui::RichText::new(format!(
                "HTML Source ({} bytes)",
                page.doc.html_source.len()
            ))
            .strong()
            .size(13.0),
        );
        ui.add_space(8.0);

        let _ = egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .rounding(egui::Rounding::same(6.0))
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
