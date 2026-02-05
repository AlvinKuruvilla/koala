//! Image data types shared across browser components.
//!
//! [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)

/// Decoded image data for a loaded image resource.
///
/// [§ 4.8.3 The img element](https://html.spec.whatwg.org/multipage/embedded-content.html#the-img-element)
///
/// Contains the decoded RGBA pixel data and intrinsic dimensions.
#[derive(Clone)]
pub struct LoadedImage {
    /// Intrinsic width of the image in pixels.
    width: u32,
    /// Intrinsic height of the image in pixels.
    height: u32,
    /// Raw RGBA pixel data (width * height * 4 bytes).
    rgba_data: Vec<u8>,
}

impl LoadedImage {
    /// Create a new `LoadedImage` from decoded RGBA pixel data.
    ///
    /// # Arguments
    ///
    /// * `width` - Intrinsic width of the image in pixels
    /// * `height` - Intrinsic height of the image in pixels
    /// * `rgba_data` - Raw RGBA pixel data (must be `width * height * 4` bytes)
    #[must_use]
    pub fn new(width: u32, height: u32, rgba_data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba_data,
        }
    }

    /// Intrinsic width of the image in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Intrinsic height of the image in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Intrinsic dimensions as `(width, height)` in `f32`, for layout.
    #[must_use]
    pub fn dimensions_f32(&self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }

    /// Raw RGBA pixel data.
    #[must_use]
    pub fn rgba_data(&self) -> &[u8] {
        &self.rgba_data
    }
}
