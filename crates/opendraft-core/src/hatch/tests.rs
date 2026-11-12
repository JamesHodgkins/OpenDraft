use std::f64::consts::{PI, TAU};

use super::testing::*;

#[test]
fn islands_follow_the_three_styles_at_every_depth() {
    // Areas of squares 20, 18, 16, 14, 12 on a side.
    let a = |side: f64| side * side;
    let sides = [20.0, 18.0, 16.0, 14.0, 12.0];
    for depth in 1..=5 {
        let normal = solid(nested(depth), HatchStyle::Normal);
        let mut expect = 0.0;
        for (i, s) in sides.iter().take(depth).enumerate() {
            expect += if i % 2 == 0 { a(*s) } else { -a(*s) };
        }
        assert!((eval(&normal).area - expect).abs() < 1e-9, "normal {depth}");
        let outer = solid(nested(depth), HatchStyle::Outer);
        let expect = if depth == 1 {
            a(20.0)
        } else {
            a(20.0) - a(18.0)
        };
        assert!((eval(&outer).area - expect).abs() < 1e-9, "outer {depth}");
        let ignore = solid(nested(depth), HatchStyle::Ignore);
        assert!(
            (eval(&ignore).area - a(20.0)).abs() < 1e-9,
            "ignore {depth}"
        );
    }
}

#[test]
fn winding_does_not_decide_what_is_a_hole() {
    // The island runs the same way as the outer loop, and still is a hole.
    let h = solid(nested(2), HatchStyle::Normal);
    assert!(h.contains(p(0.5, 10.0), TOL), "between the two");
    assert!(!h.contains(p(10.0, 10.0), TOL), "inside the island");
}

#[test]
fn disjoint_outer_loops_are_all_filled() {
    let h = solid(
        vec![square(0.0, 0.0, 10.0), square(30.0, 0.0, 10.0)],
        HatchStyle::Outer,
    );
    assert!((eval(&h).area - 200.0).abs() < 1e-9);
}

#[test]
fn a_loop_with_no_area_is_left_out() {
    let flat = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Polyline(HatchPolyline {
            vertices: vec![p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)],
            bulges: Vec::new(),
            closed: true,
        }),
        sources: Vec::new(),
    };
    let h = solid(vec![square(0.0, 0.0, 10.0), flat], HatchStyle::Normal);
    assert!((eval(&h).area - 100.0).abs() < 1e-9);
}

#[test]
fn circles_and_arc_edges_have_their_true_area() {
    let circle = |ccw: bool| HatchLoop {
        flags: HatchLoopFlags(HatchLoopFlags::EXTERNAL),
        boundary: HatchBoundary::Edges(vec![HatchEdge::CircularArc {
            center: p(5.0, 5.0),
            radius: 5.0,
            start_angle: 0.0,
            end_angle: TAU,
            counter_clockwise: ccw,
        }]),
        sources: Vec::new(),
    };
    for ccw in [true, false] {
        let area = eval(&solid(vec![circle(ccw)], HatchStyle::Normal)).area;
        assert!((area - 25.0 * PI).abs() < 0.05, "{ccw}: {area}");
    }
    // A half disc: an arc and the chord, the arc run clockwise.
    let half = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Edges(vec![
            HatchEdge::CircularArc {
                center: p(0.0, 0.0),
                radius: 5.0,
                start_angle: PI,
                end_angle: 0.0,
                counter_clockwise: false,
            },
            HatchEdge::Line {
                start: p(5.0, 0.0),
                end: p(-5.0, 0.0),
            },
        ]),
        sources: Vec::new(),
    };
    let area = eval(&solid(vec![half], HatchStyle::Normal)).area;
    assert!((area - 12.5 * PI).abs() < 0.05, "{area}");
}

#[test]
fn an_elliptic_edge_and_a_spline_edge_are_followed() {
    let ellipse = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Edges(vec![HatchEdge::EllipticArc {
            center: p(0.0, 0.0),
            major_axis: p(6.0, 0.0),
            minor_major_ratio: 0.5,
            start_param: 0.0,
            end_param: TAU,
            counter_clockwise: true,
        }]),
        sources: Vec::new(),
    };
    let area = eval(&solid(vec![ellipse], HatchStyle::Normal)).area;
    assert!((area - PI * 6.0 * 3.0).abs() < 0.05, "{area}");
    // The cubic of the HatchTest drawing: eight control points, closed by
    // repeating the first.
    let spline = HatchEdge::Spline(HatchSpline {
        degree: 3,
        periodic: false,
        knots: vec![
            0.0, 0.0, 0.0, 0.0, 5.0, 15.0, 20.0, 27.07, 34.14, 34.14, 34.14, 34.14,
        ],
        control_points: vec![
            p(45.0, 5.0),
            p(44.29, 6.82),
            p(43.67, 12.54),
            p(56.33, 12.54),
            p(55.41, 4.13),
            p(50.0, -2.42),
            p(46.0, 2.42),
            p(45.0, 5.0),
        ],
        weights: Vec::new(),
        fit_points: Vec::new(),
        start_tangent: None,
        end_tangent: None,
    });
    let l = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Edges(vec![spline]),
        sources: Vec::new(),
    };
    let result = eval(&solid(vec![l], HatchStyle::Normal));
    assert!(result.area > 40.0 && result.area < 160.0, "{}", result.area);
}

