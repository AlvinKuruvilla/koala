//! Foreign content parsing support for SVG and MathML.
//!
//! [§ 13.2.6.3 Creating and inserting nodes](https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes)
//! [§ 13.2.6.5 The rules for parsing tokens in foreign content](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inforeign)

pub mod mathml;
pub mod svg;

pub use mathml::adjust_mathml_attributes;
pub use svg::adjust_svg_attributes;

use crate::tokenizer::Attribute;

/// [§ 13.2.6.3 Adjust foreign attributes](https://html.spec.whatwg.org/multipage/parsing.html#adjust-foreign-attributes)
///
/// "When the steps below require the user agent to adjust foreign attributes
/// for a token, then, if any of the attributes on the token match the strings
/// in the first column of the following table, let the attribute be a namespaced
/// attribute, with the prefix being the string in the second column, the local
/// name being the string in the third column, and the namespace being the
/// namespace in the fourth column."
///
/// Format: (`attribute_name`, prefix, `local_name`, namespace)
///
/// NOTE: Our current DOM doesn't support namespaced attributes, so we just
/// adjust the attribute name to include the prefix for now.
const FOREIGN_ATTRIBUTE_ADJUSTMENTS: &[(&str, &str, &str, &str)] = &[
    // XLink namespace attributes
    (
        "xlink:actuate",
        "xlink",
        "actuate",
        "http://www.w3.org/1999/xlink",
    ),
    (
        "xlink:arcrole",
        "xlink",
        "arcrole",
        "http://www.w3.org/1999/xlink",
    ),
    (
        "xlink:href",
        "xlink",
        "href",
        "http://www.w3.org/1999/xlink",
    ),
    (
        "xlink:role",
        "xlink",
        "role",
        "http://www.w3.org/1999/xlink",
    ),
    (
        "xlink:show",
        "xlink",
        "show",
        "http://www.w3.org/1999/xlink",
    ),
    (
        "xlink:title",
        "xlink",
        "title",
        "http://www.w3.org/1999/xlink",
    ),
    (
        "xlink:type",
        "xlink",
        "type",
        "http://www.w3.org/1999/xlink",
    ),
    // XML namespace attributes
    (
        "xml:lang",
        "xml",
        "lang",
        "http://www.w3.org/XML/1998/namespace",
    ),
    (
        "xml:space",
        "xml",
        "space",
        "http://www.w3.org/XML/1998/namespace",
    ),
    // XMLNS namespace attributes
    ("xmlns", "", "xmlns", "http://www.w3.org/2000/xmlns/"),
    (
        "xmlns:xlink",
        "xmlns",
        "xlink",
        "http://www.w3.org/2000/xmlns/",
    ),
];

/// [§ 13.2.6.3 Adjust foreign attributes](https://html.spec.whatwg.org/multipage/parsing.html#adjust-foreign-attributes)
///
/// Adjust namespaced attributes (xlink:href, xml:lang, xmlns, etc.).
///
/// NOTE: Our DOM doesn't currently support namespaced attributes with separate
/// prefix/localName/namespace. For now, we ensure the attribute name is in the
/// correct format (e.g., "xlink:href"). Full namespace support would require
/// DOM changes to store namespace information per attribute.
pub fn adjust_foreign_attributes(attributes: &mut [Attribute]) {
    for attr in &mut *attributes {
        for &(from, prefix, local_name, _namespace) in FOREIGN_ATTRIBUTE_ADJUSTMENTS {
            if attr.name == from {
                // Ensure the attribute name is properly formatted
                // For now, we just keep the prefixed form since our DOM doesn't
                // support separate namespace storage
                if prefix.is_empty() {
                    attr.name = local_name.to_string();
                } else {
                    attr.name = format!("{prefix}:{local_name}");
                }
                break;
            }
        }
    }
}
