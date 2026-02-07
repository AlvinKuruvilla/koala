//! CSS Positioned Layout.
//!
//! [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
//!
//! "In CSS 2, a box may be laid out according to three positioning schemes:
//!
//! 1. Normal flow. In CSS 2, normal flow includes block formatting of block-level
//!    boxes, inline formatting of inline-level boxes, and relative positioning of
//!    block-level and inline-level boxes.
//!
//! 2. Floats. In the float model, a box is first laid out according to the normal
//!    flow, then taken out of the flow and shifted to the left or right as far as
//!    possible.
//!
//! 3. Absolute positioning. In the absolute positioning model, a box is removed
//!    from the normal flow entirely and assigned a position with respect to a
//!    containing block."

use super::box_model::{BoxDimensions, Rect};
use super::inline::FontMetrics;
use super::layout_box::LayoutBox;
use super::values::UnresolvedAutoEdgeSizes;

/// [§ 9.3.1 Choosing a positioning scheme: 'position' property](https://www.w3.org/TR/CSS2/visuren.html#choose-position)
///
/// "The 'position' and 'float' properties determine which of the CSS 2
/// positioning algorithms is used to calculate the position of a box."
///
/// "Values have the following meanings:
///
/// static
///   The box is a normal box, laid out according to the normal flow. The
///   'top', 'right', 'bottom', and 'left' properties do not apply.
///
/// relative
///   The box's position is calculated according to the normal flow. Then
///   the box is offset relative to its normal position.
///
/// absolute
///   The box's position (and possibly size) is specified with the 'top',
///   'right', 'bottom', and 'left' properties. These properties specify
///   offsets with respect to the box's containing block.
///
/// fixed
///   The box's position is calculated according to the 'absolute' model,
///   but in addition, the box is fixed with respect to some reference.
///
/// sticky (CSS Positioned Layout Module Level 3)
///   The box's position is calculated according to the normal flow, then
///   offset relative to its nearest scrolling ancestor."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionType {
    /// "The box is a normal box, laid out according to the normal flow."
    #[default]
    Static,
    /// "The box's position is calculated according to the normal flow.
    /// Then the box is offset relative to its normal position."
    Relative,
    /// "The box's position (and possibly size) is specified with the
    /// 'top', 'right', 'bottom', and 'left' properties."
    Absolute,
    /// "The box's position is calculated according to the 'absolute' model,
    /// but the box is fixed with respect to some reference."
    Fixed,
    /// [CSS Positioned Layout Module Level 3 § 3.2](https://www.w3.org/TR/css-position-3/#sticky-position)
    ///
    /// "A stickily positioned box is positioned similarly to a relatively
    /// positioned box, but the offset is computed with reference to the
    /// nearest ancestor with a scrolling mechanism."
    Sticky,
}

/// [§ 9.3.2 Box offsets: 'top', 'right', 'bottom', 'left'](https://www.w3.org/TR/CSS2/visuren.html#position-props)
///
/// "An element is said to be positioned if its 'position' property has
/// a value other than 'static'. Positioned elements generate positioned
/// boxes, laid out according to four properties: top, right, bottom, left."
///
/// "Values have the following meanings:
///
/// `<length>`
///   The offset is a fixed distance from the reference edge.
///
/// `<percentage>`
///   The offset is a percentage of the containing block's width (for
///   'left' or 'right') or height (for 'top' or 'bottom').
///
/// auto
///   For non-replaced elements, the effect of this value depends on which
///   of related properties have the value 'auto' as well."
#[derive(Debug, Clone, Default)]
pub struct BoxOffsets {
    /// "The 'top' property specifies how far the top margin edge of the
    /// box is offset below the top edge of the box's containing block."
    pub top: Option<f32>,
    /// "The 'right' property specifies how far the right margin edge of
    /// the box is offset to the left of the right edge of the box's
    /// containing block."
    pub right: Option<f32>,
    /// "The 'bottom' property specifies how far the bottom margin edge of
    /// the box is offset above the bottom edge of the box's containing block."
    pub bottom: Option<f32>,
    /// "The 'left' property specifies how far the left margin edge of the
    /// box is offset to the right of the left edge of the box's containing block."
    pub left: Option<f32>,
}

