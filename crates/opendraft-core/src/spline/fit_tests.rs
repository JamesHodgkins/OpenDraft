//! Fitting, grips and lengths.

use std::f64::consts::PI;

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: Point, b: Point, tolerance: f64) -> bool {
    distance(a, b) <= tolerance
}

#[test]
fn two_fit_points_make_a_straight_cubic_with_control_points_at_thirds() {
    let s = Spline::from_fit_points(vec![p(0.0, 0.0), p(3.0, 6.0)], KnotParameterization::Chord)
        .unwrap();
    assert_eq!(s.degree, 3);
    let length = 45.0f64.sqrt();
    assert_eq!(
        s.knots,
        [0.0, 0.0, 0.0, 0.0, length, length, length, length]
    );
    let expected = [p(0.0, 0.0), p(1.0, 2.0), p(2.0, 4.0), p(3.0, 6.0)];
    for (got, want) in s.control_points.iter().zip(expected) {
        assert!(near(*got, want, 1e-12), "{got:?}");
    }
    let fit = s.fit.as_ref().unwrap();
    assert_eq!(fit.points, [p(0.0, 0.0), p(3.0, 6.0)]);
    assert_eq!(fit.tolerance, 0.0);
}

#[test]
fn a_fit_spline_passes_through_its_points_with_natural_ends() {
    let points = vec![
        p(0.0, 0.0),
        p(2.0, 3.0),
        p(5.0, 1.0),
        p(7.0, 4.0),
        p(9.0, 0.0),
    ];
    let mut shapes = Vec::new();
    for how in [
        KnotParameterization::Chord,
        KnotParameterization::SqrtChord,
        KnotParameterization::Uniform,
    ] {
        let s = Spline::from_fit_points(points.clone(), how).unwrap();
        assert_eq!(s.control_points.len(), points.len() + 2);
        let (lo, hi) = s.domain();
        // Through every point, in order.
        let mut last = lo;
        for q in &points {
            let (on, u) = s.nearest(*q);
            assert!(near(on, *q, 1e-9), "{how:?} {q:?}");
            assert!(u >= last - 1e-12);
            last = u;
        }
        // No curvature at either end: the derivative stops changing.
        for (u, step) in [(lo, 1e-7), (hi, -1e-7)] {
            let (a, b) = (s.derivative_at(u), s.derivative_at(u + step));
            let second = p((b.x - a.x) / step, (b.y - a.y) / step);
            assert!(second.x.hypot(second.y) < 1e-2, "{how:?} {second:?}");
        }
        shapes.push(s.point_at(0.3));
    }
    assert!(distance(shapes[0], shapes[1]) > 1e-3 && distance(shapes[1], shapes[2]) > 1e-3);
}

#[test]
fn three_fit_points_stay_cubic_and_repeats_are_refused() {
    let s = Spline::from_fit_points(
        vec![p(0.0, 0.0), p(1.0, 1.0), p(2.0, 0.0)],
        KnotParameterization::Chord,
    )
    .unwrap();
    assert_eq!((s.degree, s.control_points.len()), (3, 5));
    assert!(near(s.point_at(s.knots[4]), p(1.0, 1.0), 1e-12));
    assert_eq!(
        Spline::from_fit_points(
            vec![p(0.0, 0.0), p(1.0, 1.0), p(1.0, 1.0)],
            KnotParameterization::Chord
        ),
        Err(SplineError::RepeatedPoint(2))
    );
    assert_eq!(
        Spline::from_fit_points(vec![p(0.0, 0.0)], KnotParameterization::Chord),
        Err(SplineError::BadFitData)
    );
    // A long run of points solves quickly and exactly.
    let many: Vec<Point> = (0..5000)
        .map(|i| p(f64::from(i), (f64::from(i) * 0.37).sin() * 10.0))
        .collect();
    let s = Spline::from_fit_points(many.clone(), KnotParameterization::SqrtChord).unwrap();
    assert!(near(s.point_at(s.knots[2000 + 3]), many[2000], 1e-9));
}

#[test]
fn a_fit_grip_refits_and_a_control_point_grip_drops_the_fit() {
    let points = vec![p(0.0, 0.0), p(2.0, 3.0), p(5.0, 1.0), p(7.0, 4.0)];
    let s = Spline::from_fit_points(points.clone(), KnotParameterization::Chord).unwrap();
    let grips = s.grips();
    assert_eq!(grips.len(), 4, "one per fit point");
    assert_eq!(grips[1].at, p(2.0, 3.0));
    let moved = s.with_grip_moved(1, p(2.0, 5.0)).unwrap();
    assert!(moved.fit.is_some());
    assert!(near(moved.nearest(p(2.0, 5.0)).0, p(2.0, 5.0), 1e-9));
    // Moving a fit point onto its neighbour leaves no curve.
    assert_eq!(s.with_grip_moved(1, p(0.0, 0.0)), None);
    // Without fit data the grips are the control points.
    let mut cv = s.clone();
    cv.fit = None;
    assert_eq!(cv.grips().len(), 6);
    let moved = cv.with_grip_moved(2, p(10.0, 10.0)).unwrap();
    assert_eq!(moved.control_points[2], p(10.0, 10.0));
    assert_eq!(moved.knots, cv.knots);
    // Stretching every point moves the curve and keeps its fit data.
    let all = s.stretched(|_| true, 1.0, 2.0).unwrap();
    assert_eq!(all, s.translated(1.0, 2.0));
    assert!(all.fit.is_some());
    assert_eq!(s.stretched(|_| false, 1.0, 2.0), None);
    // A stretch of some control points of a fit spline that cannot be
    // refitted drops the fit data.
    let mut tangent = s.clone();
    if let Some(fit) = &mut tangent.fit {
        fit.start_tangent = Some(p(1.0, 0.0));
    }
    let moved = tangent.stretched(|q| q.x > 6.0, 1.0, 0.0).unwrap();
    assert!(moved.fit.is_none());
}