#[test]
fn an_open_loop_is_an_error_not_a_guess() {
    let open = HatchLoop {
        flags: HatchLoopFlags::default(),
        boundary: HatchBoundary::Edges(vec![
            HatchEdge::Line {
                start: p(0.0, 0.0),
                end: p(10.0, 0.0),
            },
            HatchEdge::Line {
                start: p(10.0, 0.0),
                end: p(10.0, 10.0),
            },
        ]),
        sources: Vec::new(),
    };
    let h = solid(vec![open], HatchStyle::Normal);
    assert_eq!(
        h.evaluate(TOL, &HatchLimits::default()),
        Err(HatchError::OpenLoop(0))
    );
}

#[test]
fn objects_make_loops_and_open_pieces_chain_up() {
    let line = |a: Point, b: Point| Entity::Line { start: a, end: b };
    // Four lines in a scrambled order and directions.
    let pieces = [
        line(p(10.0, 0.0), p(10.0, 10.0)),
        line(p(0.0, 0.0), p(10.0, 0.0)),
        line(p(0.0, 10.0), p(10.0, 10.0)),
        line(p(0.0, 10.0), p(0.0, 0.0)),
        Entity::Circle {
            center: p(50.0, 50.0),
            radius: 3.0,
        },
        // A hatch cannot bound a hatch.
        Entity::Hatch(Box::new(solid(
            vec![square(0.0, 0.0, 1.0)],
            HatchStyle::Normal,
        ))),
    ];
    let (loops, ignored) = loops_from_entities(&pieces, 1e-6).unwrap();
    assert_eq!((loops.len(), ignored), (2, 1));
    let h = solid(loops, HatchStyle::Normal);
    assert!((eval(&h).area - (100.0 + 9.0 * PI)).abs() < 0.1);
    // A gap in the chain.
    let open = [
        line(p(0.0, 0.0), p(10.0, 0.0)),
        line(p(10.0, 0.0), p(10.0, 10.0)),
    ];
    assert_eq!(loops_from_entities(&open, 1e-6), Err(BuildError::Open(2)));
    assert_eq!(loops_from_entities(&[], 1e-6), Err(BuildError::Nothing));
}

#[test]
fn a_polyline_with_arcs_and_a_rectangle_make_polyline_loops() {
    let rect = Entity::Rectangle {
        min: p(0.0, 0.0),
        max: p(4.0, 3.0),
    };
    let (loops, _) = loops_from_entities(&[rect], 1e-6).unwrap();
    assert!(loops[0].flags.contains(HatchLoopFlags::POLYLINE));
    let rounded = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)],
        closed: true,
        bulges: vec![0.0, 1.0, 0.0, 0.0],
        widths: Vec::new(),
    };
    let (loops, _) = loops_from_entities(&[rounded], 1e-6).unwrap();
    let area = eval(&solid(loops, HatchStyle::Normal)).area;
    // A 2x2 square with a half disc of radius 1 on the right side.
    assert!((area - (4.0 + PI / 2.0)).abs() < 0.01, "{area}");
}

#[test]
fn validation_refuses_what_could_not_exist() {
    let mut h = solid(vec![square(0.0, 0.0, 1.0)], HatchStyle::Normal);
    assert_eq!(h.validate(), Ok(()));
    h.elevation = f64::NAN;
    assert_eq!(h.validate(), Err(HatchError::NonFinite));
    let mut h = solid(vec![square(0.0, 0.0, 1.0)], HatchStyle::Normal);
    if let HatchBoundary::Polyline(pl) = &mut h.loops[0].boundary {
        pl.bulges = vec![0.0];
    }
    assert!(matches!(h.validate(), Err(HatchError::BadLoop(0, _))));
    let many = vec![square(0.0, 0.0, 1.0); MAX_LOOPS + 1];
    assert!(matches!(
        solid(many, HatchStyle::Normal).validate(),
        Err(HatchError::TooLarge(_))
    ));
}

#[test]
fn the_hit_helpers_answer_from_the_boundary_alone() {
    let h = solid(nested(2), HatchStyle::Normal);
    assert!(h.contains(p(0.5, 10.0), TOL));
    assert!(!h.contains(p(10.0, 10.0), TOL));
    assert!(!h.contains(p(30.0, 10.0), TOL));
    let d = h.distance_to_boundary(p(10.0, 10.0), TOL).unwrap();
    assert!((d - 9.0).abs() < 1e-9, "{d}");
    assert_eq!(h.bounds().unwrap().max, p(20.0, 20.0));
}

#[test]
fn a_circle_stored_as_two_bulge_one_vertices_is_a_whole_circle() {
    // How AutoCAD writes a circle boundary (HatchTest.dxf).
    let circle = HatchLoop {
        flags: HatchLoopFlags(7),
        boundary: HatchBoundary::Polyline(HatchPolyline {
            vertices: vec![p(15.0, 5.0), p(25.0, 5.0)],
            bulges: vec![1.0, 1.0],
            closed: true,
        }),
        sources: Vec::new(),
    };
    let h = solid(vec![circle], HatchStyle::Normal);
    assert!((eval(&h).area - 25.0 * PI).abs() < 0.05);
    let b = h.bounds().unwrap();
    assert!((b.min.y - 0.0).abs() < 1e-9 && (b.max.y - 10.0).abs() < 1e-9);
}
