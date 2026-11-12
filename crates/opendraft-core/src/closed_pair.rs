//! A closed polyline of two vertices, as AutoCAD draws a donut or a circle: two
//! arcs back to back. Here a closed polyline of two points is one segment
//! (`bulge::segment_count`), so files are read into the four-vertex form that
//! draws the same.

use crate::{bulge, Entity, Point};

/// `entity` with a closed two-vertex polyline that has arcs rewritten as four
/// vertices, each arc cut in half (a wide segment tapers across both halves);
/// anything else is returned as it was.
pub fn expanded(entity: Entity) -> Entity {
    let Entity::Polyline {
        points,
        closed: true,
        bulges,
        widths,
    } = &entity
    else {
        return entity;
    };
    let [a, b] = points.as_slice() else {
        return entity;
    };
    if !bulge::has_arcs(bulges) {
        return entity;
    }
    let (first, second) = (bulge::at(bulges, 0), bulge::at(bulges, 1));
    let halves = |from: Point, to: Point, b: f64, w: (f64, f64)| {
        let middle = bulge::midpoint(from, to, b);
        let across = (w.0 + w.1) / 2.0;
        (
            middle,
            bulge::part(b, 0.0, 0.5),
            (w.0, across),
            (across, w.1),
        )
    };
    let flat = (0.0, 0.0);
    let wide = |i: usize| widths.get(i).copied().unwrap_or(flat);
    let (m1, h1, w1a, w1b) = halves(*a, *b, first, wide(0));
    let (m2, h2, w2a, w2b) = halves(*b, *a, second, wide(1));
    Entity::Polyline {
        points: vec![*a, m1, *b, m2],
        closed: true,
        bulges: vec![h1, h1, h2, h2],
        widths: if widths.is_empty() {
            Vec::new()
        } else {
            vec![w1a, w1b, w2a, w2b]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn ring(widths: Vec<(f64, f64)>) -> Entity {
        Entity::Polyline {
            points: vec![p(1.0, 0.0), p(-1.0, 0.0)],
            closed: true,
            bulges: vec![1.0, 1.0],
            widths,
        }
    }

    #[test]
    fn two_half_circles_become_four_quarters_with_the_same_outline() {
        let Entity::Polyline { points, bulges, .. } = expanded(ring(Vec::new())) else {
            panic!("a polyline");
        };
        assert_eq!(points.len(), 4);
        let length = bulge::path_length(&points, &bulges, true);
        assert!((length - std::f64::consts::TAU).abs() < 1e-9, "{length}");
        let area = bulge::path_area(&points, &bulges).abs();
        assert!((area - std::f64::consts::PI).abs() < 1e-9, "{area}");
    }

    #[test]
    fn widths_carry_over_and_taper_across_the_halves() {
        let Entity::Polyline { widths, .. } = expanded(ring(vec![(2.0, 4.0), (1.0, 1.0)])) else {
            panic!("a polyline");
        };
        assert_eq!(widths, [(2.0, 3.0), (3.0, 4.0), (1.0, 1.0), (1.0, 1.0)]);
    }

    #[test]
    fn other_polylines_are_left_alone() {
        let straight = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(expanded(straight.clone()), straight);
        let open = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0)],
            closed: false,
            bulges: vec![1.0, 0.0],
            widths: Vec::new(),
        };
        assert_eq!(expanded(open.clone()), open);
    }
}
