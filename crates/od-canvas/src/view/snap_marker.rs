//! The mark drawn where the cursor snaps, one shape per kind as AutoCAD
//! draws them, and its tooltip.

use egui::{vec2, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use od_core::Point;

use super::{CanvasStyle, View};
use crate::snap::{SnapKind, SnapModes, SnapPoint};

/// Half the size of a marker, in screen points.
const R: f32 = 5.0;

/// Draw `kind`'s shape centred on `c`.
fn glyph(painter: &Painter, c: Pos2, kind: SnapKind, stroke: Stroke) {
    let v = |x: f32, y: f32| c + vec2(x * R, y * R);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
    match kind {
        SnapKind::Endpoint => {
            painter.rect_stroke(
                Rect::from_center_size(c, vec2(2.0 * R, 2.0 * R)),
                0.0,
                stroke,
                StrokeKind::Middle,
            );
        }
        SnapKind::Midpoint => {
            let corners = vec![v(0.0, -1.2), v(1.2, 1.0), v(-1.2, 1.0)];
            painter.add(Shape::closed_line(corners, stroke));
        }
        SnapKind::Center => {
            painter.circle_stroke(c, R, stroke);
        }
        // A circle with an X through it.
        SnapKind::Node => {
            painter.circle_stroke(c, R, stroke);
            line(v(-0.7, -0.7), v(0.7, 0.7));
            line(v(-0.7, 0.7), v(0.7, -0.7));
        }
        SnapKind::Quadrant => {
            let corners = vec![v(0.0, -1.2), v(1.2, 0.0), v(0.0, 1.2), v(-1.2, 0.0)];
            painter.add(Shape::closed_line(corners, stroke));
        }
        SnapKind::Intersection => {
            line(v(-1.0, -1.0), v(1.0, 1.0));
            line(v(-1.0, 1.0), v(1.0, -1.0));
        }
        // The X in a square.
        SnapKind::ApparentIntersection => {
            painter.rect_stroke(
                Rect::from_center_size(c, vec2(2.4 * R, 2.4 * R)),
                0.0,
                stroke,
                StrokeKind::Middle,
            );
            line(v(-0.8, -0.8), v(0.8, 0.8));
            line(v(-0.8, 0.8), v(0.8, -0.8));
        }
        // The X with the dots of the path it was found along.
        SnapKind::ExtendedIntersection => {
            line(v(-0.8, -0.8), v(0.8, 0.8));
            line(v(-0.8, 0.8), v(0.8, -0.8));
            for x in [1.3, 1.8] {
                painter.circle_filled(v(x, 0.0), 0.7, stroke.color);
            }
        }
        // A circle with a plus through it.
        SnapKind::GeometricCenter => {
            painter.circle_stroke(c, R, stroke);
            line(v(-0.7, 0.0), v(0.7, 0.0));
            line(v(0.0, -0.7), v(0.0, 0.7));
        }
        // Two squares, one offset from the other.
        SnapKind::Insertion => {
            let square = |at: Pos2| Rect::from_center_size(at, vec2(1.4 * R, 1.4 * R));
            painter.rect_stroke(square(v(-0.35, 0.35)), 0.0, stroke, StrokeKind::Middle);
            painter.rect_stroke(square(v(0.35, -0.35)), 0.0, stroke, StrokeKind::Middle);
        }
        // A right angle with the small square of a drawn right angle.
        SnapKind::Perpendicular => {
            line(v(-1.0, 1.0), v(1.0, 1.0));
            line(v(-1.0, 1.0), v(-1.0, -1.0));
            let corner = vec![v(-1.0, 0.2), v(-0.2, 0.2), v(-0.2, 1.0)];
            painter.add(Shape::line(corner, stroke));
        }
        // A circle with a line touching it from above.
        SnapKind::Tangent => {
            painter.circle_stroke(c + vec2(0.0, 0.2 * R), 0.8 * R, stroke);
            line(v(-1.2, -0.6), v(1.2, -0.6));
        }
        // A plus with the dots of the path it follows.
        SnapKind::Extension => {
            line(v(-1.0, 0.0), v(0.6, 0.0));
            line(v(-0.2, -0.8), v(-0.2, 0.8));
            for x in [1.0, 1.5] {
                painter.circle_filled(v(x, 0.0), 0.7, stroke.color);
            }
        }
        // Two parallel lines.
        SnapKind::Parallel => {
            line(v(-1.2, 0.3), v(0.3, -1.2));
            line(v(-0.3, 1.2), v(1.2, -0.3));
        }
        SnapKind::Nearest => {
            let corners = vec![v(-1.0, -1.0), v(1.0, -1.0), v(-1.0, 1.0), v(1.0, 1.0)];
            painter.add(Shape::closed_line(corners, stroke));
        }
        SnapKind::Grid => {
            line(v(-1.0, -1.0), v(1.0, 1.0));
            line(v(-1.0, 1.0), v(1.0, -1.0));
        }
        // A small X where a tracking path is followed; a polar path is
        // shown by its line and tooltip alone.
        SnapKind::Tracking => {
            line(v(-0.6, -0.6), v(0.6, 0.6));
            line(v(-0.6, 0.6), v(0.6, -0.6));
        }
        SnapKind::Polar => {}
    }
}

impl View {
    /// The marker and tooltip of the snap point in force. `held_to` is where
    /// the cursor really went if ortho moved it off the snap.
    pub(super) fn paint_snap_marker(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        modes: &SnapModes,
        snapped: SnapPoint,
        held_to: Option<Point>,
    ) {
        let c = self.camera.world_to_screen(snapped.point, rect);
        // Ortho moved the point off the snap: show how far.
        if let Some(to) = held_to.map(|p| self.camera.world_to_screen(p, rect)) {
            if (to - c).length() > 2.0 {
                painter.line_segment(
                    [c, to],
                    Stroke::new(1.0_f32, style.snap.gamma_multiply(0.5)),
                );
            }
        }
        if modes.marker {
            glyph(painter, c, snapped.kind, Stroke::new(1.5_f32, style.snap));
        }
        let followed = matches!(snapped.kind, SnapKind::Tracking | SnapKind::Polar);
        if (modes.tooltip && !followed) || (followed && self.snap_cycle.tracking.tooltips) {
            let galley =
                painter.layout_no_wrap(snapped.label(), FontId::proportional(11.0), style.snap);
            // Beside the marker, or on its other side if that runs off the canvas.
            let right = c.x + R + 6.0;
            let at = if right + galley.size().x > rect.right() {
                c.x - R - 6.0 - galley.size().x
            } else {
                right
            };
            painter.galley(
                egui::pos2(at, c.y - R - 6.0 - galley.size().y),
                galley,
                style.snap,
            );
        }
    }

    /// Where the crosshair is drawn: on the snap point when the magnet is on,
    /// otherwise under the mouse.
    pub(super) fn pulled_to(
        &self,
        rect: Rect,
        at: Pos2,
        snapped: Option<SnapPoint>,
        modes: &SnapModes,
    ) -> Pos2 {
        let to = snapped.filter(|_| modes.magnet);
        to.map_or(at, |s| self.camera.world_to_screen(s.point, rect))
    }

    /// The box that shows how far the pointer reaches for a snap point (`APBOX`),
    /// centred on the pointer itself, not on where it was pulled to.
    pub(super) fn paint_aperture_box(
        &self,
        painter: &Painter,
        at: Pos2,
        style: &CanvasStyle,
        shown: bool,
    ) {
        if !shown {
            return;
        }
        let side = 2.0 * self.sizes.snap as f32;
        painter.rect_stroke(
            Rect::from_center_size(at, vec2(side, side)),
            0.0,
            Stroke::new(1.0_f32, style.snap.gamma_multiply(0.7)),
            StrokeKind::Middle,
        );
    }
}
