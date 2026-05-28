//! Shared rendering primitives.
//!
//! Both the `--screenshot` CLI flag and the `--wpt-protocol` mode
//! lay out a [`LoadedDocument`] at a given viewport and write a
//! PNG to disk. The bench harness runs the same pipeline but
//! repeats it N times and skips the file write. The shared work
//! lives in [`render_document_once`] so all three call sites
//! cannot drift.
//!
//! Per-stage `tracing` instrumentation lives on the small phase
//! helpers below (each `#[tracing::instrument(name = "...", skip_all)]`).
//! The orchestrator reads as a sequence of named function calls;
//! the spans are invisible at the call site. When no subscriber is
//! registered (the screenshot / WPT paths) `tracing` dispatch is
//! a few atomic loads — negligible. The bench harness installs a
//! `Layer` that collects span timings into a per-stage stats map
//! (see `bench.rs`).

use anyhow::{Context, Result};
use koala_browser::{
    FontProvider, LoadedDocument,
    renderer::{Renderer, RendererFonts},
};
use koala_css::{ComputedStyle, DisplayList, DisplayListBuilder, LayoutBox, Rect};
use koala_dom::NodeId;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Process-wide `RendererFonts` cache, mirroring the pattern in
/// `koala-ui/src/browser_page.rs::cached_fonts`. Loading the four
/// font variants from disk costs ~25 ms on macOS; before this
/// cache, every `render_document_once` call paid that cost
/// (`Renderer::new` is the uncached path). With the cache, only
/// the first call in the process loads from disk — bench mode's
/// 30+ iterations now measure the actual render cost, screenshot
/// mode is unaffected (only one render), and the WPT runner
/// amortises the cost across hundreds of test renders.
fn cached_renderer_fonts() -> &'static RendererFonts {
    static FONTS: OnceLock<RendererFonts> = OnceLock::new();
    FONTS.get_or_init(RendererFonts::from_system)
}

/// Run the full layout → display-list → paint pipeline for `doc`
/// at `width`×`height` and return the populated `Renderer`. Callers
/// either save the resulting pixel buffer to disk (screenshot, WPT
/// reference image) or discard it after reading the trace events
/// (bench harness).
///
/// Per-stage span breakdown (recorded under any subscriber that
/// matches `info`-level spans):
///
/// - `render_total` — the whole pipeline.
/// - `layout_clone` — defensive clone of the cached layout tree.
/// - `layout_pass` — recompute box dimensions for the viewport.
/// - `display_list` — walk the laid-out tree, emit paint commands.
/// - `renderer_alloc` — RGBA buffer allocation (inside `Renderer::new_with_fonts`).
/// - `rasterize` — execute the display list against the pixel buffer.
///
/// # Errors
///
/// Returns an error if the document has no layout tree (parsing
/// produced an empty result).
#[allow(clippy::cast_precision_loss)] // viewport dimensions don't need full u32 precision
#[tracing::instrument(name = "render_total", skip_all)]
pub(crate) fn render_document_once(
    doc: &LoadedDocument,
    width: u32,
    height: u32,
    font_provider: &FontProvider,
) -> Result<Renderer> {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
    };

    let layout_tree = doc
        .layout_tree
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no layout tree available"))?;

    let mut layout = clone_layout_tree(layout_tree);
    apply_layout_pass(&mut layout, viewport, font_provider);
    let display_list = build_display_list(&layout, &doc.styles);

    // `Renderer::new_with_fonts` records its own `renderer_alloc`
    // span (the buffer allocation lives inside it). `Renderer::render`
    // records `rasterize`. No span wrappers needed here.
    let mut renderer = Renderer::new_with_fonts(
        width,
        height,
        doc.images.clone(),
        cached_renderer_fonts().clone(),
    );
    renderer.render(&display_list);

    Ok(renderer)
}

/// Defensive clone of the cached layout tree before the in-place
/// layout pass mutates it.
#[tracing::instrument(name = "layout_clone", skip_all)]
fn clone_layout_tree(tree: &LayoutBox) -> LayoutBox {
    tree.clone()
}

/// Recompute box dimensions for the given viewport. Runs every
/// render — the cached layout tree from parse time only has the
/// box structure, not the dimensions, which depend on viewport
/// size and font metrics.
#[tracing::instrument(name = "layout_pass", skip_all)]
fn apply_layout_pass(layout: &mut LayoutBox, viewport: Rect, font_provider: &FontProvider) {
    let font_metrics = font_provider.metrics();
    layout.layout(viewport, viewport, &*font_metrics, viewport);
}

/// Walk the laid-out tree and emit the paint command list the
/// renderer executes.
#[tracing::instrument(name = "display_list", skip_all)]
fn build_display_list(
    layout: &LayoutBox,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> DisplayList {
    let builder = DisplayListBuilder::new(styles);
    builder.build(layout)
}

/// Lay out `doc` at the given viewport, paint the resulting display
/// list, and save the image to `output_path`. The output format is
/// inferred from the file extension by the renderer.
///
/// # Errors
///
/// Returns an error if [`render_document_once`] fails, or if the
/// renderer cannot encode or write the image.
pub(crate) fn render_document_to_path(
    doc: &LoadedDocument,
    output_path: &Path,
    width: u32,
    height: u32,
    font_provider: &FontProvider,
) -> Result<()> {
    let renderer = render_document_once(doc, width, height, font_provider)?;
    renderer
        .save(output_path)
        .context("while attempting to save rendered image")?;
    Ok(())
}
