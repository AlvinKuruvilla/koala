//! Inline `<svg>` content drawn through the renderer.

use koala_browser::{Renderer, RendererFonts, parse_html_string};
use koala_css::{DisplayCommand, DisplayList};
use koala_std::collections::HashMap;

/// Find the first element named `tag` below `id`.
fn find(dom: &koala_dom::DomTree, id: koala_dom::NodeId, tag: &str) -> Option<koala_dom::NodeId> {
    if dom.as_element(id).is_some_and(|e| e.tag_name.as_str() == tag) {
        return Some(id);
    }
    dom.children(id).iter().find_map(|&child| find(dom, child, tag))
}

/// Render `html`'s first `<svg>` into a `size` x `size` box at the origin
/// and return the RGBA pixel at (`x`, `y`).
fn pixel(html: &str, size: f32, x: u32, y: u32) -> [u8; 4] {
    let doc = parse_html_string(html);
    let node = find(&doc.dom, doc.dom.root(), "svg").expect("the page has an <svg>");
    let fonts = RendererFonts { regular: None, bold: None, italic: None, bold_italic: None };
    let mut renderer = Renderer::new_with_fonts(size as u32, size as u32, HashMap::new(), fonts)
        .with_inline_svgs(doc.inline_svgs);
    let mut list = DisplayList::new();
    list.push(DisplayCommand::DrawSvg { x: 0.0, y: 0.0, width: size, height: size, node, opacity: 1.0 });
    renderer.render(&list);
    let i = ((y * size as u32 + x) * 4) as usize;
    renderer.rgba_bytes()[i..i + 4].try_into().expect("four channels")
}

/// Inline SVG draws, and a `viewBox` scales to the box: the 2x2 viewBox
/// square fills all 40x40 pixels, not the top-left 2x2.
#[test]
fn inline_svg_fills_its_box_through_the_view_box() {
    let html = "<svg viewBox='0 0 2 2'><rect width='2' height='2' fill='#ff0000'/></svg>";
    assert_eq!(pixel(html, 40.0, 1, 1), [255, 0, 0, 255]);
    assert_eq!(pixel(html, 40.0, 38, 38), [255, 0, 0, 255]);
}

/// `currentColor` inside the SVG takes the `<svg>` element's CSS color.
#[test]
fn current_color_comes_from_css() {
    let html = "<style>svg { color: #00ff00 }</style>\
                <svg viewBox='0 0 1 1'><rect width='1' height='1' fill='currentColor'/></svg>";
    assert_eq!(pixel(html, 10.0, 5, 5), [0, 255, 0, 255]);
}
