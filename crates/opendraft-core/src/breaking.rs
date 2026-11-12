//! Breaking an object between two points. The base for BREAK.
//!
//! Both points are moved to where the outline is nearest them, the piece
//! between them is dropped and what is left comes back (the geometry of the
//! cut is `Entity::split_at`).

use crate::{Entity, Point};

/// Points closer than this are the same point.
const SAME: f64 = 1e-7;

fn same(a: Point, b: Point) -> bool {
    (a.x - b.x).hypot(a.y - b.y) < SAME
}

impl Entity {
    /// What is left of the object when the part between `first` and `second`
    /// is removed, in order along it. An empty list means the whole object
    /// goes (the points were at its two ends). With both points in the same
    /// place nothing is removed: the object is only divided there in two
    /// (a circle, or a closed shape, comes back whole). `None` when the
    /// object has no outline to break, or is a spline.
    ///
    /// On a line, arc or open polyline it does not matter which point comes
    /// first. On a circle, rectangle or closed polyline the part removed runs
    /// from `first` to `second` the way the outline is drawn (counter-
    /// clockwise for a circle, a rectangle and an anticlockwise polyline).
    /// A circle becomes an arc, a full ellipse an elliptical arc, a rectangle
    /// an open polyline.
    pub fn break_between(&self, first: Point, second: Point) -> Option<Vec<Entity>> {
        // Not cut yet: that needs knot insertion.
        if matches!(self, Entity::Spline(_)) {
            return None;
        }
        if self.infinite_line().is_some() {
            let pieces = self.long_line().break_between(first, second)?;
            return Some(pieces.into_iter().map(|p| self.resumed(p)).collect());
        }
        let (a, b) = (self.nearest_point(first)?, self.nearest_point(second)?);
        let pieces = self.split_at(&[a, b]);
        if same(a, b) {
            return Some(pieces);
        }
        let closed = matches!(
            self,
            Entity::Circle { .. }
                | Entity::Rectangle { .. }
                | Entity::Polyline { closed: true, .. }
        ) || matches!(self, Entity::Ellipse(e) if e.is_full());
        let ends = |piece: &Entity| -> Option<(Point, Point)> {
            match piece {
                Entity::Line { start, end } => Some((*start, *end)),
                Entity::Arc {
                    center,
                    radius,
                    start,
                    end,
                } => Some((
                    crate::arc::point_at(*center, *radius, *start),
                    crate::arc::point_at(*center, *radius, *end),
                )),
                Entity::Polyline { points, .. } => Some((*points.first()?, *points.last()?)),
                Entity::Ellipse(e) if !e.is_full() => Some((e.start_point(), e.end_point())),
                _ => None,
            }
        };
        let between = |piece: &Entity| {
            ends(piece).is_some_and(|(s, e)| {
                (same(s, a) && same(e, b)) || (!closed && same(s, b) && same(e, a))
            })
        };
        // A circle has no ends of its own: an unsplit one has nothing between.
        let Some(gone) = pieces.iter().position(between) else {
            return Some(pieces);
        };
        let mut left = pieces;
        left.remove(gone);
        Some(left)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    #[test]
    fn a_line_loses_the_part_between_the_points_whichever_comes_first() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        let want = vec![
            line(p(0.0, 0.0), p(3.0, 0.0)),
            line(p(7.0, 0.0), p(10.0, 0.0)),
        ];
        assert_eq!(
            l.break_between(p(3.0, 1.0), p(7.0, -2.0)),
            Some(want.clone())
        );
        assert_eq!(l.break_between(p(7.0, 0.0), p(3.0, 0.0)), Some(want));
    }

    #[test]
    fn a_second_point_past_the_end_removes_to_the_end() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            l.break_between(p(4.0, 0.0), p(50.0, 0.0)),
            Some(vec![line(p(0.0, 0.0), p(4.0, 0.0))])
        );
        assert_eq!(l.break_between(p(-5.0, 0.0), p(50.0, 0.0)), Some(vec![]));
    }

    #[test]
    fn the_same_point_twice_only_divides_the_line() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            l.break_between(p(4.0, 0.0), p(4.0, 3.0)),
            Some(vec![
                line(p(0.0, 0.0), p(4.0, 0.0)),
                line(p(4.0, 0.0), p(10.0, 0.0))
            ])
        );
    }

    #[test]
    fn a_circle_loses_the_arc_counter_clockwise_from_the_first_point() {
        let c = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let left = c.break_between(p(2.0, 0.0), p(0.0, 2.0)).unwrap();
        assert_eq!(left.len(), 1);
        match &left[0] {
            Entity::Arc { start, end, .. } => {
                assert!((start - FRAC_PI_2).abs() < 1e-9);
                assert!((end - 2.0 * PI).abs() < 1e-9 || end.abs() < 1e-9);
            }
            other => panic!("expected an arc, got {other:?}"),
        }
    }

    #[test]
    fn a_rectangle_becomes_an_open_polyline_missing_the_part_between() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(4.0, 2.0),
        };
        // From the middle of the bottom edge to the middle of the right edge.
        let left = r.break_between(p(2.0, 0.0), p(4.0, 1.0)).unwrap();
        assert_eq!(left.len(), 1);
        match &left[0] {
            Entity::Polyline { points, closed, .. } => {
                assert!(!closed);
                assert_eq!(points.first(), Some(&p(4.0, 1.0)));
                assert_eq!(points.last(), Some(&p(2.0, 0.0)));
            }
            other => panic!("expected a polyline, got {other:?}"),
        }
    }

    #[test]
    fn an_open_polyline_keeps_both_ends() {
        let pl = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)],
            closed: false,
            bulges: vec![],
            widths: Vec::new(),
        };
        let left = pl.break_between(p(2.0, 0.0), p(4.0, 2.0)).unwrap();
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn an_ellipse_breaks_like_a_circle_and_an_elliptical_arc_like_an_arc() {
        let near = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y) < 1e-9;
        let e = crate::Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        // From the end of the major axis round to the top: that quarter goes.
        let left = Entity::Ellipse(e)
            .break_between(p(4.0, 0.0), p(0.0, 2.0))
            .unwrap();
        let [Entity::Ellipse(rest)] = left.as_slice() else {
            panic!("one elliptical arc, got {left:?}")
        };
        assert!(near(rest.start_point(), p(0.0, 2.0)) && near(rest.end_point(), p(4.0, 0.0)));
        assert!((rest.sweep() - 1.5 * PI).abs() < 1e-9);
        // An arc loses the part between, whichever point comes first.
        let half = crate::Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, PI).unwrap();
        let left = Entity::Ellipse(half)
            .break_between(e.point_at(2.0), e.point_at(FRAC_PI_2))
            .unwrap();
        assert_eq!(left.len(), 2);
        let total: f64 = left
            .iter()
            .map(|piece| match piece {
                Entity::Ellipse(e) => e.sweep(),
                other => panic!("expected elliptical arcs, got {other:?}"),
            })
            .sum();
        assert!((total - (PI - (2.0 - FRAC_PI_2))).abs() < 1e-9);
    }
}
