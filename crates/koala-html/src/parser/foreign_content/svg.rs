//! SVG foreign content support.
//!
//! [§ 13.2.6.3](https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes)

use crate::tokenizer::Attribute;

/// [§ 13.2.6.3 Adjust SVG attributes](https://html.spec.whatwg.org/multipage/parsing.html#adjust-svg-attributes)
///
/// "When the steps below require the user agent to adjust SVG attributes for a
/// token, then, if the attribute's name is one of the names in the first column
/// of the following table, set the attribute's name to the name in the second
/// column."
///
/// This fixes case-sensitivity: HTML lowercases all attribute names during
/// tokenization, but SVG has case-sensitive attribute names.
const SVG_ATTRIBUTE_ADJUSTMENTS: &[(&str, &str)] = &[
    ("attributename", "attributeName"),
    ("attributetype", "attributeType"),
    ("basefrequency", "baseFrequency"),
    ("baseprofile", "baseProfile"),
    ("calcmode", "calcMode"),
    ("clippathunits", "clipPathUnits"),
    ("diffuseconstant", "diffuseConstant"),
    ("edgemode", "edgeMode"),
    ("filterunits", "filterUnits"),
    ("glyphref", "glyphRef"),
    ("gradienttransform", "gradientTransform"),
    ("gradientunits", "gradientUnits"),
    ("kernelmatrix", "kernelMatrix"),
    ("kernelunitlength", "kernelUnitLength"),
    ("keypoints", "keyPoints"),
    ("keysplines", "keySplines"),
    ("keytimes", "keyTimes"),
    ("lengthadjust", "lengthAdjust"),
    ("limitingconeangle", "limitingConeAngle"),
    ("markerheight", "markerHeight"),
    ("markerunits", "markerUnits"),
    ("markerwidth", "markerWidth"),
    ("maskcontentunits", "maskContentUnits"),
    ("maskunits", "maskUnits"),
    ("numoctaves", "numOctaves"),
    ("pathlength", "pathLength"),
    ("patterncontentunits", "patternContentUnits"),
    ("patterntransform", "patternTransform"),
    ("patternunits", "patternUnits"),
    ("pointsatx", "pointsAtX"),
    ("pointsaty", "pointsAtY"),
    ("pointsatz", "pointsAtZ"),
    ("preservealpha", "preserveAlpha"),
    ("preserveaspectratio", "preserveAspectRatio"),
    ("primitiveunits", "primitiveUnits"),
    ("refx", "refX"),
    ("refy", "refY"),
    ("repeatcount", "repeatCount"),
    ("repeatdur", "repeatDur"),
    ("requiredextensions", "requiredExtensions"),
    ("requiredfeatures", "requiredFeatures"),
    ("specularconstant", "specularConstant"),
    ("specularexponent", "specularExponent"),
    ("spreadmethod", "spreadMethod"),
    ("startoffset", "startOffset"),
    ("stddeviation", "stdDeviation"),
    ("stitchtiles", "stitchTiles"),
    ("surfacescale", "surfaceScale"),
    ("systemlanguage", "systemLanguage"),
    ("tablevalues", "tableValues"),
    ("targetx", "targetX"),
    ("targety", "targetY"),
    ("textlength", "textLength"),
    ("viewbox", "viewBox"),
    ("viewtarget", "viewTarget"),
    ("xchannelselector", "xChannelSelector"),
    ("ychannelselector", "yChannelSelector"),
    ("zoomandpan", "zoomAndPan"),
];

/// [§ 13.2.6.3 Adjust SVG attributes](https://html.spec.whatwg.org/multipage/parsing.html#adjust-svg-attributes)
///
/// Adjust attribute names for SVG elements to restore proper casing.
pub fn adjust_svg_attributes(attributes: &mut [Attribute]) {
    for attr in &mut *attributes {
        for &(from, to) in SVG_ATTRIBUTE_ADJUSTMENTS {
            if attr.name == from {
                attr.name = to.to_string();
                break;
            }
        }
    }
}