#[test]
fn a_closed_loop_shows_its_shared_control_point_once_and_moves_it_whole() {
    let s = Spline::new_nurbs(
        2,
        vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0],
        None,
        true,
        false,
    )
    .unwrap();
    assert_eq!(s.grips().len(), 4);
    let moved = s.with_grip_moved(0, p(-1.0, -1.0)).unwrap();
    assert!(moved.closed);
    assert_eq!(moved.control_points[0], p(-1.0, -1.0));
    assert_eq!(moved.control_points[4], p(-1.0, -1.0));
}

#[test]
fn length_and_midpoint_of_a_half_circle_made_of_two_quarters() {
    // Degree 2, a double knot in the middle: two rational quarters.
    let w = 0.5f64.sqrt();
    let s = Spline::new_nurbs(
        2,
        vec![
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
            p(-1.0, 1.0),
            p(-1.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
        Some(vec![1.0, w, 1.0, w, 1.0]),
        false,
        false,
    )
    .unwrap();
    assert!((s.length().unwrap() - PI).abs() < 1e-9);
    assert!(near(s.mid_point().unwrap(), p(0.0, 1.0), 1e-9));
    assert!(near(s.point_at_length(PI / 4.0).unwrap(), p(w, w), 1e-9));
    let b = s.bounds();
    assert_eq!((b.min, b.max), (p(-1.0, 0.0), p(1.0, 1.0)));
}

/// AutoCAD's own closed fits of the square (5,10) (10,10) (10,5) (5,5), as
/// "Spline Test 2.dxf" stores them: square-root and uniform knots.
#[test]
fn closed_fits_match_the_control_points_autocad_stored() {
    let square = |x: f64, y: f64| vec![p(x, y + 5.0), p(x + 5.0, y + 5.0), p(x + 5.0, y), p(x, y)];
    let want = |x: f64, y: f64, a: f64| {
        vec![
            p(x, y + 5.0),
            p(x + a, y + 5.0 + a),
            p(x + 5.0 + a, y + 5.0 + a),
            p(x + 5.0 + a, y - a),
            p(x - a, y - a),
            p(x - a, y + 5.0 - a),
            p(x, y + 5.0),
        ]
    };
    let root5 = 5.0f64.sqrt();
    for (how, x, a, steps) in [
        (KnotParameterization::SqrtChord, 5.0, 1.25, root5),
        (KnotParameterization::Uniform, 15.0, 1.25, 1.0),
    ] {
        let s = Spline::from_closed_fit_points(square(x, 5.0), how).unwrap();
        assert!(s.closed);
        assert_eq!(s.control_points.len(), 7);
        for (got, want) in s.control_points.iter().zip(want(x, 5.0, a)) {
            assert!(near(*got, want, 1e-9), "{how:?} {got:?} {want:?}");
        }
        for (i, k) in s.knots[4..8].iter().enumerate() {
            assert!((k - steps * (i + 1) as f64).abs() < 1e-9, "{how:?} {k}");
        }
        // Smooth where it closes.
        let n = s.knots.len();
        let (a, b) = (s.derivative_at(s.knots[3]), s.derivative_at(s.knots[n - 4]));
        assert!(near(a, b, 1e-9));
    }
}

#[test]
fn a_long_closed_fit_passes_through_its_points_and_closes_smoothly() {
    let many: Vec<Point> = (0..3000)
        .map(|i| {
            let t = f64::from(i) / 3000.0 * std::f64::consts::TAU;
            p(
                100.0 * t.cos() * (1.0 + 0.3 * (3.0 * t).sin()),
                80.0 * t.sin(),
            )
        })
        .collect();
    let s = Spline::from_closed_fit_points(many.clone(), KnotParameterization::Chord).unwrap();
    assert_eq!(s.control_points.len(), 3003);
    for (i, q) in many.iter().enumerate() {
        assert!(near(s.point_at(s.knots[i + 3]), *q, 1e-8), "{i}");
    }
    let n = s.knots.len();
    let (a, b) = (s.derivative_at(s.knots[3]), s.derivative_at(s.knots[n - 4]));
    assert!(near(a, b, 1e-6), "{a:?} {b:?}");
}

#[test]
fn closed_fits_refuse_too_few_or_repeated_points() {
    let how = KnotParameterization::Chord;
    assert_eq!(
        Spline::from_closed_fit_points(vec![p(0.0, 0.0), p(1.0, 0.0)], how),
        Err(SplineError::BadFitData)
    );
    assert_eq!(
        Spline::from_closed_fit_points(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)], how),
        Err(SplineError::RepeatedPoint(3))
    );
    // Three points make a closed curve.
    let s = Spline::from_closed_fit_points(vec![p(0.0, 0.0), p(4.0, 0.0), p(2.0, 3.0)], how);
    assert!(s.is_ok());
}
