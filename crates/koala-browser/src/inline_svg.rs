//! Inline `<svg>` elements, parsed into `usvg` trees for the renderer.
//!
//! Layout treats an `<svg>` in HTML content as a replaced element and
//! paints a `DrawSvg` command for its box. The content of that box is the
//! element's own subtree, which this module writes back out as standalone
//! SVG markup, once per load.
//!
//! The box is the SVG viewport. `usvg` sizes the viewport from the root's
//! `width` and `height` and fits the `viewBox` into it according to
//! `preserveAspectRatio`, or draws at 1:1 when there is no `viewBox`, so the
//! renderer parses the markup with `width` and `height` set to the box it
//! was given ([`InlineSvg::tree`]). That keeps the drawing sharp and
//! correctly fitted at whatever size layout chose, at the cost of a parse
//! per draw.
//!
//! Page CSS does not reach inside the SVG: `usvg` sees the subtree's
//! attributes and any `<style>` inside it, not the document's stylesheets.
//! The one exception is `currentColor`, which is resolved from the `<svg>`
//! element's computed CSS `color` by setting a `color` attribute on the
//! root; `usvg` inherits it down the tree.

use std::fmt::Write as _;
use std::sync::Arc;

use koala_common::diagnostics::{self, Diagnostic};
use koala_css::ComputedStyle;
use koala_dom::{DomTree, Namespace, NodeId, NodeType};
use koala_std::collections::HashMap;

/// An inline `<svg>` element's content as standalone SVG markup, ready to
/// be parsed at the size of the box it is drawn into.
#[derive(Debug)]
pub struct InlineSvg {
    /// The markup after the root's `<svg` tag name: its attributes (without
    /// `width` and `height`), content and end tag.
    after_tag_name: String,
}

impl InlineSvg {
    /// The full markup with the viewport set to `width` x `height` px.
    fn markup(&self, width: f32, height: f32) -> String {
        let mut markup = String::with_capacity(self.after_tag_name.len() + 40);
        let _ = write!(markup, "<svg width=\"{width}\" height=\"{height}\"");
        markup.push_str(&self.after_tag_name);
        markup
    }

    /// The content parsed with its viewport set to `width` x `height` px.
    /// `None` if `usvg` rejects it, which [`load_inline_svgs`] has already
    /// reported for the document.
    #[must_use]
    pub fn tree(&self, width: f32, height: f32) -> Option<usvg::Tree> {
        usvg::Tree::from_str(&self.markup(width, height), &usvg::Options::default()).ok()
    }
}

