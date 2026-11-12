//! What AREA and DIST report: the length and area of an object, and of a
//! list of corner points. Pure functions; the commands only ask and say.

use std::f64::consts::PI;

use crate::{arc, bulge, Entity, Point};

/// The length (the perimeter, when `closed`) and the area of one object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub length: f64,
    /// The object ends where it starts: its length is a perimeter or
    /// circumference.
    pub closed: bool,
    /// The area inside, or for an open polyline, spline or ellipse arc the
    /// area it makes with the straight line from its end to its start (as
    /// AutoCAD reports it). `None` for a line or an arc.
    pub area: Option<f64>,
}

impl Entity {
    /// The length and area of this object, or `None` for one that has
    /// neither (text, dimensions, hatches, points, blocks).
    pub fn measured(&self) -> Option<Measured> {
        match self {
            Entity::Line { start, end } => Some(Measured {
                length: (end.x - start.x).hypot(end.y - start.y),
                closed: false,
                area: None,
            }),
            Entity::Arc {
                radius, start, end, ..
            } => Some(Measured {
                length: radius.abs() * arc::sweep(*start, *end),
                closed: false,
                area: None,
            }),
            Entity::Circle { radius, .. } => Some(Measured {
                length: 2.0 * PI * radius.abs(),
                closed: true,
                area: Some(PI * radius * radius),
            }),
            Entity::Rectangle { min, max } => {
                let (w, h) = ((max.x - min.x).abs(), (max.y - min.y).abs());
                Some(Measured {
                    length: 2.0 * (w + h),
                    closed: true,
                    area: Some(w * h),
                })
            }
            Entity::Polyline {
                points,
                bulges,
                closed,
                ..
            } if points.len() >= 2 => Some(Measured {
                length: bulge::path_length(points, bulges, *closed),
                closed: *closed,
                area: Some(bulge::path_area(points, bulges)),
            }),
            Entity::Ellipse(e) => Some(Measured {
                length: e.length(),
                closed: e.is_full(),
                area: Some(e.area()),
            }),
            Entity::Spline(s) => {
                let b = self.bounds();
                let size = (b.max.x - b.min.x).hypot(b.max.y - b.min.y);
                let flat = s.flatten((size * 1e-6).max(1e-9)).ok()?;
                (flat.len() >= 2).then(|| Measured {
                    length: bulge::path_length(&flat, &[], false),
                    closed: s.closed,
                    area: Some(bulge::path_area(&flat, &[])),
                })
            }
            _ => None,
        }
    }
}

/// The area and perimeter of the shape whose corners are `points`, joined in
/// order and back to the first.
pub fn corners(points: &[Point]) -> (f64, f64) {
    (
        bulge::path_area(points, &[]),
        bulge::path_length(points, &[], true),
    )
}

/// What DIST reports for two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Separation {
    pub distance: f64,
    /// Radians counter-clockwise from east, 0 to 2π.
    pub angle: f64,
    pub dx: f64,
    pub dy: f64,
}

/// The distance, direction and offsets from `a` to `b`.
pub fn separation(a: Point, b: Point) -> Separation {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    Separation {
        distance: dx.hypot(dy),
        angle: dy.atan2(dx).rem_euclid(2.0 * PI),
        dx,
        dy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn circle_and_rectangle() {
        let c = Entity::Circle {
            center: p(1.0, 1.0),
            radius: 2.0,
        }
        .measured()
        .unwrap();
        assert!((c.length - 4.0 * PI).abs() < 1e-12 && c.closed);
        assert!((c.area.unwrap() - 4.0 * PI).abs() < 1e-12);
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(3.0, 2.0),
        }
        .measured()
        .unwrap();
        assert_eq!((r.length, r.area), (10.0, Some(6.0)));
    }

    #[test]
    fn lines_and_arcs_have_a_length_and_no_area() {
        let l = Entity::Line {
            start: p(0.0, 0.0),
            end: p(3.0, 4.0),
        }
        .measured()
        .unwrap();
        assert_eq!((l.length, l.closed, l.area), (5.0, false, None));
        let quarter = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 2.0,
            start: 0.0,
            end: PI / 2.0,
        }
        .measured()
        .unwrap();
        assert!((quarter.length - PI).abs() < 1e-12 && quarter.area.is_none());
    }

    #[test]
    fn open_polyline_is_closed_for_area_only() {
        let open = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: vec![],
            widths: vec![],
        }
        .measured()
        .unwrap();
        assert_eq!(
            (open.length, open.closed, open.area),
            (7.0, false, Some(6.0))
        );
        let closed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: true,
            bulges: vec![],
            widths: vec![],
        }
        .measured()
        .unwrap();
        assert_eq!((closed.length, closed.closed), (12.0, true));
    }

    #[test]
    fn a_polyline_with_a_half_circle_counts_the_arc() {
        // A 2 by 2 square whose top is a half circle bulging out.
        let e = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
            closed: true,
            bulges: vec![0.0, 0.0, -1.0, 0.0],
            widths: vec![],
        }
        .measured()
        .unwrap();
        // Bulge -1 turns clockwise, so the half circle bulges inward here.
        let area = e.area.unwrap();
        assert!((area - (4.0 - PI / 2.0)).abs() < 1e-9, "{area}");
        assert!((e.length - (2.0 + 2.0 + 2.0 + PI)).abs() < 1e-9);
    }

    #[test]
    fn things_without_size_are_refused() {
        assert!(Entity::Point { at: p(0.0, 0.0) }.measured().is_none());
    }

    #[test]
    fn corners_give_area_and_perimeter() {
        let (area, perimeter) = corners(&[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]);
        assert_eq!((area, perimeter), (6.0, 12.0));
        // Either way round.
        let (area, _) = corners(&[p(4.0, 3.0), p(4.0, 0.0), p(0.0, 0.0)]);
        assert_eq!(area, 6.0);
    }

    #[test]
    fn separation_reports_direction_and_offsets() {
        let s = separation(p(1.0, 1.0), p(4.0, 5.0));
        assert_eq!((s.distance, s.dx, s.dy), (5.0, 3.0, 4.0));
        assert!((s.angle.to_degrees() - 53.130102).abs() < 1e-5);
        let back = separation(p(0.0, 0.0), p(0.0, -1.0));
        assert!((back.angle.to_degrees() - 270.0).abs() < 1e-9);
    }
}
