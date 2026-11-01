//! Stretching an object: the defining points inside a region move, the rest
//! stay put. The base for STRETCH.
//!
//! - A line moves the end (or ends) inside. An arc does too, keeping the
//!   angle it turns through; a polyline moves its vertices and keeps each
//!   segment's bulge.
//! - A circle or an ellipse moves whole when its centre is inside; its
//!   axes and ends change only through its grips.
//! - A rectangle moves its corners; if what is left is still a rectangle
//!   with sides along the axes it stays one, otherwise it becomes a closed
//!   polyline.
//! - A spline moves its fit points (and is fitted again through them) or,
//!   without fit data it can refit, its control points.
//! - An object with every defining point inside simply moves.

use crate::intersect::corners;
use crate::{arc, bulge, Entity, Point};

const EPS: f64 = 1e-9;

impl Entity {
    /// The object with the points for which `inside` is true moved by
    /// `(dx, dy)`, or `None` when none of them is (nothing to change).
    pub fn stretched(&self, inside: impl Fn(Point) -> bool, dx: f64, dy: f64) -> Option<Entity> {
        let shift = |p: Point, moves: bool| if moves { p.translated(dx, dy) } else { p };
        match self {
            Entity::Dimension(d) => d
                .stretched(&inside, dx, dy)
                .map(|d| Entity::Dimension(Box::new(d))),
            Entity::MLeader(m) => m
                .stretched(&inside, dx, dy)
                .map(|m| Entity::MLeader(Box::new(m))),
            Entity::Leader(l) => l
                .stretched(&inside, dx, dy)
                .map(|l| Entity::Leader(Box::new(l))),
            // Only the insertion point moves the text; the width has its own grip.
            Entity::MText(m) => inside(m.insertion).then(|| self.translated(dx, dy)),
            // A block moves when its insertion point is caught, as in AutoCAD.
            Entity::Insert(i) => inside(i.insertion).then(|| self.translated(dx, dy)),
            Entity::AttributeDefinition(d) => {
                inside(d.text.position).then(|| self.translated(dx, dy))
            }
            // Moved whole when the crossing window holds all of it; its
            // boundary is not stretched piece by piece.
            Entity::Hatch(_) => {
                let b = self.bounds();
                (inside(b.min) && inside(b.max)).then(|| self.translated(dx, dy))
            }
            Entity::Circle { center, .. } => inside(*center).then(|| self.translated(dx, dy)),
            Entity::Point { at } => inside(*at).then(|| self.translated(dx, dy)),
            Entity::Xline { origin, .. } | Entity::Ray { origin, .. } => {
                inside(*origin).then(|| self.translated(dx, dy))
            }
            Entity::Ellipse(e) => inside(e.center).then(|| self.translated(dx, dy)),
            Entity::Spline(s) => s
                .stretched(&inside, dx, dy)
                .map(|s| Entity::Spline(Box::new(s))),
            Entity::Line { start, end } => {
                let (a, b) = (inside(*start), inside(*end));
                (a || b).then(|| Entity::Line {
                    start: shift(*start, a),
                    end: shift(*end, b),
                })
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let from = arc::point_at(*center, *radius, *start);
                let to = arc::point_at(*center, *radius, *end);
                let (a, b) = (inside(from), inside(to));
                match (a, b) {
                    (false, false) => None,
                    (true, true) => Some(self.translated(dx, dy)),
                    _ => {
                        let turn = bulge::from_sweep(arc::sweep(*start, *end), true);
                        let s = bulge::arc_of(shift(from, a), shift(to, b), turn)?;
                        Some(Entity::Arc {
                            center: s.center,
                            radius: s.radius,
                            start: s.start,
                            end: s.end,
                        })
                    }
                }
            }
            Entity::Rectangle { min, max } => {
                let c = corners(*min, *max);
                let moves: Vec<bool> = c.iter().map(|p| inside(*p)).collect();
                if moves.iter().all(|m| !m) {
                    return None;
                }
                let moved: Vec<Point> = c.iter().zip(&moves).map(|(p, m)| shift(*p, *m)).collect();
                Some(rectangle_or_polyline(moved))
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => {
                let moves: Vec<bool> = points.iter().map(|p| inside(*p)).collect();
                if moves.iter().all(|m| !m) {
                    return None;
                }
                Some(Entity::Polyline {
                    points: points
                        .iter()
                        .zip(&moves)
                        .map(|(p, m)| shift(*p, *m))
                        .collect(),
                    closed: *closed,
                    bulges: bulges.clone(),
                    widths: widths.clone(),
                })
            }
        }
    }
}

/// Four corners in order: a rectangle if the sides run along the axes.
fn rectangle_or_polyline(c: Vec<Point>) -> Entity {
    let level = |a: f64, b: f64| (a - b).abs() < EPS;
    if level(c[0].y, c[1].y)
        && level(c[1].x, c[2].x)
        && level(c[2].y, c[3].y)
        && level(c[3].x, c[0].x)
    {
        let (min, max) = (
            Point::new(c[0].x.min(c[2].x), c[0].y.min(c[2].y)),
            Point::new(c[0].x.max(c[2].x), c[0].y.max(c[2].y)),
        );
        return Entity::Rectangle { min, max };
    }
    Entity::Polyline {
        points: c,
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Everything left of x = 5.
    fn left(q: Point) -> bool {
        q.x < 5.0
    }

    #[test]
    fn a_line_moves_only_the_end_inside() {
        let l = Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        };
        assert_eq!(
            l.stretched(left, 0.0, 3.0),
            Some(Entity::Line {
                start: p(0.0, 3.0),
                end: p(10.0, 0.0)
            })
        );
        assert_eq!(l.stretched(|_| false, 1.0, 1.0), None);
        assert_eq!(
            l.stretched(|_| true, 1.0, 1.0),
            Some(l.translated(1.0, 1.0))
        );
    }

    #[test]
    fn a_circle_moves_when_its_centre_is_inside() {
        let c = Entity::Circle {
            center: p(1.0, 0.0),
            radius: 2.0,
        };
        assert_eq!(c.stretched(left, 4.0, 0.0), Some(c.translated(4.0, 0.0)));
        let far = Entity::Circle {
            center: p(9.0, 0.0),
            radius: 20.0,
        };
        assert_eq!(far.stretched(left, 4.0, 0.0), None);
    }

    #[test]
    fn an_arc_keeps_its_angle_when_one_end_moves() {
        // A quarter circle of radius 1 about the origin, from east to north.
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 1.0,
            start: 0.0,
            end: std::f64::consts::FRAC_PI_2,
        };
        // Move the north end (x = 0 is inside) 1 to the left, so the chord grows.
        let Some(Entity::Arc {
            center,
            radius,
            start,
            end,
        }) = a.stretched(|q| q.x < 0.5, -1.0, 0.0)
        else {
            panic!("expected an arc");
        };
        assert!(radius > 1.0);
        assert!((arc::sweep(start, end) - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let s = arc::point_at(center, radius, start);
        assert!(
            (s.x - 1.0).abs() < 1e-9 && s.y.abs() < 1e-9,
            "east end stays"
        );
    }

    #[test]
    fn a_polyline_moves_its_vertices_inside() {
        let pl = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(8.0, 0.0)],
            closed: false,
            bulges: vec![0.5, 0.0, 0.0],
            widths: Vec::new(),
        };
        let Some(Entity::Polyline { points, bulges, .. }) = pl.stretched(left, 0.0, 2.0) else {
            panic!("expected a polyline");
        };
        assert_eq!(points, [p(0.0, 2.0), p(4.0, 2.0), p(8.0, 0.0)]);
        assert_eq!(bulges, [0.5, 0.0, 0.0]);
    }