/// Every outermost `<svg>` in `dom`, keyed by the element. Markup `usvg`
/// cannot parse is reported and left out, so its box stays empty.
#[must_use]
pub fn load_inline_svgs(
    dom: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> HashMap<NodeId, Arc<InlineSvg>> {
    let mut trees = HashMap::default();
    collect(dom, styles, dom.root(), &mut trees);
    trees
}

/// Find the outermost `<svg>` elements under `id` and parse each one. The
/// parser only puts an element in the SVG namespace inside an `<svg>`, so
/// the first SVG-namespace `<svg>` on a path from the root is outermost, and
/// nothing below it needs visiting.
fn collect(
    dom: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    id: NodeId,
    trees: &mut HashMap<NodeId, Arc<InlineSvg>>,
) {
    if let Some(element) = dom.as_element(id)
        && element.namespace == Namespace::Svg
        && element.tag_name.as_str() == "svg"
    {
        let color = styles.get(&id).and_then(|style| style.color.clone());
        let svg = InlineSvg {
            after_tag_name: serialize_after_tag_name(dom, id, color.as_ref()),
        };
        // Parse once now so broken markup is reported with the rest of the
        // load's problems; the renderer's own parses run on another thread,
        // whose reports nobody collects. The size is a placeholder.
        match usvg::Tree::from_str(&svg.markup(100.0, 100.0), &usvg::Options::default()) {
            Ok(_) => {
                let _ = trees.insert(id, Arc::new(svg));
            }
            Err(error) => diagnostics::report(|| Diagnostic::InlineSvgNotRendered {
                error: error.to_string(),
            }),
        }
        return;
    }
    for &child in dom.children(id) {
        collect(dom, styles, child, trees);
    }
}

/// The `<svg>` subtree at `root` as standalone SVG markup, minus the root's
/// `<svg` and its `width` and `height` (see [`InlineSvg`]).
///
/// This is a minimal XML writer, enough for `usvg`, rather than the DOM
/// Parsing spec's XML serialization algorithm: it declares the SVG and
/// XLink namespaces on the root, writes element and attribute names as the
/// HTML parser left them (already case-corrected for SVG), escapes text and
/// attribute values, and drops comments.
fn serialize_after_tag_name(
    dom: &DomTree,
    root: NodeId,
    color: Option<&koala_css::ColorValue>,
) -> String {
    let mut out = String::new();
    out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    out.push_str(" xmlns:xlink=\"http://www.w3.org/1999/xlink\"");
    // `usvg` resolves `currentColor` from the nearest `color` attribute.
    if let Some(c) = color {
        let _ = write!(out, " color=\"#{:02x}{:02x}{:02x}{:02x}\"", c.r, c.g, c.b, c.a);
    }
    if let Some(element) = dom.as_element(root) {
        // The viewport size comes from the box, and the declarations and
        // color were written above; copies from the document would conflict.
        let skip = ["width", "height", "xmlns", "xmlns:xlink", "color"];
        write_attributes(element, &skip, &mut out);
    }
    out.push('>');
    for &child in dom.children(root) {
        write_node(dom, child, &mut out);
    }
    out.push_str("</svg>");
    out
}

/// Write the node `id` and its descendants.
fn write_node(dom: &DomTree, id: NodeId, out: &mut String) {
    let Some(node) = dom.get(id) else {
        return;
    };
    match &node.node_type {
        NodeType::Element(element) => {
            out.push('<');
            out.push_str(element.tag_name.as_str());
            write_attributes(element, &[], out);
            out.push('>');
            for &child in dom.children(id) {
                write_node(dom, child, out);
            }
            out.push_str("</");
            out.push_str(element.tag_name.as_str());
            out.push('>');
        }
        NodeType::Text(text) => escape_into(text, false, out),
        NodeType::Comment(_) | NodeType::Document => {}
    }
}

/// Write `element`'s attributes, except those named in `skip`.
fn write_attributes(element: &koala_dom::ElementData, skip: &[&str], out: &mut String) {
    for (name, value) in &element.attrs {
        if skip.contains(&name.as_str()) {
            continue;
        }
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape_into(value, true, out);
        out.push('"');
    }
}

/// Append `text` with the characters XML requires escaped: `&` and `<`
/// everywhere, `>` for symmetry, and `"` inside a double-quoted attribute.
fn escape_into(text: &str, in_attribute: bool, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if in_attribute => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koala_dom::{AttributesMap, ElementData};

    fn svg_element(tree: &mut DomTree, tag: &str, attrs: &[(&str, &str)]) -> NodeId {
        let mut map = AttributesMap::new();
        for &(name, value) in attrs {
            let _ = map.insert(name.to_string(), value.to_string());
        }
        tree.alloc(NodeType::Element(ElementData {
            tag_name: tag.into(),
            attrs: map,
            namespace: Namespace::Svg,
        }))
    }

    /// The root carries the viewport size, the namespace declarations and
    /// the CSS color, and attribute values and text are escaped. The
    /// document's `width` and `height` are replaced by the box size.
    #[test]
    fn serializes_a_standalone_document() {
        let mut tree = DomTree::new();
        let svg = svg_element(&mut tree, "svg", &[("viewBox", "0 0 10 10"), ("width", "99")]);
        let text = svg_element(&mut tree, "text", &[("data-x", "a\"b")]);
        let content = tree.alloc(NodeType::Text("1 < 2 & 3".to_string()));
        tree.append_child(NodeId::ROOT, svg);
        tree.append_child(svg, text);
        tree.append_child(text, content);

        let color = koala_css::ColorValue { r: 255, g: 0, b: 16, a: 128 };
        let inline = InlineSvg {
            after_tag_name: serialize_after_tag_name(&tree, svg, Some(&color)),
        };
        assert_eq!(
            inline.markup(24.0, 12.0),
            "<svg width=\"24\" height=\"12\" xmlns=\"http://www.w3.org/2000/svg\" \
             xmlns:xlink=\"http://www.w3.org/1999/xlink\" color=\"#ff001080\" \
             viewBox=\"0 0 10 10\"><text data-x=\"a&quot;b\">1 &lt; 2 &amp; 3</text></svg>"
        );
        assert!(inline.tree(24.0, 12.0).is_some(), "usvg parses the markup");
    }
}
