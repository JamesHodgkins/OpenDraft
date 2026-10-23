//! What a dimension is drawn with: lines, arcs, filled arrowheads and text
//! strokes, collected as ordinary entities.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use super::font;
use super::style::Arrow;
use crate::text_style::TextStyleId;
use crate::{Entity, Point};

const TINY: f64 = 1e-12;

pub(super) fn dist(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

pub(super) fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

pub(super) fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

pub(super) fn sub(a: Point, b: Point) -> (f64, f64) {
    (a.x - b.x, a.y - b.y)
}

pub(super) fn along(p: Point, dir: (f64, f64), t: f64) -> Point {
    Point::new(p.x + dir.0 * t, p.y + dir.1 * t)
}

pub(super) fn unit(v: (f64, f64)) -> Option<(f64, f64)> {
    let len = v.0.hypot(v.1);
    (len.is_finite() && len > TINY).then(|| (v.0 / len, v.1 / len))
}

pub(super) fn direction(angle: f64) -> (f64, f64) {
    (angle.cos(), angle.sin())
}

pub(super) fn left(v: (f64, f64)) -> (f64, f64) {
    (-v.1, v.0)
}

/// An angle for text along `angle` that never reads upside down: turned by a
/// half turn when it would point left (AutoCAD keeps text readable from the
/// bottom and the right).
pub(super) fn readable(angle: f64) -> f64 {
    let a = angle.rem_euclid(TAU);
    if a > FRAC_PI_2 + 1e-9 && a <= 3.0 * FRAC_PI_2 + 1e-9 {
        (a - PI).rem_euclid(TAU)
    } else {
        a
    }
}

/// Where the line `a`-`b` lies outside a box centred on `centre`, turned by
/// `angle`, with half sizes `half`: zero, one or two pieces.
pub(super) fn outside_box(
    a: Point,
    b: Point,
    centre: Point,
    angle: f64,
    half: (f64, f64),
) -> Vec<(Point, Point)> {
    let to_box = |p: Point| {
        let q = p.rotated(centre, -angle);
        (q.x - centre.x, q.y - centre.y)
    };
    let (pa, pb) = (to_box(a), to_box(b));
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let delta = (pb.0 - pa.0, pb.1 - pa.1);
    for (p, d, h) in [(pa.0, delta.0, half.0), (pa.1, delta.1, half.1)] {
        if d.abs() < TINY {
            if p.abs() > h {
                return vec![(a, b)];
            }
        } else {
            let (u, v) = ((-h - p) / d, (h - p) / d);
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
        }
    }
    if t0 >= t1 {
        return vec![(a, b)];
    }
    let at = |t: f64| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    let mut pieces = Vec::new();
    if t0 > 1e-9 {
        pieces.push((a, at(t0)));
    }
    if t1 < 1.0 - 1e-9 {
        pieces.push((at(t1), b));
    }
    pieces
}

#[derive(Default)]
pub(super) struct Sketch {
    pub pieces: Vec<Entity>,
}

impl Sketch {
    pub fn line(&mut self, start: Point, end: Point) {
        if dist(start, end) > TINY {
            self.pieces.push(Entity::Line { start, end });
        }
    }

    pub fn arc(&mut self, center: Point, radius: f64, start: f64, end: f64) {
        if radius > TINY {
            self.pieces.push(Entity::Arc {
                center,
                radius,
                start: crate::arc::normalize(start),
                end: crate::arc::normalize(end),
            });
        }
    }

    pub fn polyline(&mut self, points: Vec<Point>) {
        if points.len() >= 2 {
            self.pieces.push(Entity::Polyline {
                points,
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new(),
            });
        }
    }

    /// An arrowhead with its point at `tip`; `back` is the unit direction
    /// from the tip along the arrow's body.
    pub fn arrow(&mut self, kind: Arrow, tip: Point, back: (f64, f64), size: f64) {
        let side = left(back);
        match kind {
            Arrow::None => {}
            Arrow::Closed => {
                // A tapering wide segment is a filled triangle.
                self.pieces.push(Entity::Polyline {
                    points: vec![along(tip, back, size), tip],
                    closed: false,
                    bulges: Vec::new(),
                    widths: vec![(size / 3.0, 0.0), (0.0, 0.0)],
                });
            }
            Arrow::Open => {
                let tail = along(tip, back, size);
                let half = size / 6.0;
                self.polyline(vec![along(tail, side, half), tip, along(tail, side, -half)]);
            }
            Arrow::Tick => {
                let (s, c) = FRAC_PI_4.sin_cos();
                let slash = (back.0 * c - back.1 * s, back.0 * s + back.1 * c);
                self.line(
                    along(tip, slash, -size / 2.0),
                    along(tip, slash, size / 2.0),
                );
            }
        }
    }

    /// Text with its middle at `centre`, along `angle`.
    pub fn text(&mut self, text: &str, height: f64, centre: Point, angle: f64, style: TextStyleId) {
        if let Some(label) = font::place(text, height, centre, angle, style) {
            self.pieces.push(Entity::MText(Box::new(label)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn readable_flips_left_pointing_text() {
        assert!((readable(0.0)).abs() < 1e-12);
        assert!((readable(PI) - 0.0).abs() < 1e-9);
        assert!((readable(FRAC_PI_2) - FRAC_PI_2).abs() < 1e-9);
        assert!((readable(-FRAC_PI_2) - FRAC_PI_2).abs() < 1e-9);
        assert!((readable(0.75 * PI) - (0.75 * PI - PI).rem_euclid(TAU)).abs() < 1e-9);
    }

    #[test]
    fn a_box_cuts_a_line_in_two() {
        let pieces = outside_box(p(0.0, 0.0), p(10.0, 0.0), p(5.0, 0.0), 0.0, (1.0, 1.0));
        assert_eq!(pieces.len(), 2);
        assert!((pieces[0].1.x - 4.0).abs() < 1e-9);
        assert!((pieces[1].0.x - 6.0).abs() < 1e-9);
    }

    #[test]
    fn a_box_off_the_line_leaves_it_whole() {
        let pieces = outside_box(p(0.0, 0.0), p(10.0, 0.0), p(5.0, 5.0), 0.0, (1.0, 1.0));
        assert_eq!(pieces, vec![(p(0.0, 0.0), p(10.0, 0.0))]);
    }

    #[test]
    fn a_box_over_the_end_trims_it() {
        let pieces = outside_box(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 0.0), 0.0, (2.0, 1.0));
        assert_eq!(pieces.len(), 1);
        assert!((pieces[0].0.x - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_turned_box_cuts_a_sloped_line() {
        let pieces = outside_box(
            p(0.0, 0.0),
            p(10.0, 10.0),
            p(5.0, 5.0),
            FRAC_PI_4,
            (1.0, 1.0),
        );
        assert_eq!(pieces.len(), 2);
        assert!((dist(pieces[0].1, p(5.0, 5.0)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_closed_arrow_is_a_filled_triangle() {
        let mut s = Sketch::default();
        s.arrow(Arrow::Closed, p(0.0, 0.0), (1.0, 0.0), 3.0);
        let Entity::Polyline { points, widths, .. } = &s.pieces[0] else {
            panic!("polyline expected");
        };
        assert_eq!(points[1], p(0.0, 0.0));
        assert_eq!(points[0], p(3.0, 0.0));
        assert_eq!(widths[0], (1.0, 0.0));
        assert_eq!(widths.len(), points.len());
    }

    #[test]
    fn no_arrow_draws_nothing() {
        let mut s = Sketch::default();
        s.arrow(Arrow::None, p(0.0, 0.0), (1.0, 0.0), 3.0);
        assert!(s.pieces.is_empty());
    }
}
