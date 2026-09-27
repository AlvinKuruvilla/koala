//! Drawing commands far larger than the canvas.

use koala_browser::{Renderer, RendererFonts};
use koala_css::{BorderRadius, ColorValue, DisplayCommand, DisplayList};
use koala_std::collections::HashMap;

/// A rectangle f32::MAX tall fills the canvas and returns. The fill loop
/// used to walk all of its ~4 billion rows (the height saturates to
/// u32::MAX), rejecting each pixel, which hung the renderer on Google.
#[test]
fn huge_fill_rect_is_clipped_to_the_canvas() {
    let fonts = RendererFonts { regular: None, bold: None, italic: None, bold_italic: None };
    let mut renderer = Renderer::new_with_fonts(4, 4, HashMap::new(), fonts);
    let mut list = DisplayList::new();
    list.push(DisplayCommand::FillRect {
        x: 0.0,
        y: 0.0,
        width: 4.0,
        height: f32::MAX,
        color: ColorValue { r: 255, g: 0, b: 0, a: 255 },
        border_radius: BorderRadius::default(),
    });
    renderer.render(&list);
    assert_eq!(&renderer.rgba_bytes()[..4], &[255, 0, 0, 255]);
    assert_eq!(&renderer.rgba_bytes()[60..64], &[255, 0, 0, 255]);
}
