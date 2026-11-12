//! Ghost dimensions that carry the dynamic-input fields, as AutoCAD does: a
//! length and an angle for a vector, two orthogonal lengths for a rectangle.
//! Pure screen-space geometry; the overlay paints it and seats the boxes.
use eframe::egui::{Painter, Pos2, Stroke, Vec2};
use std::f32::consts::TAU;

use super::Field;

/// The size of a numeric field's box.
pub const BOX: Vec2 = Vec2::new(68.0, 24.0);
/// A dimension line never stands closer than this to the geometry it measures.
const MIN_OFFSET: f32 = 24.0;
/// Clear space between a box and the line it must not touch.
const MARGIN: f32 = 8.0;
/// The gap left between the geometry and the start of an extension line.
const GAP: f32 = 4.0;
/// How far an extension line runs past its dimension line.
const OVERSHOOT: f32 = 6.0;
const ARROW: f32 = 7.0;
/// Shorter than this a dimension has no room to be drawn.
const MIN_LENGTH: f32 = 24.0;

/// What to draw and where each field's box is centred.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub boxes: Vec<(Field, Pos2)>,
    pub lines: Vec<Vec<Pos2>>,
    /// Arrowhead tip and the direction it points.
    pub arrows: Vec<(Pos2, Vec2)>,
}

impl Layout {
    pub fn paint(&self, painter: &Painter, stroke: Stroke) {
        for line in &self.lines {
            painter.add(eframe::egui::Shape::line(line.clone(), stroke));
        }
        for (tip, dir) in &self.arrows {
            let side = Vec2::new(-dir.y, dir.x) * (ARROW * 0.3);
            let back = *tip - *dir * ARROW;
            painter.add(eframe::egui::Shape::convex_polygon(
                vec![*tip, back + side, back - side],
                stroke.color,
                Stroke::NONE,
            ));
        }
    }

    /// A dimension of `from`..`to` standing far enough `out` to the side that
    /// its box, centred on the line, clears the measured geometry. Returns the
    /// offset used.
    fn dimension(&mut self, from: Pos2, to: Pos2, out: Vec2) -> f32 {
        let offset = (reach(out) + MARGIN).max(MIN_OFFSET);
        let along = (to - from).normalized();
        let (a, b) = (from + out * offset, to + out * offset);
        for (end, tip) in [(from, a), (to, b)] {
            self.lines
                .push(vec![end + out * GAP, tip + out * OVERSHOOT]);
        }
        self.lines.push(vec![a, b]);
        self.arrows.push((a, -along));
        self.arrows.push((b, along));
        offset
    }
}

/// How far a field's box reaches from its centre in the direction `v`.
pub fn reach(v: Vec2) -> f32 {
    (BOX.x / 2.0) * v.x.abs() + (BOX.y / 2.0) * v.y.abs()
}

/// The side of a line its dimension stands on. It alternates with each
/// quadrant the line points into (angles counter-clockwise on screen): 0-90
/// and 180-270 take the left of the line's direction as drawn on screen
/// (above, for a line pointing right), 90-180 and 270-360 the other side.
fn outward(along: Vec2) -> Vec2 {
    let n = Vec2::new(along.y, -along.x);
    let quadrant = ((-along.y).atan2(along.x).rem_euclid(TAU) / (TAU / 4.0)).floor() as i32;
    if quadrant % 2 == 0 {
        n
    } else {
        -n
    }
}

/// A length along `base`..`cursor`.
pub fn linear(base: Pos2, cursor: Pos2) -> Option<Layout> {
    let d = cursor - base;
    if d.length() < MIN_LENGTH {
        return None;
    }
    let out = outward(d.normalized());
    let mut layout = Layout::default();
    let offset = layout.dimension(base, cursor, out);
    layout
        .boxes
        .push((Field::Distance, base + d * 0.5 + out * offset));
    Some(layout)
}

/// A length along `base`..`cursor` and the angle it makes with the zero
/// direction. `zero` is that direction's screen angle; `step` is 1.0 when
/// angles grow clockwise on screen and -1.0 when they grow counter-clockwise.
pub fn polar(base: Pos2, cursor: Pos2, zero: f32, step: f32) -> Option<Layout> {
    let mut layout = linear(base, cursor)?;
    let d = cursor - base;
    // The arc runs past the end of the line it measures.
    let radius = d.length() + 18.0;
    // Past a half turn the arc flips to the other side and counts negative.
    let mut sweep = ((d.y.atan2(d.x) - zero) * step).rem_euclid(TAU);
    if sweep > TAU / 2.0 {
        sweep -= TAU;
    }
    let at = |t: f32| {
        let a = zero + step * sweep * t;
        base + Vec2::angled(a) * radius
    };
    let points: Vec<Pos2> = (0..=24).map(|i| at(i as f32 / 24.0)).collect();
    let tangent = |from: Pos2, to: Pos2| (to - from).normalized();
    if sweep.abs() > 1e-3 {
        layout
            .arrows
            .push((points[0], tangent(points[1], points[0])));
        layout
            .arrows
            .push((points[24], tangent(points[23], points[24])));
    }
    layout.lines.push(vec![
        base + Vec2::angled(zero) * (GAP),
        base + Vec2::angled(zero) * (radius + OVERSHOOT),
    ]);
    layout.lines.push(points);
    // The box sits outside the arc, clear of the line.
    let mid = Vec2::angled(zero + step * sweep * 0.5);
    layout
        .boxes
        .push((Field::Angle, at(0.5) + mid * (reach(mid) + 4.0)));
    Some(layout)
}