/// Layout engine for positioned elements.
///
/// [§ 9.3 Positioning schemes](https://www.w3.org/TR/CSS2/visuren.html#positioning-scheme)
pub struct PositionedLayout;

impl PositionedLayout {
    /// [§ 9.4.3 Relative positioning](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
    ///
    /// "Once a box has been laid out according to the normal flow, it may be
    /// shifted relative to its normal position. This is called relative
    /// positioning."
    ///
    /// "Offsetting a box (B1) in this way has no effect on the box (B2) that
    /// follows: B2 is given a position as if B1 were not offset and B2 is
    /// not re-positioned after B1's offset is applied."
    ///
    /// "For relatively positioned elements, 'left' and 'right' move the box(es)
    /// horizontally, without changing their size."
    pub fn layout_relative(box_dims: &mut BoxDimensions, offsets: &BoxOffsets) {
        // STEP 1: Layout the box in normal flow (already done before this is called)

        // STEP 2: Apply horizontal offset
        // [§ 9.4.3](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
        //
        // "If both 'left' and 'right' are 'auto', the used values are both 0
        //  (i.e., the boxes stay in their original position)."
        //
        // "If 'left' is 'auto', its used value is minus the value of 'right'
        //  (i.e., the box is moved to the left by the value of 'right')."
        //
        // "If 'right' is 'auto', its used value is minus the value of 'left'
        //  (i.e., the box is moved to the right by the value of 'left')."
        //
        // "If neither 'left' nor 'right' is 'auto', the position is
        //  over-constrained, and one of them has to be ignored. If the
        //  'direction' property of the containing block is 'ltr', the value
        //  of 'left' wins and 'right' becomes -'left'. If 'direction' of the
        //  containing block is 'rtl', 'right' wins and 'left' is ignored."
        let offset_x = match (offsets.left, offsets.right) {
            // Both auto: no offset
            (None, None) => 0.0,
            // left specified, right auto: move right by left
            (Some(left), None) => left,
            // left auto, right specified: move left by right
            (None, Some(right)) => -right,
            // Both specified (over-constrained): left wins for LTR
            // NOTE: We assume LTR direction. When 'direction' property is
            // implemented, this should check the containing block's direction.
            (Some(left), Some(_right)) => left,
        };
        box_dims.content.x += offset_x;

        // STEP 3: Apply vertical offset
        // [§ 9.4.3](https://www.w3.org/TR/CSS2/visuren.html#relative-positioning)
        //
        // "If both are 'auto', their used values are both 0."
        //
        // "If one of them is 'auto', it becomes the negative of the other."
        //
        // "If neither is 'auto', 'bottom' is ignored (i.e., the used value
        //  of 'bottom' will be minus the value of 'top')."
        let offset_y = match (offsets.top, offsets.bottom) {
            // Both auto: no offset
            (None, None) => 0.0,
            // top specified, bottom auto: move down by top
            (Some(top), None) => top,
            // top auto, bottom specified: move up by bottom
            (None, Some(bottom)) => -bottom,
            // Both specified (over-constrained): top wins
            (Some(top), Some(_bottom)) => top,
        };
        box_dims.content.y += offset_y;
    }

