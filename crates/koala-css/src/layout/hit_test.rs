//! Finding the box under a point.
//!
//! [CSSOM View § 5 elementFromPoint()](https://drafts.csswg.org/cssom-view/#dom-document-elementfrompoint)
//!
//! "If there is a box in the viewport that would be a target for hit
//! testing at coordinates x,y, when applying the transforms that apply to
//! the descendants of the viewport, return the associated element"
//!
//! "Note: The specifics of hit testing are out of scope of this
//! specification and therefore the exact details of `elementFromPoint()`
//! and `caretPositionFromPoint()` are therefore too."
//!
//! What is specified is the order: "For each box in the viewport, in paint
//! order, starting with the topmost box" (`elementsFromPoint()`). So the
//! walk here is `DisplayListBuilder::paint_box` run backwards, and the two
//! must change together:
//!
//! ```text
//!   paint_box (back to front)          hit_test (front to back)
//!   1. own background and border       4. positioned children, last first
//!   2. line boxes' text                3. in-flow children, last first
//!   3. in-flow children                2. inline-box fragments, last first
//!   4. positioned children             1. own border box
//! ```
//!
//! Text is not a target of its own. A point on a text fragment goes to the
//! innermost inline box whose fragment contains it, or failing that to the
//! block that owns the line.
//!
//! # Trade-offs
//!
//! - **No `pointer-events`**: the property is not parsed, so every box
//!   behaves as `auto`.
//! - **Rectangular overflow clip**: CSS Backgrounds 3 § 4.3 clips "overflow
//!   values other than visible to the curved padding edge". Paint clips to
//!   the plain rectangle, and the hit test matches paint, not the spec.

use super::box_model::Rect;
use super::inline::FragmentContent;
use super::layout_box::{BoxType, ElementBoxId, LayoutBox};
use crate::style::computed::{Overflow, Visibility};
use crate::style::values::{BorderRadius, PositionType};

/// What [`LayoutBox::hit_test`] found under a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitTestResult {
    /// The topmost box that is a target for hit testing. A `::before` or
    /// `::after` box is returned as itself; its element is
    /// `ElementBoxId::node`.
    pub target: ElementBoxId,
    /// Whether the point is over a run of text in `target`.
    ///
    /// [CSS UI 4 § 5.1.1.1](https://drafts.csswg.org/css-ui-4/#cursor)
    ///
    /// "auto behaves as text over selectable text or editable elements,
    /// and default otherwise."
    pub over_text: bool,
}

