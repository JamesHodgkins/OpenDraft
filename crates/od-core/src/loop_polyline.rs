//! A hatch boundary loop as a closed polyline: what BOUNDARY makes of the
//! loops the tracer finds.
//!
//! Lines and arcs keep their shape (arcs as bulges); a loop with an ellipse
//! or a spline in it, or a circle that is a single edge, is followed within a
//! tolerance and made of straight segments.

use std::f64::consts::TAU;

use crate::hatch::{HatchBoundary, HatchEdge, HatchLoop};
use crate::{arc, bulge, Entity, Point};

/// The bulge of half the arc a bulge makes.
fn half(bulge: f64) -> f64 {
    (bulge.atan() / 2.0).tan()
}

fn closed(points: Vec<Point>, bulges: Vec<f64>) -> Entity {
    Entity::Polyline {
        points,
        closed: true,
        bulges,
        widths: Vec::new(),
    }
}

/// A loop of vertices and bulges. A closed polyline of two vertices is one
/// segment here (see `bulge::segment_count`) though a boundary reads it as
/// two, so each arc of such a loop is cut in half.
fn from_vertices(vertices: &[Point], bulges: &[f64]) -> Option<Entity> {
    match vertices {
        [a, b] => {
            let (first, second) = (bulge::at(bulges, 0), bulge::at(bulges, 1));
            let points = vec![
                *a,
                bulge::midpoint(*a, *b, first),
                *b,
                bulge::midpoint(*b, *a, second),
            ];
            let bulges = vec![half(first), half(first), half(second), half(second)];
            Some(closed(points, bulges))
        }
        [] | [_] => None,
        _ => {
            let bulges = if bulges.is_empty() {
                Vec::new()
            } else {
                bulges.to_vec()
            };
            Some(closed(vertices.to_vec(), bulges))
        }
    }
}

/// The start of an edge and the bulge of its segment, for a line or an arc
/// less than a whole circle.
fn segment(edge: &HatchEdge) -> Option<(Point, f64)> {
    match edge {
        HatchEdge::Line { start, .. } => Some((*start, 0.0)),
        HatchEdge::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            counter_clockwise,
        } => {
            let sweep = if *counter_clockwise {
                arc::sweep(*start_angle, *end_angle)
            } else {
                -arc::sweep(*end_angle, *start_angle)
            };
            if !radius.is_finite() || *radius <= 0.0 || sweep.abs() >= TAU - 1e-9 {
                return None;
            }
            let at = arc::point_at(*center, *radius, *start_angle);
            Some((at, (sweep / 4.0).tan()))
        }
        _ => None,
    }
}

/// A whole circle (one edge) as four quarter arcs.
fn circle(edge: &HatchEdge) -> Option<Entity> {
    let HatchEdge::CircularArc {
        center,
        radius,
        start_angle,
        end_angle,
        counter_clockwise,
    } = edge
    else {
        return None;
    };
    let sweep = arc::sweep(*start_angle, *end_angle);
    let whole = sweep < 1e-9 || sweep > TAU - 1e-9;
    if !whole || !radius.is_finite() || *radius <= 0.0 {
        return None;
    }
    let turn = if *counter_clockwise { 1.0 } else { -1.0 };
    let points = (0..4)
        .map(|k| {
            let angle = start_angle + turn * f64::from(k) * std::f64::consts::FRAC_PI_2;
            arc::point_at(*center, *radius, angle)
        })
        .collect();
    let bulge = turn * std::f64::consts::FRAC_PI_8.tan();
    Some(closed(points, vec![bulge; 4]))
}

/// The loop as a closed polyline, curves other than arcs followed within
/// `tolerance`; `None` when it cannot be made (an open or empty loop).
pub fn loop_polyline(l: &HatchLoop, tolerance: f64) -> Option<Entity> {
    if let HatchBoundary::Polyline(p) = &l.boundary {
        return from_vertices(&p.vertices, &p.bulges);
    }
    if let HatchBoundary::Edges(edges) = &l.boundary {
        if let [only] = edges.as_slice() {
            if let Some(circle) = circle(only) {
                return Some(circle);
            }
        }
        let found: Option<Vec<(Point, f64)>> = edges.iter().map(segment).collect();
        if let Some(found) = found.filter(|f| f.len() >= 2) {
            let (points, bulges): (Vec<Point>, Vec<f64>) = found.into_iter().unzip();
            return from_vertices(&points, &bulges);
        }
    }
    let points = l.contour(0, tolerance, tolerance * 10.0).ok()?;
    Some(closed(points, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hatch::loops_from_entities;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn area(e: &Entity) -> f64 {
        match e {
            Entity::Polyline { points, bulges, .. } => bulge::path_area(points, bulges).abs(),
            other => panic!("{other:?}"),
        }
    }

    fn only(entities: &[Entity]) -> HatchLoop {
        let (mut loops, _) = loops_from_entities(entities, 1e-6).unwrap();
        assert_eq!(loops.len(), 1);
        loops.remove(0)
    }

    #[test]
    fn a_circle_becomes_four_arcs_with_its_area() {
        let circle = Entity::Circle {
            center: p(1.0, 1.0),
            radius: 2.0,
        };
        let e = loop_polyline(&only(&[circle]), 1e-6).unwrap();
        let Entity::Polyline { points, .. } = &e else {
            unreachable!()
        };
        assert_eq!(points.len(), 4);
        assert!(
            (area(&e) - std::f64::consts::PI * 4.0).abs() < 1e-9,
            "{}",
            area(&e)
        );
    }

    #[test]
    fn lines_and_an_arc_keep_the_arc() {
        // A half disc: the diameter and the arc over it.
        let diameter = Entity::Line {
            start: p(-1.0, 0.0),
            end: p(1.0, 0.0),
        };
        let top = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 1.0,
            start: 0.0,
            end: std::f64::consts::PI,
        };
        let e = loop_polyline(&only(&[diameter, top]), 1e-6).unwrap();
        let Entity::Polyline { points, bulges, .. } = &e else {
            unreachable!()
        };
        assert_eq!(points.len(), 4, "two segments, one cut in half each");
        assert!(bulges.iter().any(|b| b.abs() > 0.1));
        assert!(
            (area(&e) - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "{}",
            area(&e)
        );
    }

    #[test]
    fn a_rectangle_is_four_straight_segments() {
        let square = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(3.0, 2.0),
        };
        let e = loop_polyline(&only(&[square]), 1e-6).unwrap();
        assert!((area(&e) - 6.0).abs() < 1e-12);
    }

    #[test]
    fn an_ellipse_is_followed_within_the_tolerance() {
        let ellipse = crate::Ellipse::new(p(0.0, 0.0), p(3.0, 0.0), 0.5, 0.0, TAU).unwrap();
        let e = loop_polyline(&only(&[Entity::Ellipse(ellipse)]), 1e-4).unwrap();
        let exact = std::f64::consts::PI * 3.0 * 1.5;
        assert!((area(&e) - exact).abs() < 1e-2, "{}", area(&e));
    }
}
