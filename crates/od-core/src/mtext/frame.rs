//! The reference frame: where the text's box lies in the drawing, how big the
//! layout makes it, and the grips on it.
//!
//! Entities do not know their style, so the context-free queries here
//! (bounds, snapping, grips) lay out in the stock style. A style with another
//! width factor or slant is painted from its own layout but picked and
//! snapped by the stock one.

use std::rc::Rc;

use super::MText;
use crate::grip::{Grip, GripKind};
use crate::text::{memoized_layout, TextLayout};
use crate::text_style::TextStyle;
use crate::{Bounds, Point};

impl MText {
    /// The direction as a unit vector.
    pub fn unit_direction(&self) -> (f64, f64) {
        let length = self.direction.x.hypot(self.direction.y);
        if length > 0.0 && length.is_finite() {
            (self.direction.x / length, self.direction.y / length)
        } else {
            (1.0, 0.0)
        }
    }

    /// The text laid out in `style` (remembered, so asking again is cheap).
    pub fn layout(&self, style: &TextStyle) -> Rc<TextLayout> {
        memoized_layout(self, style)
    }

    /// The width and height of the frame, in the stock style.
    pub fn extent(&self) -> (f64, f64) {
        let layout = self.layout(&TextStyle::standard());
        (layout.frame_width, layout.frame_height)
    }

    /// A point `across` along the text direction and `up` along its
    /// perpendicular, measured from the insertion point.
    fn at(&self, across: f64, up: f64) -> Point {
        let (ux, uy) = self.unit_direction();
        Point::new(
            self.insertion.x + across * ux - up * uy,
            self.insertion.y + across * uy + up * ux,
        )
    }

    /// Offsets of the box's left edge and top edge from the insertion point.
    fn offsets(&self, width: f64, height: f64) -> (f64, f64) {
        let (across, down) = self.attachment.fractions();
        (-across * width, down * height)
    }

    /// The drawing point for layout coordinates: `x` along the text and `y`
    /// up from the top of the block (so lines sit at negative `y`). The
    /// attachment is applied using `layout`'s own frame.
    pub fn world_point(&self, layout: &TextLayout, x: f64, y: f64) -> Point {
        let (left, top) = self.offsets(layout.frame_width, layout.frame_height);
        self.at(left + x, top + y)
    }

    /// The layout coordinates of a drawing point: the inverse of
    /// [`MText::world_point`].
    pub fn layout_point(&self, layout: &TextLayout, world: Point) -> (f64, f64) {
        let (ux, uy) = self.unit_direction();
        let (left, top) = self.offsets(layout.frame_width, layout.frame_height);
        let (dx, dy) = (world.x - self.insertion.x, world.y - self.insertion.y);
        (dx * ux + dy * uy - left, -dx * uy + dy * ux - top)
    }

    /// The frame's corners: top left, top right, bottom right, bottom left.
    pub fn frame(&self) -> [Point; 4] {
        let (width, height) = self.extent();
        let (left, top) = self.offsets(width, height);
        let (right, bottom) = (left + width, top - height);
        [
            self.at(left, top),
            self.at(right, top),
            self.at(right, bottom),
            self.at(left, bottom),
        ]
    }

    /// Whether `p` lies inside the frame (its edges count).
    pub fn contains(&self, p: Point) -> bool {
        let (ux, uy) = self.unit_direction();
        let (width, height) = self.extent();
        let (left, top) = self.offsets(width, height);
        let (dx, dy) = (p.x - self.insertion.x, p.y - self.insertion.y);
        let (across, up) = (dx * ux + dy * uy, -dx * uy + dy * ux);
        (left..=left + width).contains(&across) && (top - height..=top).contains(&up)
    }

    pub fn bounds(&self) -> Bounds {
        let [a, b, c, d] = self.frame();
        Bounds::from_points(a, c).union(Bounds::from_points(b, d))
    }

    /// Where an object snap can land: the insertion point, the four corners
    /// and the four edge middles.
    pub fn snap_points(&self) -> Vec<Point> {
        let corners = self.frame();
        let mut points = vec![self.insertion];
        points.extend(corners);
        points.extend((0..4).map(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
        }));
        points
    }

    /// The insertion point, which moves the text, then, when the text wraps,
    /// the middle of the right edge, which sets the width.
    pub fn grips(&self) -> Vec<Grip> {
        let mut grips = vec![Grip {
            at: self.insertion,
            kind: GripKind::Move,
        }];
        if self.reference_width > 0.0 {
            let (width, height) = self.extent();
            let (left, top) = self.offsets(width, height);
            grips.push(Grip {
                at: self.at(left + width, top - height / 2.0),
                kind: GripKind::Stretch,
            });
        }
        grips
    }

    /// The text with grip `index` dragged to `to`. The width grip moves the
    /// right edge and keeps the left one where it is.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<MText> {
        let grips = self.grips();
        let from = grips.get(index)?.at;
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        if !(dx.is_finite() && dy.is_finite()) {
            return None;
        }
        if index == 0 {
            return Some(self.translated(dx, dy));
        }
        let (ux, uy) = self.unit_direction();
        let (old_width, height) = self.extent();
        let width = old_width + dx * ux + dy * uy;
        if !(width.is_finite() && width > 1e-9) {
            return None;
        }
        let (old_left, _) = self.offsets(old_width, height);
        let (new_left, _) = self.offsets(width, height);
        let shift = old_left - new_left;
        let mut moved = self.clone();
        moved.reference_width = width;
        moved.insertion = Point::new(self.insertion.x + shift * ux, self.insertion.y + shift * uy);
        Some(moved)
    }
}