    /// [§ 10.3.7 Absolutely positioned, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
    ///
    /// "The constraint that determines the used values for these elements is:
    ///
    /// 'left' + 'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
    /// 'padding-right' + 'border-right-width' + 'margin-right' + 'right'
    /// = width of containing block"
    ///
    /// [§ 10.6.4 Absolutely positioned, non-replaced elements](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-height)
    ///
    /// "For absolutely positioned elements, the used values of the vertical
    /// dimensions must satisfy this constraint:
    ///
    /// 'top' + 'margin-top' + 'border-top-width' + 'padding-top' + 'height' +
    /// 'padding-bottom' + 'border-bottom-width' + 'margin-bottom' + 'bottom'
    /// = height of containing block"
    #[allow(clippy::similar_names)]
    pub fn layout_absolute(
        layout_box: &mut LayoutBox,
        containing_block: Rect,
        viewport: Rect,
        font_metrics: &dyn FontMetrics,
    ) {
        // STEP 1: Resolve padding, border, and margin to used values.
        let resolved_padding = layout_box.padding.resolve(viewport);
        let resolved_border = layout_box.border_width.resolve(viewport);
        let resolved_margin = layout_box.margin.resolve(viewport);

        layout_box.dimensions.padding.left = resolved_padding.left;
        layout_box.dimensions.padding.right = resolved_padding.right;
        layout_box.dimensions.padding.top = resolved_padding.top;
        layout_box.dimensions.padding.bottom = resolved_padding.bottom;

        layout_box.dimensions.border.left = resolved_border.left;
        layout_box.dimensions.border.right = resolved_border.right;
        layout_box.dimensions.border.top = resolved_border.top;
        layout_box.dimensions.border.bottom = resolved_border.bottom;

        let pl = resolved_padding.left;
        let pr = resolved_padding.right;
        let bl = resolved_border.left;
        let br = resolved_border.right;
        let pt = resolved_padding.top;
        let pb = resolved_padding.bottom;
        let bt = resolved_border.top;
        let bb = resolved_border.bottom;

        // STEP 2: Resolve the horizontal constraint equation.
        // [§ 10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
        //
        // "The constraint that determines the used values for these elements is:
        // 'left' + 'margin-left' + 'border-left-width' + 'padding-left' + 'width' +
        // 'padding-right' + 'border-right-width' + 'margin-right' + 'right'
        // = width of containing block"
        let cb_width = containing_block.width;

        let left_auto = layout_box.offsets.left.is_none();
        let right_auto = layout_box.offsets.right.is_none();
        let width_auto = layout_box.width.as_ref().is_none_or(|al| {
            UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).is_auto()
        });
        let ml_auto = resolved_margin.left.is_auto();
        let mr_auto = resolved_margin.right.is_auto();

