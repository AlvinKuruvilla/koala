//! Shared rendering primitives.
//!
//! Both the `--screenshot` CLI flag and the `--wpt-protocol`
//! mode lay out a [`LoadedDocument`] at a given viewport and
//! write a PNG to disk. The shared work lives here so the two
//! call sites cannot drift.

use anyhow::{Context, Result};
use koala_browser::{FontProvider, LoadedDocument, renderer::Renderer};
use koala_css::DisplayListBuilder;
use std::path::Path;

/// Lay out `doc` at the given viewport, paint the resulting
/// display list, and save the image to `output_path`. The output
/// format is inferred from the file extension by the renderer.
///
/// # Errors
///
/// Returns an error if the document has no layout tree (i.e. parsing
/// failed silently and `LoadedDocument::layout_tree` is `None`), or
/// if the renderer cannot encode or write the image.
#[allow(clippy::cast_precision_loss)] // viewport dimensions don't need full u32 precision
pub(crate) fn render_document_to_path(
    doc: &LoadedDocument,
    output_path: &Path,
    width: u32,
    height: u32,
    font_provider: &FontProvider,
) -> Result<()> {
    let viewport = koala_css::Rect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
    };

    let layout_tree = doc
        .layout_tree
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no layout tree available"))?;

    let mut layout = layout_tree.clone();
    let font_metrics = font_provider.metrics();
    layout.layout(viewport, viewport, &*font_metrics, viewport);

    let builder = DisplayListBuilder::new(&doc.styles);
    let display_list = builder.build(&layout);

    let mut renderer = Renderer::new(width, height, doc.images.clone());
    renderer.render(&display_list);
    renderer
        .save(output_path)
        .context("while attempting to save rendered image")?;
    Ok(())
}
