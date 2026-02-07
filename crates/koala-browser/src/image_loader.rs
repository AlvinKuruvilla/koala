//! Image loading pipeline: fetch, detect format, and decode.
//!
//! [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
//!
//! This module separates the three concerns of image loading into a clean
//! pipeline:
//!
//! 1. **Fetch** — [`fetch_image_bytes()`] consolidates HTTP, data URL, and
//!    local file reads into a single function.
//! 2. **Detect** — [`detect_format()`] determines whether bytes are SVG or
//!    raster using extension, MIME type, and magic-byte sniffing.
//! 3. **Decode** — [`ImageDecoder`] trait with [`SvgDecoder`] and
//!    [`RasterDecoder`] implementations.
//!
//! The [`ImageLoaderPipeline`] ties these together behind a single
//! `decode(bytes, path_for_ext, resolved_url)` entry point.

use koala_common::image::LoadedImage;
use koala_common::warning::warn_once;
use std::fs;

/// Detected image format.
///
/// Only two variants are needed: the `image` crate handles raster sub-format
/// detection (PNG/JPEG/GIF/WebP/…) internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// SVG vector image (decoded via usvg + resvg).
    Svg,
    /// Raster image (PNG, JPEG, GIF, WebP, etc. — decoded via the `image` crate).
    Raster,
}

/// Strip query string (`?…`) and fragment identifier (`#…`) from a URL so
/// that the remaining path can be checked for a file extension.
///
/// [URL Standard § 4.1](https://url.spec.whatwg.org/#concept-url-path)
#[must_use]
pub fn strip_url_decorations(resolved: &str) -> &str {
    let without_fragment = resolved.split_once('#').map_or(resolved, |(b, _)| b);
    without_fragment
        .split_once('?')
        .map_or(without_fragment, |(b, _)| b)
}

/// Emit `warn_once` messages for fragment identifiers and query strings
/// present in an image URL.
pub fn warn_url_decorations(src: &str, resolved: &str) {
    // TODO: Handle SVG fragment identifiers (§ 7.1 of SVG spec) —
    // e.g. `icons.svg#globe-blue` should extract a single element
    // from a sprite sheet rather than rendering the whole document.
    if let Some((_before, frag)) = resolved.split_once('#') {
        warn_once(
            "image",
            &format!(
                "ignoring SVG fragment identifier '#{frag}' in '{src}' \
                 (sprite sheets not yet supported)"
            ),
        );
    }

    // TODO: Handle URL query parameters that hint at image sizing —
    // e.g. `?w=1024` may indicate a server-side resize or could
    // inform client-side rasterization dimensions.
    let without_fragment = resolved.split_once('#').map_or(resolved, |(b, _)| b);
    if let Some((_before, qry)) = without_fragment.split_once('?') {
        warn_once(
            "image",
            &format!(
                "ignoring query string '?{qry}' in '{src}' \
                 (URL parameters not yet handled)"
            ),
        );
    }
}

