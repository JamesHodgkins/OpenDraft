use std::f64::consts::PI;

use super::testing::*;

fn line(a: Point, b: Point) -> Entity {
    Entity::Line { start: a, end: b }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Entity {
    Entity::Rectangle {
        min: p(x0, y0),
        max: p(x1, y1),
    }
}

fn area_of(t: &Traced) -> f64 {
    solid(t.loops.clone(), HatchStyle::Normal)
        .area(1e-4)
        .unwrap()
}

const TOL: f64 = 1e-4;

#[test]
fn four_separate_lines_make_a_square_and_a_rectangle_is_exact() {
    let lines = [
        line(p(0.0, 0.0), p(10.0, 0.0)),
        line(p(10.0, 0.0), p(10.0, 10.0)),
        line(p(10.0, 10.0), p(0.0, 10.0)),
        line(p(0.0, 10.0), p(0.0, 0.0)),
    ];
    let t = trace(&lines, p(5.0, 5.0), TOL).unwrap();
    assert_eq!(t.loops.len(), 1);
    assert!((area_of(&t) - 100.0).abs() < 1e-9);
    assert_eq!(t.outer_sources, [0, 1, 2, 3]);
    // One whole closed object comes back as itself, not as a polygon.
    let t = trace(&[rect(0.0, 0.0, 10.0, 10.0)], p(5.0, 5.0), TOL).unwrap();
    assert!(matches!(&t.loops[0].boundary, HatchBoundary::Polyline(pl) if pl.vertices.len() == 4));
    let circle = Entity::Circle {
        center: p(0.0, 0.0),
        radius: 5.0,
    };
    let t = trace(&[circle], p(1.0, 1.0), TOL).unwrap();
    assert!(matches!(t.loops[0].boundary, HatchBoundary::Edges(_)));
    assert!((area_of(&t) - 25.0 * PI).abs() < 0.05);
}

#[test]
fn crossing_rectangles_give_the_piece_under_the_point() {
    let two = [rect(0.0, 0.0, 10.0, 10.0), rect(5.0, 5.0, 15.0, 15.0)];
    // The overlap, the two L-shaped remainders.
    let overlap = trace(&two, p(7.0, 7.0), TOL).unwrap();
    assert!((area_of(&overlap) - 25.0).abs() < 1e-9);
    let first_only = trace(&two, p(2.0, 2.0), TOL).unwrap();
    assert!((area_of(&first_only) - 75.0).abs() < 1e-9);
    assert!(first_only.outer_sources.contains(&0) && first_only.outer_sources.contains(&1));
    assert_eq!(
        trace(&two, p(20.0, 20.0), TOL).unwrap_err(),
        TraceError::NoBoundary
    );
}

#[test]
fn objects_inside_are_islands_and_touching_ones_are_not() {
    let objects = [
        rect(0.0, 0.0, 20.0, 20.0),
        Entity::Circle {
            center: p(10.0, 10.0),
            radius: 3.0,
        },
        // A loose line inside, and one that touches the wall.
        line(p(2.0, 2.0), p(4.0, 2.0)),
        line(p(0.0, 15.0), p(5.0, 15.0)),
    ];
    let t = trace(&objects, p(1.0, 10.0), TOL).unwrap();
    assert_eq!(t.loops.len(), 2, "the circle only");
    assert!((area_of(&t) - (400.0 - 9.0 * PI)).abs() < 0.1);
    // Inside the circle: its own loop alone, no island.
    let inner = trace(&objects, p(10.0, 10.0), TOL).unwrap();
    assert_eq!(inner.loops.len(), 1);
    assert!((area_of(&inner) - 9.0 * PI).abs() < 0.1);
}

#[test]
fn a_gap_a_dead_end_and_a_tiny_scale_are_handled() {
    // A square with a gap in one side is not a boundary.
    let open = [
        line(p(0.0, 0.0), p(10.0, 0.0)),
        line(p(10.0, 0.0), p(10.0, 4.0)),
        line(p(10.0, 6.0), p(10.0, 10.0)),
        line(p(10.0, 10.0), p(0.0, 10.0)),
        line(p(0.0, 10.0), p(0.0, 0.0)),
    ];
    assert_eq!(
        trace(&open, p(5.0, 5.0), TOL).unwrap_err(),
        TraceError::NoBoundary
    );
    // A spur poking into a closed square does not change its boundary.
    let mut spur = vec![rect(0.0, 0.0, 10.0, 10.0), line(p(10.0, 5.0), p(7.0, 5.0))];
    let t = trace(&spur, p(2.0, 2.0), TOL).unwrap();
    assert!((area_of(&t) - 100.0).abs() < 1e-9);
    spur.clear();
    assert_eq!(
        trace(&spur, p(0.0, 0.0), TOL).unwrap_err(),
        TraceError::NoBoundary
    );
    // The same square a millionth the size.
    let small = [rect(0.0, 0.0, 1e-6, 1e-6)];
    let t = trace(&small, p(5e-7, 5e-7), 1e-12).unwrap();
    assert!((area_of(&t) - 1e-12).abs() < 1e-20);
}

#[test]
fn a_bulged_polyline_and_a_spline_are_followed_by_their_curves() {
    let d = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
        closed: false,
        bulges: vec![0.0, 1.0, 0.0, 0.0],
        widths: Vec::new(),
    };
    // Open: the half circle on the right plus the three sides, closed by a
    // line along the left.
    let wall = line(p(0.0, 2.0), p(0.0, 0.0));
    let t = trace(&[d, wall], p(1.0, 1.0), 1e-4).unwrap();
    assert!(
        (area_of(&t) - (4.0 + PI / 2.0)).abs() < 0.01,
        "{}",
        area_of(&t)
    );
}
