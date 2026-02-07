//! Browser chrome theme: color palettes, font setup, and egui visual overrides.
//!
//! This module defines the semantic color palette and visual styling for both
//! the Dark and Light themes. All browser chrome rendering code references
//! [`ColorPalette`] colors instead of hardcoded values.

use egui::{Color32, FontFamily, Rounding, Shadow, Stroke, Vec2, Visuals};

/// Application theme variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// Light theme with bright backgrounds and dark text.
    Light,
    /// Dark theme with deep backgrounds and light text.
    Dark,
}

impl Theme {
    /// Return the opposite theme.
    pub const fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    /// Emoji icon representing the *action* of switching themes.
    ///
    /// When in Light mode the icon shows a moon (switch to dark),
    /// when in Dark mode the icon shows a sun (switch to light).
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Light => "\u{1f319}",       // 🌙
            Self::Dark => "\u{2600}\u{fe0f}", // ☀️
        }
    }

    /// Build the semantic color palette for this theme.
    #[must_use]
    pub fn palette(self) -> ColorPalette {
        match self {
            Self::Dark => ColorPalette::dark(),
            Self::Light => ColorPalette::light(),
        }
    }

    /// Construct fully-customized [`egui::Visuals`] from this theme's palette.
    #[must_use]
    pub fn visuals(self) -> Visuals {
        let p = self.palette();

        let mut v = match self {
            Self::Dark => Visuals::dark(),
            Self::Light => Visuals::light(),
        };

        // -- Global fills --
        v.panel_fill = p.bg_surface;
        v.window_fill = p.bg_surface;
        v.extreme_bg_color = p.bg_input;
        v.faint_bg_color = p.bg_elevated;
        v.code_bg_color = p.bg_elevated;

        // -- Links and semantic colors --
        v.hyperlink_color = p.accent;
        v.warn_fg_color = p.warning;
        v.error_fg_color = p.error;

        // -- Selection --
        v.selection.bg_fill = p.accent_muted;
        v.selection.stroke = Stroke::new(1.0, p.accent);

        // -- Window chrome --
        v.window_rounding = Rounding::same(10.0);
        v.window_shadow = Shadow::NONE;
        v.window_stroke = Stroke::new(1.0, p.border_subtle);
        v.menu_rounding = Rounding::same(8.0);
        v.popup_shadow = Shadow {
            offset: [0.0, 4.0].into(),
            blur: 12.0,
            spread: 0.0,
            color: Color32::from_black_alpha(40),
        };

        // -- Widget states --
        let rounding = Rounding::same(6.0);

        // Non-interactive (labels, separators)
        v.widgets.noninteractive.bg_fill = p.bg_surface;
        v.widgets.noninteractive.weak_bg_fill = p.bg_surface;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_secondary);
        v.widgets.noninteractive.rounding = rounding;

        // Inactive (default button state)
        v.widgets.inactive.bg_fill = p.bg_elevated;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text_secondary);
        v.widgets.inactive.rounding = rounding;

        // Hovered
        v.widgets.hovered.bg_fill = p.bg_hover;
        v.widgets.hovered.weak_bg_fill = p.bg_hover;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border_default);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text_primary);
        v.widgets.hovered.rounding = rounding;
        v.widgets.hovered.expansion = 0.0;

        // Active (pressed)
        v.widgets.active.bg_fill = p.bg_active;
        v.widgets.active.weak_bg_fill = p.bg_active;
        v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.active.fg_stroke = Stroke::new(1.0, p.text_primary);
        v.widgets.active.rounding = rounding;
        v.widgets.active.expansion = 0.0;

        // Open (e.g. combo box open)
        v.widgets.open.bg_fill = p.bg_active;
        v.widgets.open.weak_bg_fill = p.bg_active;
        v.widgets.open.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.open.fg_stroke = Stroke::new(1.0, p.text_primary);
        v.widgets.open.rounding = rounding;

        // -- Misc --
        v.indent_has_left_vline = true;
        v.striped = false;
        v.slider_trailing_fill = true;

        v
    }

    /// Apply this theme's visuals and spacing overrides to the given context.
    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_visuals(self.visuals());

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        ctx.set_style(style);
    }
}

/// Semantic color palette for the browser chrome.
///
/// Every color has a clear role in the UI hierarchy. Constructed per-theme
/// via [`Theme::palette`] and stored on `BrowserApp` for direct reference.
#[derive(Debug, Clone, Copy)]
pub struct ColorPalette {
    // -- Backgrounds (darkest→lightest in dark mode) --
    /// Deepest background: canvas, central panel.
    pub bg_base: Color32,
    /// Surface background: panels (nav bar, status bar, debug panel).
    pub bg_surface: Color32,
    /// Elevated background: cards, URL bar, code blocks.
    pub bg_elevated: Color32,
    /// Input background: text fields.
    pub bg_input: Color32,
    /// Hover-state background for buttons.
    pub bg_hover: Color32,
    /// Active/pressed-state background for buttons.
    pub bg_active: Color32,

    // -- Borders --
    /// Subtle border: panel edges, separator lines.
    pub border_subtle: Color32,
    /// Default border: input fields, focus rings.
    pub border_default: Color32,