        let left_val = layout_box.offsets.left.unwrap_or(0.0);
        let right_val = layout_box.offsets.right.unwrap_or(0.0);
        let width_val = layout_box.width.as_ref().map_or(0.0, |al| {
            UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).to_px_or(0.0)
        });
        let ml_val = resolved_margin.left.to_px_or(0.0);
        let mr_val = resolved_margin.right.to_px_or(0.0);

        let (used_left, used_width, used_ml, used_mr, _used_right) =
            Self::solve_horizontal_constraint(
                cb_width,
                left_auto,
                left_val,
                right_auto,
                right_val,
                width_auto,
                width_val,
                ml_auto,
                ml_val,
                mr_auto,
                mr_val,
                bl,
                pl,
                pr,
                br,
            );

        layout_box.dimensions.content.width = used_width;
        layout_box.dimensions.margin.left = used_ml;
        layout_box.dimensions.margin.right = used_mr;

        // STEP 3: Resolve the vertical constraint equation.
        // [§ 10.6.4](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-height)
        //
        // Same pattern as horizontal but with top/height/bottom.
        let cb_height = containing_block.height;

        let top_auto = layout_box.offsets.top.is_none();
        let bottom_auto = layout_box.offsets.bottom.is_none();
        let height_auto = layout_box.height.as_ref().is_none_or(|al| {
            UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).is_auto()
        });
        let mt_auto = resolved_margin.top.is_auto();
        let mb_auto = resolved_margin.bottom.is_auto();

        let top_val = layout_box.offsets.top.unwrap_or(0.0);
        let bottom_val = layout_box.offsets.bottom.unwrap_or(0.0);
        let height_val = layout_box.height.as_ref().map_or(0.0, |al| {
            UnresolvedAutoEdgeSizes::resolve_auto_length(al, viewport).to_px_or(0.0)
        });
        let mt_val = resolved_margin.top.to_px_or(0.0);
        let mb_val = resolved_margin.bottom.to_px_or(0.0);

        // Position the content box horizontally first so children can lay out.
        // [§ 10.3.7](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
        layout_box.dimensions.content.x =
            containing_block.x + used_left + used_ml + bl + pl;

        // Temporarily set the y position (will be refined after height is resolved).
        // We need a valid y for child layout.
        let used_top_for_children = if top_auto { 0.0 } else { top_val };
        let used_mt_for_children = if mt_auto { 0.0 } else { mt_val };
        layout_box.dimensions.content.y =
            containing_block.y + used_top_for_children + used_mt_for_children + bt + pt;

        // STEP 4: Lay out children to determine auto height.
        //
        // If height is auto, we need to know content height from children.
        // Use the same approach as layout_block: generate anonymous boxes,
        // lay out children, then compute height.
        if height_auto {
            layout_box.generate_anonymous_boxes();

            if layout_box.all_children_inline() && !layout_box.children.is_empty() {
                layout_box.layout_inline_children(viewport, font_metrics);
            } else {
                layout_box.layout_block_children(viewport, font_metrics);
            }

            // Auto height: use the content height computed by child layout.
            layout_box.calculate_block_height(viewport, font_metrics);

            // Now resolve vertical constraint with the known content height.
            let content_height = layout_box.dimensions.content.height;

            let (used_top, _used_height, used_mt, used_mb) =
                Self::solve_vertical_constraint(
                    cb_height,
                    top_auto,
                    top_val,
                    bottom_auto,
                    bottom_val,
                    false, // height is no longer auto — we measured it
                    content_height,
                    mt_auto,
                    mt_val,
                    mb_auto,
                    mb_val,
                    bt,
                    pt,
                    pb,
                    bb,
                );

            layout_box.dimensions.margin.top = used_mt;
            layout_box.dimensions.margin.bottom = used_mb;
            layout_box.dimensions.content.y =
                containing_block.y + used_top + used_mt + bt + pt;

            // Re-position children to match the final y.
            // The difference between the temporary y and the final y.
            let y_delta = layout_box.dimensions.content.y
                - (containing_block.y + used_top_for_children + used_mt_for_children + bt + pt);
            if y_delta.abs() > 0.001 {
                Self::shift_children_y(&mut layout_box.children, y_delta);
            }
        } else {
            // Height is explicit — resolve vertical constraint fully.
            let (used_top, used_height, used_mt, used_mb) =
                Self::solve_vertical_constraint(
                    cb_height,
                    top_auto,
                    top_val,
                    bottom_auto,
                    bottom_val,
                    height_auto,
                    height_val,
                    mt_auto,
                    mt_val,
                    mb_auto,
                    mb_val,
                    bt,
                    pt,
                    pb,
                    bb,
                );

            layout_box.dimensions.content.height = used_height;
            layout_box.dimensions.margin.top = used_mt;
            layout_box.dimensions.margin.bottom = used_mb;
            layout_box.dimensions.content.y =
                containing_block.y + used_top + used_mt + bt + pt;

            // Lay out children within the positioned box.
            layout_box.generate_anonymous_boxes();
            if layout_box.all_children_inline() && !layout_box.children.is_empty() {
                layout_box.layout_inline_children(viewport, font_metrics);
            } else {
                layout_box.layout_block_children(viewport, font_metrics);
            }

            // Restore the explicit height — child layout may have
            // overwritten it (e.g., layout_inline_children sets content
            // height from line boxes).
            layout_box.dimensions.content.height = used_height;
        }

        // STEP 5: Lay out any absolutely positioned children of this box.
        layout_box.layout_absolute_children(viewport, font_metrics);
    }

    /// Solve the horizontal constraint equation per § 10.3.7.
    ///
    /// Returns (`used_left`, `used_width`, `used_margin_left`, `used_margin_right`, `used_right`).
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools, clippy::similar_names)]
    fn solve_horizontal_constraint(
        cb_width: f32,
        left_auto: bool,
        left_val: f32,
        right_auto: bool,
        right_val: f32,
        width_auto: bool,
        width_val: f32,
        ml_auto: bool,
        ml_val: f32,
        mr_auto: bool,
        mr_val: f32,
        bl: f32,
        pl: f32,
        pr: f32,
        br: f32,
    ) -> (f32, f32, f32, f32, f32) {
        let auto_count = [left_auto, width_auto, right_auto]
            .iter()
            .filter(|&&a| a)
            .count();

        // [§ 10.3.7 Case 1](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
        //
        // "If all three of 'left', 'width', and 'right' are 'auto': First set
        // any 'auto' values for 'margin-left' and 'margin-right' to 0. Then,
        // if the 'direction' property of the element establishing the static-
        // position containing block is 'ltr' set 'left' to the static position..."
        //
        // v1 simplification: static position = 0
        if auto_count == 3 {
            let ml = if ml_auto { 0.0 } else { ml_val };
            let mr = if mr_auto { 0.0 } else { mr_val };
            // left = 0 (static position fallback)
            let left = 0.0;
            // width = shrink-to-fit; for v1 use 0 (content will expand it)
            let width = 0.0;
            let right = cb_width - left - ml - bl - pl - width - pr - br - mr;
            return (left, width, ml, mr, right);
        }

        // [§ 10.3.7 Case 2](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
        //
        // "If none of the three is 'auto': If both 'margin-left' and
        // 'margin-right' are 'auto', solve the equation under the extra
        // constraint that the two margins get equal values..."
        if auto_count == 0 {
            let remaining = cb_width - left_val - bl - pl - width_val - pr - br - right_val;
            if ml_auto && mr_auto {
                let half = remaining / 2.0;
                return (left_val, width_val, half, half, right_val);
            } else if ml_auto {
                return (left_val, width_val, remaining - mr_val, mr_val, right_val);
            } else if mr_auto {
                return (left_val, width_val, ml_val, remaining - ml_val, right_val);
            }
            // Over-constrained: ignore 'right' (LTR).
            // [§ 10.3.7]: "If the 'direction' property... is 'ltr', ignore
            // the value for 'right' and solve for that value."
            let ml = if ml_auto { 0.0 } else { ml_val };
            let mr = if mr_auto { 0.0 } else { mr_val };
            let right = cb_width - left_val - ml - bl - pl - width_val - pr - br - mr;
            return (left_val, width_val, ml, mr, right);
        }

        // [§ 10.3.7 Case 3](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-width)
        //
        // "Otherwise, set 'auto' values for 'margin-left' and 'margin-right'
        // to 0, and pick the one of the following six rules that is the first
        // to match..."
        let ml = if ml_auto { 0.0 } else { ml_val };
        let mr = if mr_auto { 0.0 } else { mr_val };

        match (left_auto, width_auto, right_auto) {
            // Exactly one auto value among left/width/right:
            (true, false, false) => {
                // Solve for left
                let left = cb_width - ml - bl - pl - width_val - pr - br - mr - right_val;
                (left, width_val, ml, mr, right_val)
            }
            (false, true, false) => {
                // Solve for width
                let width = cb_width - left_val - ml - bl - pl - pr - br - mr - right_val;
                (left_val, width.max(0.0), ml, mr, right_val)
            }
            (false, false, true) => {
                // Solve for right
                let right = cb_width - left_val - ml - bl - pl - width_val - pr - br - mr;
                (left_val, width_val, ml, mr, right)
            }
            // Two auto values:
            (true, true, false) => {
                // left and width auto, right specified
                // [§ 10.3.7]: "left and width auto: set left to static
                // position, solve for width"
                let left = 0.0; // static position fallback
                let width = cb_width - left - ml - bl - pl - pr - br - mr - right_val;
                (left, width.max(0.0), ml, mr, right_val)
            }
            (true, false, true) => {
                // left and right auto, width specified
                // [§ 10.3.7]: "left and right auto: if direction is ltr,
                // set left to static position, solve for right"
                let left = 0.0; // static position fallback
                let right = cb_width - left - ml - bl - pl - width_val - pr - br - mr;
                (left, width_val, ml, mr, right)
            }
            (false, true, true) => {
                // width and right auto
                // [§ 10.3.7]: "width and right auto: shrink-to-fit width,
                // solve for right"
                // v1: use 0 for shrink-to-fit (content expands)
                let width = 0.0;
                let right = cb_width - left_val - ml - bl - pl - width - pr - br - mr;
                (left_val, width, ml, mr, right)
            }
            // All three auto handled above, none auto handled above
            _ => unreachable!(),
        }
    }

    /// Solve the vertical constraint equation per § 10.6.4.
    ///
    /// Returns (`used_top`, `used_height`, `used_margin_top`, `used_margin_bottom`).
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools, clippy::similar_names)]
    fn solve_vertical_constraint(
        cb_height: f32,
        top_auto: bool,
        top_val: f32,
        bottom_auto: bool,
        bottom_val: f32,
        height_auto: bool,
        height_val: f32,
        mt_auto: bool,
        mt_val: f32,
        mb_auto: bool,
        mb_val: f32,
        bt: f32,
        pt: f32,
        pb: f32,
        bb: f32,
    ) -> (f32, f32, f32, f32) {
        let auto_count = [top_auto, height_auto, bottom_auto]
            .iter()
            .filter(|&&a| a)
            .count();

        // [§ 10.6.4](https://www.w3.org/TR/CSS2/visudet.html#abs-non-replaced-height)
        //
        // "If all three of 'top', 'height', and 'bottom' are auto, set
        // 'top' to the static position and apply rule number three below."
        if auto_count == 3 {
            let mt = if mt_auto { 0.0 } else { mt_val };
            let mb = if mb_auto { 0.0 } else { mb_val };
            let top = 0.0; // static position fallback
            // height auto: use content height (0 if no content)
            let height = height_val; // caller should have measured
            // bottom = cb_height - top - mt - bt - pt - height - pb - bb - mb
            // (not needed for positioning, only top/height/margins matter)
            return (top, height, mt, mb);
        }

        if auto_count == 0 {
            let remaining = cb_height - top_val - bt - pt - height_val - pb - bb - bottom_val;
            if mt_auto && mb_auto {
                let half = remaining / 2.0;
                return (top_val, height_val, half, half);
            } else if mt_auto {
                return (top_val, height_val, remaining - mb_val, mb_val);
            } else if mb_auto {
                return (top_val, height_val, mt_val, remaining - mt_val);
            }
            // Over-constrained: ignore 'bottom'.
            let mt = if mt_auto { 0.0 } else { mt_val };
            let mb = if mb_auto { 0.0 } else { mb_val };
            return (top_val, height_val, mt, mb);
        }

        // Set auto margins to 0, solve for the remaining auto value.
        let mt = if mt_auto { 0.0 } else { mt_val };
        let mb = if mb_auto { 0.0 } else { mb_val };

        match (top_auto, height_auto, bottom_auto) {
            (true, false, false) => {
                // Solve for top
                let top = cb_height - mt - bt - pt - height_val - pb - bb - mb - bottom_val;
                (top, height_val, mt, mb)
            }
            (false, true, false) => {
                // Solve for height
                let height = cb_height - top_val - mt - bt - pt - pb - bb - mb - bottom_val;
                (top_val, height.max(0.0), mt, mb)
            }
            (false, false, true) => {
                // Solve for bottom (doesn't affect position, just return used values)
                (top_val, height_val, mt, mb)
            }
            (true, true, false) => {
                // top and height auto, bottom specified
                // [§ 10.6.4]: "top and height auto: set top to static
                // position, use content height"
                let top = 0.0; // static position fallback
                let height = height_val; // caller measured
                (top, height, mt, mb)
            }
            (true, false, true) => {
                // top and bottom auto, height specified
                // Set top to static position
                let top = 0.0; // static position fallback
                (top, height_val, mt, mb)
            }
            (false, true, true) => {
                // height and bottom auto
                // Use content height
                (top_val, height_val, mt, mb)
            }
            _ => unreachable!(),
        }
    }

    /// Recursively shift all children's y positions by a delta.
    ///
    /// Used when the final y position of an absolutely positioned box
    /// differs from the temporary y used during child layout.
    fn shift_children_y(children: &mut [LayoutBox], delta: f32) {
        for child in children.iter_mut() {
            child.dimensions.content.y += delta;
            Self::shift_children_y(&mut child.children, delta);
            // Also shift line box fragments.
            for line_box in &mut child.line_boxes {
                for fragment in &mut line_box.fragments {
                    fragment.bounds.y += delta;
                }
            }
        }
    }

    /// [§ 9.3.1 Fixed positioning](https://www.w3.org/TR/CSS2/visuren.html#fixed-positioning)
    ///
    /// "Fixed positioning is a subcategory of absolute positioning. The only
    /// difference is that for a fixed positioned box, the containing block is
    /// established by the viewport."
    ///
    /// "Fixed backgrounds on the root element are fixed with respect to the
    /// canvas and do not move during scrolling."
    ///
    /// TODO: Implement fixed positioning:
    ///
    /// STEP 1: Use the viewport as the containing block
    ///   `// let containing_block = viewport;`
    ///
    /// STEP 2: Apply the same algorithm as absolute positioning
    ///   `// Self::layout_absolute(box_dims, offsets, viewport);`
    ///
    /// STEP 3: Mark the box as fixed (for scroll behavior)
    ///   // During painting, fixed boxes are positioned relative to the viewport
    ///   // regardless of scroll position.
    pub fn layout_fixed(_box_dims: &mut BoxDimensions, _offsets: &BoxOffsets, _viewport: Rect) {
        todo!("Fixed positioning: position box relative to viewport")
    }

    /// [CSS Positioned Layout Module Level 3 § 3.2 Sticky positioning](https://www.w3.org/TR/css-position-3/#sticky-position)
    ///
    /// "A stickily positioned box is positioned similarly to a relatively
    /// positioned box, but the offset is computed with reference to the
    /// nearest ancestor with a scrolling mechanism (created when overflow
    /// is hidden, scroll, auto, or overlay)."
    ///
    /// "The stickily positioned box sticks to the relevant edge of the scroll
    /// container, provided the box would otherwise be visible within the
    /// scrollport."
    ///
    /// TODO: Implement sticky positioning:
    ///
    /// STEP 1: Layout the box in normal flow (already done)
    ///
    /// STEP 2: Find the nearest scroll ancestor
    ///   // Walk up the tree to find an ancestor with overflow != visible
    ///
    /// STEP 3: Compute the sticky constraint rectangle
    ///   // The box sticks within the intersection of:
    ///   //   - Its containing block (minus margins)
    ///   //   - The scroll container's visible area (minus sticky offsets)
    ///
    /// STEP 4: Apply sticky offset based on scroll position
    ///   // If scrolled past the sticky threshold, offset the box so it
    ///   // appears to stick to the edge of the scroll container.
    ///   // The box should never leave its containing block.
    pub fn layout_sticky(
        _box_dims: &mut BoxDimensions,
        _offsets: &BoxOffsets,
        _scroll_container: Rect,
        _scroll_offset: f32,
    ) {
        todo!("Sticky positioning: offset box within scroll container")
    }
}
