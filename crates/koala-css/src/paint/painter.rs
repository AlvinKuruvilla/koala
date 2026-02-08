//! Painter - generates display list from layout tree
//!
//! [CSS 2.1 Appendix E.2 Painting order](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
//!
//! The painter walks the layout tree and generates drawing commands in the
//! correct painting order (back to front).

use std::collections::HashMap;

use koala_dom::NodeId;

use crate::layout::inline::FragmentContent;
use crate::layout::positioned::PositionType;
use crate::style::ComputedStyle;
use crate::style::BorderRadius;
use crate::{BoxType, LayoutBox};

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
    pub const fn new(styles: &'a HashMap<NodeId, ComputedStyle>) -> Self {
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
    #[allow(clippy::cast_possible_truncation)]
    fn paint_box(
        &self,
        layout_box: &LayoutBox,
        display_list: &mut DisplayList,
        parent_style: Option<&ComputedStyle>,
    ) {
        // [§ 11.2 'visibility'](https://www.w3.org/TR/CSS2/visufx.html#visibility)
        //
        // "The 'visibility' property specifies whether the boxes generated
        // by an element are rendered. Invisible boxes still affect layout.
        // ...the box and its content are invisible."
        //
        // [§ 3.2 'opacity'](https://www.w3.org/TR/css-color-4/#transparency)
        //
        // "If opacity is 0, the element is fully transparent (invisible)."
        //
        // NOTE: visibility is inherited, so child elements also get hidden
        // unless they override with visibility: visible. We check both
        // visibility and opacity: boxes that are hidden or fully transparent
        // are skipped entirely (no background, border, or text drawn).
        // Children are still visited because visibility inherits — a child
        // could override back to visible.
        let is_visible = layout_box.visibility == crate::style::computed::Visibility::Visible
            && layout_box.opacity > 0.0;

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

        // Compute border-box coordinates once for shadow + background + border painting.
        //
        // [CSS Backgrounds § 3.7](https://www.w3.org/TR/css-backgrounds-3/#background-painting-area)
        //
        // "The initial value of 'background-clip' is 'border-box', meaning
        // the background is painted within the border box."
        let border_box_x = padding_x - dims.border.left;
        let border_box_y = padding_y - dims.border.top;
        let border_box_width =
            padding_width + dims.border.left + dims.border.right;
        let border_box_height =
            padding_height + dims.border.top + dims.border.bottom;

        // [CSS 2.1 Appendix E.2 Step 2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the background color of the element"
        //
        // Only paint background/border/text if the box is visible.
        if let Some(style) = style
            && is_visible
        {
            // [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
            //
            // "An outer box-shadow casts a shadow as if the border-box of the
            // element were opaque. ...it is painted below the background of the
            // element, but above the background and border of the element below it."
            //
            // Painted in reverse order: last shadow in the list is painted first
            // (furthest back), first shadow is painted last (on top).
            for shadow in layout_box.box_shadow.iter().rev() {
                if !shadow.inset {
                    display_list.push(DisplayCommand::DrawBoxShadow {
                        border_box_x,
                        border_box_y,
                        border_box_width,
                        border_box_height,
                        offset_x: shadow.offset_x,
                        offset_y: shadow.offset_y,
                        blur_radius: shadow.blur_radius,
                        spread_radius: shadow.spread_radius,
                        color: shadow.color.clone(),
                        inset: false,
                    });
                }
            }

            if let Some(bg) = &style.background_color {
                display_list.push(DisplayCommand::FillRect {
                    x: border_box_x,
                    y: border_box_y,
                    width: border_box_width,
                    height: border_box_height,
                    color: bg.clone(),
                    border_radius: layout_box.border_radius,
                });
            }

            // [CSS 2.1 Appendix E.2 Step 2](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
            // "the border of the element"
            self.paint_borders(
                style,
                padding_x,
                padding_y,
                padding_width,
                padding_height,
                display_list,
            );

            // [§ 6.1 'box-shadow'](https://www.w3.org/TR/css-backgrounds-3/#box-shadow)
            //
            // "The inner shadow is drawn inside the padding edge:
            // ...it is painted above the background of the element
            // but below the content (including the border)."
            for shadow in layout_box.box_shadow.iter().rev() {
                if shadow.inset {
                    display_list.push(DisplayCommand::DrawBoxShadow {
                        border_box_x,
                        border_box_y,
                        border_box_width,
                        border_box_height,
                        offset_x: shadow.offset_x,
                        offset_y: shadow.offset_y,
                        blur_radius: shadow.blur_radius,
                        spread_radius: shadow.spread_radius,
                        color: shadow.color.clone(),
                        inset: true,
                    });
                }
            }
        }

        // [§ 11.1.1 overflow](https://www.w3.org/TR/CSS2/visufx.html#overflow)
        // "When overflow is not 'visible', content is clipped to the padding edge."
        let needs_clip = style.is_some_and(|s| {
            s.overflow
                .is_some_and(|o| o != crate::style::computed::Overflow::Visible)
        });
        if needs_clip {
            display_list.push(DisplayCommand::PushClip {
                x: padding_x,
                y: padding_y,
                width: padding_width,
                height: padding_height,
            });
        }

        // Only paint content (images, text) if the box is visible.
        if is_visible {
            // [CSS 2.1 Appendix E.2 Step 5](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
            // "the replaced content of replaced inline-level elements"
            if layout_box.is_replaced
                && let Some(ref src) = layout_box.replaced_src
            {
                display_list.push(DisplayCommand::DrawImage {
                    x: dims.content.x,
                    y: dims.content.y,
                    width: dims.content.width,
                    height: dims.content.height,
                    src: src.clone(),
                });
            }

            // [CSS 2.1 Appendix E.2 Step 7](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
            // "the element's text"
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
                                text_decoration: text_run.text_decoration,
                            });
                        }
                    }
                }
            }
            // NOTE: AnonymousInline text is NOT drawn here. It is always
            // consumed by the parent's inline formatting context and rendered
            // via the parent's line_boxes above. Drawing it again from the
            // child's own paint_box would produce duplicate text.
        }

        // [CSS 2.1 Appendix E.2 Step 4](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "the in-flow, non-inline-level, non-positioned descendants"
        //
        // Paint children in two passes:
        //   1. Normal-flow and relatively positioned children (tree order)
        //   2. Absolutely/fixed positioned children (on top)
        //
        // [CSS 2.1 Appendix E.2 Step 8](https://www.w3.org/TR/CSS2/zindex.html#painting-order)
        // "All positioned descendants with 'z-index: auto' or 'z-index: 0',
        // in tree order."
        //
        // v1: We don't implement z-index, so absolute children always
        // paint on top of normal-flow children.
        for child in &layout_box.children {
            if !matches!(
                child.position_type,
                PositionType::Absolute | PositionType::Fixed
            ) {
                self.paint_box(child, display_list, effective_style);
            }
        }
        for child in &layout_box.children {
            if matches!(
                child.position_type,
                PositionType::Absolute | PositionType::Fixed
            ) {
                self.paint_box(child, display_list, effective_style);
            }
        }

        if needs_clip {
            display_list.push(DisplayCommand::PopClip);
        }
    }

    /// Paint borders for a box.
    ///
    /// [CSS Backgrounds and Borders § 4](https://www.w3.org/TR/css-backgrounds-3/#borders)
    ///
    /// Borders are drawn outside the padding box. For simplicity, we draw solid
    /// rectangles for each border side (ignoring border-style for now — all styles
    /// render as solid).
    #[allow(clippy::cast_possible_truncation, clippy::unused_self)]
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
            .map_or(0.0, |b| b.width.to_px() as f32);
        let right_width = style
            .border_right
            .as_ref()
            .map_or(0.0, |b| b.width.to_px() as f32);
        let bottom_width = style
            .border_bottom
            .as_ref()
            .map_or(0.0, |b| b.width.to_px() as f32);
        let left_width = style
            .border_left
            .as_ref()
            .map_or(0.0, |b| b.width.to_px() as f32);

        // Top border: spans full width including corners
        if let Some(border) = &style.border_top
            && top_width > 0.0
        {
            display_list.push(DisplayCommand::FillRect {
                x: padding_x - left_width,
                y: padding_y - top_width,
                width: padding_width + left_width + right_width,
                height: top_width,
                color: border.color.clone(),
                border_radius: BorderRadius::default(),
            });
        }

        // Bottom border: spans full width including corners
        if let Some(border) = &style.border_bottom
            && bottom_width > 0.0
        {
            display_list.push(DisplayCommand::FillRect {
                x: padding_x - left_width,
                y: padding_y + padding_height,
                width: padding_width + left_width + right_width,
                height: bottom_width,
                color: border.color.clone(),
                border_radius: BorderRadius::default(),
            });
        }

        // Left border: between top and bottom borders
        if let Some(border) = &style.border_left
            && left_width > 0.0
        {
            display_list.push(DisplayCommand::FillRect {
                x: padding_x - left_width,
                y: padding_y,
                width: left_width,
                height: padding_height,
                color: border.color.clone(),
                border_radius: BorderRadius::default(),
            });
        }

        // Right border: between top and bottom borders
        if let Some(border) = &style.border_right
            && right_width > 0.0
        {
            display_list.push(DisplayCommand::FillRect {
                x: padding_x + padding_width,
                y: padding_y,
                width: right_width,
                height: padding_height,
                color: border.color.clone(),
                border_radius: BorderRadius::default(),
            });
        }
    }
}
