//! Exploding a compound object into the simple ones it is made of. The base
//! for EXPLODE.
//!
//! A rectangle becomes four lines and a polyline one line or arc per
//! segment. Lines, circles, arcs and ellipses are already simple.

use crate::intersect::corners;
use crate::{bulge, Entity, Point};

impl Entity {
    /// The lines and arcs this object is made of, in order, or `None` when
    /// it cannot be exploded. Segments with no length are skipped.
    pub fn explode(&self) -> Option<Vec<Entity>> {
        match self {
            Entity::Line { .. }
            | Entity::Circle { .. }
            | Entity::Arc { .. }
            | Entity::Ellipse(_)
            | Entity::Point { .. }
            | Entity::Spline(_) => None,
            Entity::Dimension(d) => Some(d.picture()),
            // Its lines and text would have to become loose pieces, losing
            // the multileader; that needs a command of its own.
            Entity::MLeader(_) => None,
            // Its arrowhead and line, as loose pieces.
            Entity::Leader(l) => Some(l.picture()),
            // Text would have to become glyph outlines, losing its meaning;
            // that needs a command of its own.
            Entity::MText(_) => None,
            // Turning a pattern into its lines loses the hatch; not built.
            Entity::Hatch(_) => None,
            // One level down; the command keeps each part's own layer and
            // properties through `Insert::exploded`.
            Entity::Insert(i) => Some(i.exploded().into_iter().map(|p| p.entity).collect()),
            Entity::Rectangle { min, max } => {
                let c = corners(*min, *max);
                Some(pieces(&c, &[], true))
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => Some(pieces(points, bulges, *closed)),
        }
    }
}

fn pieces(points: &[Point], bulges: &[f64], closed: bool) -> Vec<Entity> {
    let count = bulge::segment_count(points.len(), closed);
    (0..count)
        .filter_map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (b.x - a.x).hypot(b.y - a.y) <= 1e-12 {
                return None;
            }
            Some(match bulge::arc_of(a, b, bulge::at(bulges, i)) {
                Some(s) => Entity::Arc {
                    center: s.center,
                    radius: s.radius,
                    start: s.start,
                    end: s.end,
                },
                None => Entity::Line { start: a, end: b },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn simple_objects_do_not_explode() {
        let line = Entity::Line {
            start: p(0.0, 0.0),
            end: p(1.0, 0.0),
        };
        assert_eq!(line.explode(), None);
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(circle.explode(), None);
    }

    #[test]
    fn a_rectangle_gives_four_lines_round_its_corners() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(4.0, 2.0),
        };
        let parts = r.explode().unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(
            parts[0],
            Entity::Line {
                start: p(0.0, 0.0),
                end: p(4.0, 0.0)
            }
        );
        assert_eq!(
            parts[3],
            Entity::Line {
                start: p(0.0, 2.0),
                end: p(0.0, 0.0)
            }
        );
    }

    #[test]
    fn an_open_polyline_gives_one_piece_per_segment() {
        let pl = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: false,
            bulges: vec![],
            widths: Vec::new(),
        };
        assert_eq!(pl.explode().unwrap().len(), 2);
        let closed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: true,
            bulges: vec![],
            widths: Vec::new(),
        };
        assert_eq!(closed.explode().unwrap().len(), 3);
    }

    #[test]
    fn a_bulged_segment_becomes_an_arc_and_repeated_points_are_skipped() {
        // A half circle from (0,0) to (2,0) bulging counter-clockwise (below).
        let pl = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 0.0), p(2.0, 3.0)],
            closed: false,
            bulges: vec![1.0, 0.0, 0.0, 0.0],
            widths: Vec::new(),
        };
        let parts = pl.explode().unwrap();
        assert_eq!(parts.len(), 2);
        match &parts[0] {
            Entity::Arc { center, radius, .. } => {
                assert!((center.x - 1.0).hypot(center.y) < 1e-9);
                assert!((radius - 1.0).abs() < 1e-9);
            }
            other => panic!("expected an arc, got {other:?}"),
        }
        assert!(matches!(parts[1], Entity::Line { .. }));
    }
}
