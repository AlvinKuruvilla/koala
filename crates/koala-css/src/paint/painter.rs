//! Painter - generates display list from layout tree
//!
//! [CSS 2.1 Appendix E.2 Painting order](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
//!
//! The painter walks the layout tree and generates drawing commands in the
//! correct painting order (back to front).

use std::collections::HashMap;

use koala_dom::NodeId;

use crate::layout::inline::{FontStyle, FragmentContent};
use crate::style::ComputedStyle;
use crate::{BoxType, ColorValue, LayoutBox};

use super::{DisplayCommand, DisplayList};

/// Painter that generates a display list from a layout tree.
///
/// [CSS 2.1 Appendix E.2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
///
/// The painter implements the CSS painting order algorithm, which determines
/// what gets drawn and in what order.
pub struct Painter<'a> {
    /// Computed styles for each node, used to get colors, fonts, etc.
    styles: &'a HashMap<NodeId, ComputedStyle>,
}

impl<'a> Painter<'a> {
    /// Create a new painter with access to computed styles.
    #[must_use]
    pub fn new(styles: &'a HashMap<NodeId, ComputedStyle>) -> Self {
        Self { styles }
    }

    /// Paint a layout tree and return the display list.
    ///
    /// [CSS 2.1 Appendix E.2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
    ///
    /// This is the main entry point for painting. It walks the layout tree
    /// and generates all drawing commands in the correct order.
    #[must_use]
    pub fn paint(&self, layout: &LayoutBox) -> DisplayList {
        let mut display_list = DisplayList::new();
        self.paint_box(layout, &mut display_list, None);
        display_list
    }

    /// Paint a single layout box and its descendants.
    ///
    /// [CSS 2.1 Appendix E.2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
    ///
    /// For each stacking context, the following layers are painted in order:
    /// 1. Background color
    /// 2. Background image
    /// 3. Border
    /// 4. Descendants (in tree order for non-positioned, non-floated elements)
    /// 5. Outline
    fn paint_box(
        &self,
        layout_box: &LayoutBox,
        display_list: &mut DisplayList,
        parent_style: Option<&ComputedStyle>,
    ) {
        let dims = &layout_box.dimensions;

        // Get style for this box if it has a node
        let style = match &layout_box.box_type {
            BoxType::Principal(node_id) => self.styles.get(node_id),
            _ => None,
        };

        // Use own style or inherit from parent for certain properties
        let effective_style = style.or(parent_style);

        // Calculate the padding box (used for border painting reference).
        let padding_x = dims.content.x - dims.padding.left;
        let padding_y = dims.content.y - dims.padding.top;
        let padding_width = dims.content.width + dims.padding.left + dims.padding.right;
        let padding_height = dims.content.height + dims.padding.top + dims.padding.bottom;

        // [CSS 2.1 Appendix E.2 Step 2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the background color of the element"
        if let Some(style) = style {
            if let Some(bg) = &style.background_color {
                // [CSS Backgrounds § 3.7](https://www.w3.org/TR/css-backgrounds-3/#background-painting-area)
                //
                // "The initial value of 'background-clip' is 'border-box', meaning
                // the background is painted within the border box."
                let border_top = style.border_top.as_ref().map_or(0.0, |b| b.width.to_px() as f32);
                let border_right = style.border_right.as_ref().map_or(0.0, |b| b.width.to_px() as f32);
                let border_bottom = style.border_bottom.as_ref().map_or(0.0, |b| b.width.to_px() as f32);
                let border_left = style.border_left.as_ref().map_or(0.0, |b| b.width.to_px() as f32);

                display_list.push(DisplayCommand::FillRect {
                    x: padding_x - border_left,
                    y: padding_y - border_top,
                    width: padding_width + border_left + border_right,
                    height: padding_height + border_top + border_bottom,
                    color: bg.clone(),
                });
            }

            // [CSS 2.1 Appendix E.2 Step 2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
            // "the border of the element"
            self.paint_borders(style, padding_x, padding_y, padding_width, padding_height, display_list);
        }

        // [CSS 2.1 Appendix E.2 Step 5](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the replaced content of replaced inline-level elements"
        //
        // If this is a replaced element (e.g., <img>), emit a DrawImage
        // command using the content rect dimensions and src attribute.
        if layout_box.is_replaced {
            if let Some(ref src) = layout_box.replaced_src {
                display_list.push(DisplayCommand::DrawImage {
                    x: dims.content.x,
                    y: dims.content.y,
                    width: dims.content.width,
                    height: dims.content.height,
                    src: src.clone(),
                });
            }
        }

        // [CSS 2.1 Appendix E.2 Step 7](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the element's text"
        //
        // If this box has line_boxes (i.e., it established an inline
        // formatting context), paint text from the line box fragments.
        // These fragments have correct positions computed by InlineLayout.
        if !layout_box.line_boxes.is_empty() {
            for line_box in &layout_box.line_boxes {
                for fragment in &line_box.fragments {
                    if let FragmentContent::Text(text_run) = &fragment.content {
                        display_list.push(DisplayCommand::DrawText {
                            x: fragment.bounds.x,
                            y: fragment.bounds.y,
                            text: text_run.text.clone(),
                            font_size: text_run.font_size,
                            color: text_run.color.clone(),
                            font_weight: text_run.font_weight,
                            font_style: text_run.font_style,
                        });
                    }
                }
            }
        } else if let BoxType::AnonymousInline(text) = &layout_box.box_type {
            // Fallback for AnonymousInline boxes that are NOT part of an
            // inline formatting context (e.g., text directly in a block
            // whose height was computed by calculate_block_height STEP 2).
            let text_color = effective_style
                .and_then(|s| s.color.as_ref())
                .cloned()
                .unwrap_or(ColorValue::BLACK);

            let font_size = effective_style
                .and_then(|s| s.font_size.as_ref())
                .map(|fs| fs.to_px() as f32)
                .unwrap_or(16.0);

            let font_weight = effective_style
                .and_then(|s| s.font_weight)
                .unwrap_or(400);

            let font_style = effective_style
                .and_then(|s| s.font_style.as_deref())
                .map(FontStyle::from_css)
                .unwrap_or_default();

            display_list.push(DisplayCommand::DrawText {
                x: dims.content.x,
                y: dims.content.y,
                text: text.clone(),
                font_size,
                color: text_color,
                font_weight,
                font_style,
            });
        }

        // [CSS 2.1 Appendix E.2 Step 4](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the in-flow, non-inline-level, non-positioned descendants"
        //
        // Paint children in tree order. For boxes with line_boxes, the
        // inline children's text is already painted above from fragments,
        // but we still recurse for backgrounds/borders of child elements.
        for child in &layout_box.children {
            self.paint_box(child, display_list, effective_style);
        }
    }