/// Detect whether `bytes` represent an SVG or a raster image.
///
/// Uses a three-step strategy:
///
/// 1. **Extension check** — fast path for `.svg` in `path_for_ext`.
/// 2. **Data URL MIME check** — `data:image/svg` prefix in `resolved_url`.
/// 3. **Magic-byte sniffing** — trims leading whitespace and checks the
///    first 256 bytes for `<?xml` or `<svg` prefixes.
/// 4. **Default** — [`ImageFormat::Raster`].
#[must_use]
pub fn detect_format(path_for_ext: &str, resolved_url: &str, bytes: &[u8]) -> ImageFormat {
    // 1. Extension check (.svg fast path)
    if std::path::Path::new(path_for_ext)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
    {
        return ImageFormat::Svg;
    }

    // 2. Data URL MIME check
    if resolved_url.starts_with("data:image/svg") {
        return ImageFormat::Svg;
    }

    // 3. Magic-byte sniffing — trim leading whitespace, inspect first 256 bytes
    let trimmed = bytes
        .iter()
        .skip_while(|&&b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r')
        .take(256)
        .copied()
        .collect::<Vec<u8>>();

    if trimmed.starts_with(b"<?xml") || trimmed.starts_with(b"<svg") {
        return ImageFormat::Svg;
    }

    // 4. Default
    ImageFormat::Raster
}

/// A decoder that can turn raw bytes into a [`LoadedImage`].
pub trait ImageDecoder {
    /// Human-readable name (for diagnostics).
    fn name(&self) -> &'static str;

    /// Whether this decoder handles the given format.
    fn supports(&self, format: ImageFormat) -> bool;

    /// Attempt to decode `bytes` into a [`LoadedImage`].
    ///
    /// # Errors
    ///
    /// Returns an error string if the bytes cannot be decoded by this decoder.
    fn decode(&self, bytes: &[u8]) -> Result<LoadedImage, String>;
}

/// Decodes SVG images via usvg → resvg rasterization.
pub struct SvgDecoder;

impl ImageDecoder for SvgDecoder {
    fn name(&self) -> &'static str {
        "SVG (resvg)"
    }

    fn supports(&self, format: ImageFormat) -> bool {
        format == ImageFormat::Svg
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn decode(&self, bytes: &[u8]) -> Result<LoadedImage, String> {
        let opts = usvg::Options::default();
        let tree =
            usvg::Tree::from_data(bytes, &opts).map_err(|e| format!("failed to parse SVG: {e}"))?;

        let size = tree.size();
        let (w, h) = (size.width() as u32, size.height() as u32);
        if w == 0 || h == 0 {
            return Err("SVG has zero-size dimensions".to_string());
        }

        let mut pixmap = tiny_skia::Pixmap::new(w, h)
            .ok_or_else(|| "failed to allocate pixmap for SVG".to_string())?;

        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        Ok(LoadedImage::new(w, h, pixmap.take()))
    }
}

/// Decodes raster images (PNG, JPEG, GIF, WebP, …) via the `image` crate.
pub struct RasterDecoder;

impl ImageDecoder for RasterDecoder {
    fn name(&self) -> &'static str {
        "Raster (image crate)"
    }

    fn supports(&self, format: ImageFormat) -> bool {
        format == ImageFormat::Raster
    }

    fn decode(&self, bytes: &[u8]) -> Result<LoadedImage, String> {
        let dynamic_img =
            image::load_from_memory(bytes).map_err(|e| format!("could not decode image ({e})"))?;
        let rgba = dynamic_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Ok(LoadedImage::new(w, h, rgba.into_raw()))
    }
}

/// Fetch image bytes from `resolved_url`.
///
/// Consolidates the three-way fetch (HTTP, data URL, local file) into one
/// function.
///
/// # Errors
///
/// Returns an error string if the fetch fails (network error, file not found,
/// or invalid data URL).
pub fn fetch_image_bytes(resolved_url: &str) -> Result<Vec<u8>, String> {
    if resolved_url.starts_with("http://") || resolved_url.starts_with("https://") {
        koala_common::net::fetch_bytes(resolved_url)
    } else if resolved_url.starts_with("data:") {
        koala_common::net::fetch_bytes_from_data_url(resolved_url)
    } else {
        fs::read(resolved_url).map_err(|e| format!("failed to read '{resolved_url}': {e}"))
    }
}

/// Image loading pipeline that detects format and dispatches to the
/// appropriate decoder.
pub struct ImageLoaderPipeline {
    decoders: Vec<Box<dyn ImageDecoder>>,
}

impl ImageLoaderPipeline {
    /// Create a pipeline with the default decoders (SVG + raster).
    #[must_use]
    pub fn new() -> Self {
        Self {
            decoders: vec![Box::new(SvgDecoder), Box::new(RasterDecoder)],
        }
    }

    /// Detect the image format and decode `bytes` into a [`LoadedImage`].
    ///
    /// `path_for_ext` is the URL stripped of query/fragment (for extension
    /// checking). `resolved_url` is the full resolved URL (for data-URL MIME
    /// detection).
    ///
    /// # Errors
    ///
    /// Returns an error string if no decoder supports the detected format or
    /// if decoding fails.
    pub fn decode(
        &self,
        bytes: &[u8],
        path_for_ext: &str,
        resolved_url: &str,
    ) -> Result<LoadedImage, String> {
        let format = detect_format(path_for_ext, resolved_url, bytes);

        for decoder in &self.decoders {
            if decoder.supports(format) {
                return decoder.decode(bytes);
            }
        }

        Err(format!("no decoder available for format {format:?}"))
    }
}

impl Default for ImageLoaderPipeline {
    fn default() -> Self {
        Self::new()
    }
}