impl LayoutBox {
    /// The topmost box at (`x`, `y`) that is a target for hit testing, or
    /// `None` if there is none. Coordinates are in the same space as the
    /// layout tree's dimensions.
    ///
    /// # Time complexity
    ///
    /// *O*(*n*) in the number of boxes and line fragments, plus a search of
    /// the owning block's descendants for each inline-box fragment hit.
    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTestResult> {
        // [CSS Overflow 3 § 3](https://drafts.csswg.org/css-overflow-3/#overflow-control)
        //
        // "the box's content is clipped to its overflow clip edge"
        //
        // Paint clips to the padding box (`DisplayCommand::PushClip`), so
        // content outside it is not under the pointer. The box's own border
        // box, painted before the clip, still is.
        let content_reachable =
            self.overflow == Overflow::Visible || contains(self.dimensions.padding_box(), x, y);

        let mut over_text = false;
        if content_reachable {
            // STEP 1: Positioned children, painted last, so tested first.
            for child in self.children.iter().rev() {
                if is_out_of_flow_positioned(child)
                    && let Some(hit) = child.hit_test(x, y)
                {
                    return Some(hit);
                }
            }

            // STEP 2: In-flow and floated children. Inline-blocks and
            // replaced inline elements are among them, and are painted
            // after the line's text, so they come before the fragments.
            for child in self.children.iter().rev() {
                if !is_out_of_flow_positioned(child)
                    && let Some(hit) = child.hit_test(x, y)
                {
                    return Some(hit);
                }
            }

            // STEP 3: The inline boxes on this box's lines. A nested box's
            // fragment follows its parent's, so the first match walking
            // backwards is the innermost box. A text fragment comes after
            // the inline boxes that contain it, so whether the point is on
            // text is known by the time its box is found.
            for line in self.line_boxes.iter().rev() {
                for fragment in line.fragments.iter().rev() {
                    if !contains(fragment.bounds, x, y) {
                        continue;
                    }
                    match &fragment.content {
                        FragmentContent::Text(run) if !run.text.is_empty() => over_text = true,
                        &FragmentContent::InlineBox { id, .. }
                            if find_box(&self.children, id).is_some_and(is_hit_target) =>
                        {
                            return Some(HitTestResult {
                                target: id,
                                over_text,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        // STEP 4: This box's own border box.
        //
        // A non-replaced inline box has no geometry of its own; its
        // fragments, tested in STEP 3 by the block that owns the lines,
        // stand in for it.
        if is_hit_target(self)
            && !self.is_inline_box()
            && contains_curved(self.dimensions.border_box(), &self.border_radius, x, y)
        {
            return self.box_type.element_box_id().map(|target| HitTestResult {
                target,
                over_text,
            });
        }

        None
    }

    /// [§ 9.2.2](https://www.w3.org/TR/CSS2/visuren.html#inline-boxes)
    ///
    /// "An inline box is one that is both inline-level and whose contents
    /// participate in its containing inline formatting context."
    fn is_inline_box(&self) -> bool {
        matches!(self.box_type, BoxType::Principal(_) | BoxType::PseudoElement(..))
            && self.display == crate::style::DisplayValue::inline()
            && !self.is_replaced
    }
}

/// [CSS Display 4 § 5](https://drafts.csswg.org/css-display-4/#visibility)
///
/// "Invisible boxes are not rendered (as if they were fully transparent),
/// cannot be interacted with (and behave as if they had pointer-events:
/// none)"
///
/// [CSS UI 4 § 6.2](https://drafts.csswg.org/css-ui-4/#pointer-events-control)
///
/// "Note: This pointer-events: none only excludes the element itself from
/// hit-testing, not the whole subtree."
///
/// So an invisible box is skipped but its descendants are still tested.
/// `opacity: 0` does not make a box invisible in this sense, though paint
/// skips it.
fn is_hit_target(layout_box: &LayoutBox) -> bool {
    layout_box.visibility == Visibility::Visible
}

fn is_out_of_flow_positioned(layout_box: &LayoutBox) -> bool {
    matches!(
        layout_box.position_type,
        PositionType::Absolute | PositionType::Fixed
    )
}

fn find_box(children: &[LayoutBox], target: ElementBoxId) -> Option<&LayoutBox> {
    children.iter().find_map(|child| {
        if child.box_type.element_box_id() == Some(target) {
            Some(child)
        } else {
            find_box(&child.children, target)
        }
    })
}

/// Half-open, so a zero-sized rectangle contains nothing.
fn contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

/// [CSS Backgrounds 3 § 4.3 Corner Clipping](https://www.w3.org/TR/css-backgrounds-3/#corner-clipping)
///
/// "other effects that clip painting or event handling to the border,
/// padding, or content edge must clip to their respective curves. For
/// example, ... pointer events to the curved border edge"
///
/// Each corner is a quarter circle, as the renderer draws it. The § 4.5
/// scaling of radii that overlap is not applied, matching the renderer.
fn contains_curved(rect: Rect, radius: &BorderRadius, x: f32, y: f32) -> bool {
    if !contains(rect, x, y) {
        return false;
    }

    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;

    // A point in a corner's r × r square is inside only if it is within r
    // of the circle's centre, the square's inner corner. Anywhere else in
    // the rectangle is inside.
    let outside_arc = |r: f32, cx: f32, cy: f32| {
        let (dx, dy) = (x - cx, y - cy);
        dx.mul_add(dx, dy * dy) > r * r
    };
    let r = radius.top_left;
    if x < left + r && y < top + r {
        return !outside_arc(r, left + r, top + r);
    }
    let r = radius.top_right;
    if x > right - r && y < top + r {
        return !outside_arc(r, right - r, top + r);
    }
    let r = radius.bottom_right;
    if x > right - r && y > bottom - r {
        return !outside_arc(r, right - r, bottom - r);
    }
    let r = radius.bottom_left;
    if x < left + r && y > bottom - r {
        return !outside_arc(r, left + r, bottom - r);
    }
    true
}
