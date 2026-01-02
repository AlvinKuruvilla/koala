//! MathML foreign content support.
//!
//! [§ 13.2.6.3](https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes)

use crate::tokenizer::Attribute;

/// [§ 13.2.6.3 Adjust MathML attributes](https://html.spec.whatwg.org/multipage/parsing.html#adjust-mathml-attributes)
///
/// "When the steps below require the user agent to adjust MathML attributes for
/// a token, then, if the attribute's name is 'definitionurl', set the
/// attribute's name to 'definitionURL'."
const MATHML_ATTRIBUTE_ADJUSTMENTS: &[(&str, &str)] = &[("definitionurl", "definitionURL")];

/// [§ 13.2.6.3 Adjust MathML attributes](https://html.spec.whatwg.org/multipage/parsing.html#adjust-mathml-attributes)
///
/// Adjust attribute names for MathML elements to restore proper casing.
pub fn adjust_mathml_attributes(attributes: &mut [Attribute]) {
    for attr in attributes.iter_mut() {
        for &(from, to) in MATHML_ATTRIBUTE_ADJUSTMENTS {
            if attr.name == from {
                attr.name = to.to_string();
                break;
            }
        }
    }
}