    /// Paint borders for a box.
    ///
    /// [CSS Backgrounds and Borders § 4](https://www.w3.org/TR/css-backgrounds-3/#borders)
    ///
    /// Borders are drawn outside the padding box. For simplicity, we draw solid
    /// rectangles for each border side (ignoring border-style for now — all styles
    /// render as solid).
    fn paint_borders(
        &self,
        style: &ComputedStyle,
        padding_x: f32,
        padding_y: f32,
        padding_width: f32,
        padding_height: f32,
        display_list: &mut DisplayList,
    ) {
        // Get border widths (default to 0 if not set)
        let top_width = style
            .border_top
            .as_ref()
            .map(|b| b.width.to_px() as f32)
            .unwrap_or(0.0);
        let right_width = style
            .border_right
            .as_ref()
            .map(|b| b.width.to_px() as f32)
            .unwrap_or(0.0);
        let bottom_width = style
            .border_bottom
            .as_ref()
            .map(|b| b.width.to_px() as f32)
            .unwrap_or(0.0);
        let left_width = style
            .border_left
            .as_ref()
            .map(|b| b.width.to_px() as f32)
            .unwrap_or(0.0);

        // Top border: spans full width including corners
        if let Some(border) = &style.border_top {
            if top_width > 0.0 {
                display_list.push(DisplayCommand::FillRect {
                    x: padding_x - left_width,
                    y: padding_y - top_width,
                    width: padding_width + left_width + right_width,
                    height: top_width,
                    color: border.color.clone(),
                });
            }
        }

        // Bottom border: spans full width including corners
        if let Some(border) = &style.border_bottom {
            if bottom_width > 0.0 {
                display_list.push(DisplayCommand::FillRect {
                    x: padding_x - left_width,
                    y: padding_y + padding_height,
                    width: padding_width + left_width + right_width,
                    height: bottom_width,
                    color: border.color.clone(),
                });
            }
        }

        // Left border: between top and bottom borders
        if let Some(border) = &style.border_left {
            if left_width > 0.0 {
                display_list.push(DisplayCommand::FillRect {
                    x: padding_x - left_width,
                    y: padding_y,
                    width: left_width,
                    height: padding_height,
                    color: border.color.clone(),
                });
            }
        }

        // Right border: between top and bottom borders
        if let Some(border) = &style.border_right {
            if right_width > 0.0 {
                display_list.push(DisplayCommand::FillRect {
                    x: padding_x + padding_width,
                    y: padding_y,
                    width: right_width,
                    height: padding_height,
                    color: border.color.clone(),
                });
            }
        }
    }
}