    // -- Text --
    /// Primary text: headings, important content.
    pub text_primary: Color32,
    /// Secondary text: labels, descriptions.
    pub text_secondary: Color32,
    /// Tertiary text: hints, keyboard shortcuts, timestamps.
    pub text_tertiary: Color32,

    // -- Accent --
    /// Primary accent: links, focus indicators, active elements.
    pub accent: Color32,
    /// Accent hover state.
    pub accent_hover: Color32,
    /// Accent at low opacity: selection backgrounds, active tab fill.
    pub accent_muted: Color32,

    // -- Semantic --
    /// Success indicator (e.g. styled-node dot in debug panel).
    pub success: Color32,
    /// Warning text (e.g. parse warnings).
    pub warning: Color32,
    /// Error text (e.g. parse errors).
    pub error: Color32,
}

impl ColorPalette {
    /// Dark palette with blue accent.
    fn dark() -> Self {
        Self {
            bg_base: Color32::from_rgb(13, 14, 18),
            bg_surface: Color32::from_rgb(19, 20, 26),
            bg_elevated: Color32::from_rgb(26, 27, 35),
            bg_input: Color32::from_rgb(30, 31, 40),
            bg_hover: Color32::from_rgb(37, 38, 49),
            bg_active: Color32::from_rgb(46, 47, 61),
            border_subtle: Color32::from_rgb(42, 43, 54),
            border_default: Color32::from_rgb(58, 59, 74),
            text_primary: Color32::from_rgb(232, 233, 237),
            text_secondary: Color32::from_rgb(147, 148, 161),
            text_tertiary: Color32::from_rgb(92, 93, 110),
            accent: Color32::from_rgb(59, 130, 246),
            accent_hover: Color32::from_rgb(96, 165, 250),
            accent_muted: Color32::from_rgba_unmultiplied(59, 130, 246, 64),
            success: Color32::from_rgb(74, 222, 128),
            warning: Color32::from_rgb(251, 191, 36),
            error: Color32::from_rgb(248, 113, 113),
        }
    }

    /// Light palette with blue accent.
    fn light() -> Self {
        Self {
            bg_base: Color32::from_rgb(255, 255, 255),
            bg_surface: Color32::from_rgb(248, 249, 251),
            bg_elevated: Color32::from_rgb(240, 241, 244),
            bg_input: Color32::from_rgb(235, 237, 242),
            bg_hover: Color32::from_rgb(229, 231, 237),
            bg_active: Color32::from_rgb(218, 220, 229),
            border_subtle: Color32::from_rgb(229, 231, 237),
            border_default: Color32::from_rgb(209, 211, 220),
            text_primary: Color32::from_rgb(26, 27, 35),
            text_secondary: Color32::from_rgb(107, 108, 126),
            text_tertiary: Color32::from_rgb(147, 148, 161),
            accent: Color32::from_rgb(37, 99, 235),
            accent_hover: Color32::from_rgb(29, 78, 216),
            accent_muted: Color32::from_rgba_unmultiplied(37, 99, 235, 32),
            success: Color32::from_rgb(34, 197, 94),
            warning: Color32::from_rgb(217, 119, 6),
            error: Color32::from_rgb(220, 38, 38),
        }
    }
}

/// Load the Inter font family (regular, bold, italic, bold-italic) and
/// register font families for content rendering.
///
/// Called once at startup from [`BrowserApp::new`].
///
/// Font families registered:
/// - `Proportional` — Inter Regular (default for all UI and normal-weight text)
/// - `Name("inter-bold")` — Inter Bold (used when `font-weight >= 700`)
/// - `Name("inter-italic")` — Inter Italic (used when `font-style: italic/oblique`)
/// - `Name("inter-bold-italic")` — Inter Bold Italic (bold + italic combined)
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Regular
    let _ = fonts.font_data.insert(
        "inter".to_owned(),
        egui::FontData::from_static(include_bytes!("../../res/fonts/Inter-Regular.ttf")),
    );

    // Bold
    let _ = fonts.font_data.insert(
        "inter-bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../../res/fonts/Inter-Bold.ttf")),
    );

    // Italic
    let _ = fonts.font_data.insert(
        "inter-italic".to_owned(),
        egui::FontData::from_static(include_bytes!("../../res/fonts/Inter-Italic.ttf")),
    );

    // Bold Italic
    let _ = fonts.font_data.insert(
        "inter-bold-italic".to_owned(),
        egui::FontData::from_static(include_bytes!("../../res/fonts/Inter-BoldItalic.ttf")),
    );

    // Insert Inter Regular at the front of Proportional so it takes priority.
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());

    // Register bold as a named family.
    fonts
        .families
        .entry(FontFamily::Name("inter-bold".into()))
        .or_default()
        .push("inter-bold".to_owned());

    // Register italic as a named family.
    fonts
        .families
        .entry(FontFamily::Name("inter-italic".into()))
        .or_default()
        .push("inter-italic".to_owned());

    // Register bold-italic as a named family.
    fonts
        .families
        .entry(FontFamily::Name("inter-bold-italic".into()))
        .or_default()
        .push("inter-bold-italic".to_owned());

    ctx.set_fonts(fonts);
}