    #[test]
    fn a_rectangle_stays_one_when_a_side_moves_along_its_axis() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 4.0),
        };
        // The left side (both left corners) moves left.
        assert_eq!(
            r.stretched(left, -2.0, 0.0),
            Some(Entity::Rectangle {
                min: p(-2.0, 0.0),
                max: p(10.0, 4.0)
            })
        );
    }

    #[test]
    fn a_rectangle_with_one_corner_moved_becomes_a_closed_polyline() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 4.0),
        };
        let one = |q: Point| q.x < 5.0 && q.y < 2.0;
        let Some(Entity::Polyline { points, closed, .. }) = r.stretched(one, -1.0, -1.0) else {
            panic!("expected a polyline");
        };
        assert!(closed);
        assert_eq!(points[0], p(-1.0, -1.0));
        assert_eq!(points[1], p(10.0, 0.0));
    }

    #[test]
    fn an_ellipse_moves_whole_when_its_centre_is_caught() {
        let e = crate::Ellipse::new(Point::new(0.0, 0.0), Point::new(4.0, 0.0), 0.5, 0.0, 2.0);
        let e = Entity::Ellipse(e.unwrap());
        let near_centre = |q: Point| q.x.abs() < 1.0 && q.y.abs() < 1.0;
        assert_eq!(
            e.stretched(near_centre, 3.0, 1.0),
            Some(e.translated(3.0, 1.0))
        );
        // Catching only an end changes nothing: that is what grips are for.
        assert_eq!(e.stretched(|q: Point| q.x > 3.0, 3.0, 1.0), None);
    }
}
