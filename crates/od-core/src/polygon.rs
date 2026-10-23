//! Regular polygons, as AutoCAD's POLYGON makes them: about a centre,
//! inscribed in or circumscribed about a circle, or on an edge. Each function
//! returns the vertices counter-clockwise; the command makes a closed
//! polyline of them.

use std::f64::consts::{PI, TAU};

use crate::Point;

/// The fewest and most sides POLYGON accepts.
pub const MIN_SIDES: usize = 3;
pub const MAX_SIDES: usize = 1024;

/// How the circle given about the centre relates to the polygon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// The circle passes through the vertices; a point given is a vertex.
    Inscribed,
    /// The circle touches the middle of each edge; a point given is the
    /// middle of an edge.
    Circumscribed,
}

fn sides_ok(sides: usize) -> bool {
    (MIN_SIDES..=MAX_SIDES).contains(&sides)
}

/// `sides` vertices on a circle of `radius` about `center`, the first at
/// `start` radians.
fn around(center: Point, radius: f64, start: f64, sides: usize) -> Vec<Point> {
    (0..sides)
        .map(|k| {
            let a = start + TAU * k as f64 / sides as f64;
            Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect()
}

/// The polygon about `center` whose size and turn come from `at`: a vertex
/// when inscribed, the middle of an edge when circumscribed. `None` for a
/// side count out of range or `at` on the centre.
pub fn about_point(center: Point, at: Point, sides: usize, fit: Fit) -> Option<Vec<Point>> {
    let (dx, dy) = (at.x - center.x, at.y - center.y);
    let distance = dx.hypot(dy);
    if !sides_ok(sides) || !distance.is_finite() || distance <= 1e-12 {
        return None;
    }
    let angle = dy.atan2(dx);
    let half = PI / sides as f64;
    Some(match fit {
        Fit::Inscribed => around(center, distance, angle, sides),
        Fit::Circumscribed => around(center, distance / half.cos(), angle - half, sides),
    })
}

/// The polygon about `center` for a typed `radius`, turned as AutoCAD turns
/// it: with its bottom edge level.
pub fn about_radius(center: Point, radius: f64, sides: usize, fit: Fit) -> Option<Vec<Point>> {
    if !radius.is_finite() || radius <= 0.0 || !sides_ok(sides) {
        return None;
    }
    // The bottom edge's middle is straight below the centre.
    let down = -PI / 2.0;
    let at = match fit {
        Fit::Inscribed => down + PI / sides as f64,
        Fit::Circumscribed => down,
    };
    let point = Point::new(center.x + radius * at.cos(), center.y + radius * at.sin());
    about_point(center, point, sides, fit)
}

/// The polygon with the edge from `first` to `second`, lying to the left of
/// it (counter-clockwise), as POLYGON's Edge option makes it.
pub fn on_edge(first: Point, second: Point, sides: usize) -> Option<Vec<Point>> {
    let (dx, dy) = (second.x - first.x, second.y - first.y);
    let length = dx.hypot(dy);
    if !sides_ok(sides) || !length.is_finite() || length <= 1e-12 {
        return None;
    }
    let start = dy.atan2(dx);
    let mut points = Vec::with_capacity(sides);
    let mut at = first;
    for k in 0..sides {
        points.push(at);
        let a = start + TAU * k as f64 / sides as f64;
        at = Point::new(at.x + length * a.cos(), at.y + length * a.sin());
    }
    Some(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Twice the signed area: positive when counter-clockwise.
    fn area2(points: &[Point]) -> f64 {
        let n = points.len();
        (0..n)
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum()
    }

    #[test]
    fn an_inscribed_polygon_has_its_vertex_at_the_point() {
        let square = about_point(p(0.0, 0.0), p(2.0, 0.0), 4, Fit::Inscribed).unwrap();
        assert_eq!(square.len(), 4);
        assert!(close(square[0], p(2.0, 0.0)));
        assert!(close(square[1], p(0.0, 2.0)));
        assert!(area2(&square) > 0.0, "counter-clockwise");
    }

    #[test]
    fn a_circumscribed_polygon_has_an_edge_middle_at_the_point() {
        let square = about_point(p(0.0, 0.0), p(2.0, 0.0), 4, Fit::Circumscribed).unwrap();
        assert!(close(square[0], p(2.0, -2.0)));
        assert!(close(square[1], p(2.0, 2.0)));
        let hexagon = about_point(p(1.0, 1.0), p(1.0, 4.0), 6, Fit::Circumscribed).unwrap();
        for i in 0..6 {
            let (a, b) = (hexagon[i], hexagon[(i + 1) % 6]);
            let mid = p((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            assert!(((mid.x - 1.0).hypot(mid.y - 1.0) - 3.0).abs() < 1e-9);
        }
    }

    #[test]
    fn a_typed_radius_leaves_the_bottom_edge_level() {
        for fit in [Fit::Inscribed, Fit::Circumscribed] {
            for sides in [3, 4, 5, 6, 7] {
                let points = about_radius(p(0.0, 0.0), 5.0, sides, fit).unwrap();
                let lowest = points.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
                let bottom = points.iter().filter(|q| (q.y - lowest).abs() < 1e-9);
                assert_eq!(bottom.count(), 2, "{sides} sides {fit:?}");
            }
        }
        let inscribed = about_radius(p(0.0, 0.0), 5.0, 6, Fit::Inscribed).unwrap();
        assert!(inscribed
            .iter()
            .all(|q| (q.x.hypot(q.y) - 5.0).abs() < 1e-9));
        let square = about_radius(p(0.0, 0.0), 5.0, 4, Fit::Circumscribed).unwrap();
        assert!(close(square[0], p(-5.0, -5.0)));
        assert!(close(square[1], p(5.0, -5.0)));
    }

    #[test]
    fn an_edge_polygon_starts_on_the_edge_and_lies_to_its_left() {
        let triangle = on_edge(p(0.0, 0.0), p(4.0, 0.0), 3).unwrap();
        assert!(close(triangle[0], p(0.0, 0.0)));
        assert!(close(triangle[1], p(4.0, 0.0)));
        assert!(close(triangle[2], p(2.0, 12.0_f64.sqrt())));
        assert!(area2(&triangle) > 0.0);
        let hexagon = on_edge(p(1.0, 1.0), p(1.0, 3.0), 6).unwrap();
        for i in 0..6 {
            let (a, b) = (hexagon[i], hexagon[(i + 1) % 6]);
            assert!(((b.x - a.x).hypot(b.y - a.y) - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn bad_input_makes_no_polygon() {
        assert!(about_point(p(0.0, 0.0), p(0.0, 0.0), 5, Fit::Inscribed).is_none());
        assert!(about_point(p(0.0, 0.0), p(1.0, 0.0), 2, Fit::Inscribed).is_none());
        assert!(about_radius(p(0.0, 0.0), 1.0, MAX_SIDES + 1, Fit::Inscribed).is_none());
        assert!(about_radius(p(0.0, 0.0), -1.0, 5, Fit::Inscribed).is_none());
        assert!(on_edge(p(1.0, 1.0), p(1.0, 1.0), 5).is_none());
        assert!(about_radius(p(0.0, 0.0), 1.0, MAX_SIDES, Fit::Circumscribed).is_some());
    }
}