/// The two sides of the box with corners `base` and `cursor`: the width under
/// (or over) the bottom edge, the height beside the edge the cursor is on.
pub fn cartesian(base: Pos2, cursor: Pos2) -> Option<Layout> {
    let d = cursor - base;
    if d.x.abs() < MIN_LENGTH || d.y.abs() < MIN_LENGTH {
        return None;
    }
    let corner = Pos2::new(cursor.x, base.y);
    let below = Vec2::new(0.0, -d.y.signum());
    let beside = Vec2::new(d.x.signum(), 0.0);
    let mut layout = Layout::default();
    let below_by = layout.dimension(base, corner, below);
    let beside_by = layout.dimension(corner, cursor, beside);
    layout.boxes.push((
        Field::X,
        base + Vec2::new(d.x * 0.5, 0.0) + below * below_by,
    ));
    layout.boxes.push((
        Field::Y,
        corner + Vec2::new(0.0, d.y * 0.5) + beside * beside_by,
    ));
    Some(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centre(layout: &Layout, field: Field) -> Pos2 {
        layout.boxes.iter().find(|b| b.0 == field).unwrap().1
    }

    #[test]
    fn distance_side_alternates_with_each_quadrant() {
        // Screen y points down, so a line up and to the right is 0-90.
        let side = |x: f32, y: f32| {
            let layout = linear(Pos2::ZERO, Pos2::new(x, y)).unwrap();
            let mid = Pos2::new(x / 2.0, y / 2.0);
            let c = centre(&layout, Field::Distance);
            // The box's near edge stays clear of the line it measures.
            let n = (c - mid).normalized();
            assert!((c - mid).length() - reach(n) >= MARGIN - 1e-3);
            let u = Vec2::new(x, y).normalized();
            n.dot(Vec2::new(u.y, -u.x)).signum()
        };
        // Each step goes over an axis, so the side swaps every time.
        let around = [(100.0, -50.0), (-1.0, -100.0), (-100.0, 50.0), (1.0, 100.0)];
        assert_eq!(side(100.0, -1.0), 1.0);
        assert_eq!(side(1.0, -100.0), 1.0);
        for (i, (x, y)) in around.into_iter().enumerate() {
            let expected = if i % 2 == 0 { 1.0 } else { -1.0 };
            assert_eq!(side(x, y), expected, "({x},{y})");
        }
        assert_eq!(side(-100.0, -1.0), -1.0);
        assert_eq!(side(-100.0, 1.0), 1.0);
        assert_eq!(side(-1.0, 100.0), 1.0);
        assert_eq!(side(1.0, 100.0), -1.0);
        assert_eq!(side(100.0, 1.0), -1.0);
    }

    #[test]
    fn short_vectors_have_no_ghost_dimensions() {
        assert!(linear(Pos2::ZERO, Pos2::new(10.0, 5.0)).is_none());
        assert!(polar(Pos2::ZERO, Pos2::ZERO, 0.0, -1.0).is_none());
        assert!(cartesian(Pos2::ZERO, Pos2::new(100.0, 10.0)).is_none());
    }

    #[test]
    fn angle_box_sits_on_an_arc_between_zero_and_the_line() {
        // Up and to the right on screen is 45 degrees counter-clockwise.
        let cursor = Pos2::new(100.0, -100.0);
        let layout = polar(Pos2::ZERO, cursor, 0.0, -1.0).unwrap();
        let c = centre(&layout, Field::Angle);
        // The box lies beyond the arc, which lies beyond the line's end.
        assert!(c.to_vec2().length() > cursor.to_vec2().length() + 18.0);
        assert!((c.y / c.x + (22.5_f32).to_radians().tan()).abs() < 1e-3);
        // Clockwise angles put the same line at -45 degrees: the arc flips
        // to the short way round instead of sweeping 315.
        let clockwise = polar(Pos2::ZERO, cursor, 0.0, 1.0).unwrap();
        assert!(centre(&clockwise, Field::Angle).y < 0.0);
    }

    #[test]
    fn rectangle_dimensions_stand_outside_the_box() {
        for (dx, dy) in [
            (120.0, -80.0),
            (-120.0, -80.0),
            (120.0, 80.0),
            (-120.0, 80.0),
        ] {
            let layout = cartesian(Pos2::ZERO, Pos2::new(dx, dy)).unwrap();
            let x = centre(&layout, Field::X);
            let y = centre(&layout, Field::Y);
            assert_eq!(x.x, dx / 2.0);
            assert_eq!(x.y.signum(), -dy.signum());
            assert_eq!(y.y, dy / 2.0);
            assert_eq!((y.x - dx).signum(), dx.signum());
            // Each box clears the edge it measures.
            assert!(x.y.abs() - BOX.y / 2.0 >= MARGIN - 1e-3);
            assert!((y.x - dx).abs() - BOX.x / 2.0 >= MARGIN - 1e-3);
        }
    }
}
