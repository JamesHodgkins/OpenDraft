use std::f64::consts::FRAC_PI_2;

use super::*;
use crate::Affine;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: Point, b: Point, tolerance: f64) -> bool {
    distance(a, b) <= tolerance
}

/// The rational quarter of the unit circle from (1, 0) to (0, 1).
fn quarter() -> Spline {
    let w = 0.5f64.sqrt();
    Spline::new_nurbs(
        2,
        vec![p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        Some(vec![1.0, w, 1.0]),
        false,
        false,
    )
    .unwrap()
}

/// A small deterministic random source, so failures repeat.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }

    /// A random valid spline: degree 1 to 5, uneven knots (some repeated),
    /// rational about half the time.
    fn spline(&mut self) -> Spline {
        let degree = 1 + (self.next() * 5.0) as usize;
        let count = degree + 1 + (self.next() * 6.0) as usize;
        let points = (0..count)
            .map(|_| p(self.range(-50.0, 50.0), self.range(-50.0, 50.0)))
            .collect();
        let mut knots = vec![0.0; degree + 1];
        let mut at = 0.0;
        while knots.len() < count {
            at += if self.next() < 0.2 {
                0.0
            } else {
                self.range(0.1, 2.0)
            };
            let repeats = knots.iter().filter(|k| **k == at).count();
            knots.push(if repeats >= degree { at + 0.5 } else { at });
            at = knots[knots.len() - 1];
        }
        let end = at + self.range(0.1, 2.0);
        knots.extend(std::iter::repeat_n(end, degree + 1));
        let weights =
            (self.next() < 0.5).then(|| (0..count).map(|_| self.range(0.2, 5.0)).collect());
        Spline::new_nurbs(degree as u8, points, knots, weights, false, false).unwrap()
    }
}

#[test]
fn a_rational_quarter_circle_is_exactly_round() {
    let q = quarter();
    assert!(q.is_rational());
    for i in 0..=20 {
        let at = q.point_at(f64::from(i) / 20.0);
        assert!((at.x.hypot(at.y) - 1.0).abs() < 1e-12, "{at:?}");
    }
    assert_eq!(q.start_point(), p(1.0, 0.0));
    assert!(near(q.end_point(), p(0.0, 1.0), 1e-15));
    assert!((q.length().unwrap() - FRAC_PI_2).abs() < 1e-9);
    let mid = q.mid_point().unwrap();
    assert!(near(mid, p(0.5f64.sqrt(), 0.5f64.sqrt()), 1e-9), "{mid:?}");
    let outline = q.flatten(1e-4).unwrap();
    assert!(outline.len() > 4);
    for w in outline.windows(2) {
        let m = p((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
        assert!((m.x.hypot(m.y) - 1.0).abs() <= 1e-4 + 1e-12);
        assert!((w[0].x.hypot(w[0].y) - 1.0).abs() < 1e-12);
    }
}

#[test]
fn a_degree_one_spline_is_its_control_polygon() {
    let s = Spline::from_control_points(1, vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]).unwrap();
    assert_eq!(s.knots, [0.0, 0.0, 0.5, 1.0, 1.0]);
    assert!(near(s.point_at(0.5), p(4.0, 0.0), 1e-12));
    assert!(near(s.point_at(0.75), p(4.0, 1.5), 1e-12));
    assert!((s.length().unwrap() - 7.0).abs() < 1e-9);
    assert!(near(s.mid_point().unwrap(), p(3.5, 0.0), 1e-9));
    assert_eq!(
        s.flatten(1e-9).unwrap(),
        [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)]
    );
}

#[test]
fn a_clamped_spline_runs_from_its_first_control_point_to_its_last() {
    let mut rng = Rng(7);
    for _ in 0..200 {
        let s = rng.spline();
        assert!(near(s.start_point(), s.control_points[0], 1e-9));
        let last = *s.control_points.last().unwrap();
        assert!(near(s.end_point(), last, 1e-9));
    }
}

#[test]
fn bad_values_are_refused_with_the_reason() {
    let good = || {
        Spline::from_control_points(3, vec![p(0.0, 0.0), p(1.0, 1.0), p(2.0, 0.0), p(3.0, 1.0)])
            .unwrap()
    };
    let with = |f: &dyn Fn(&mut Spline)| {
        let mut s = good();
        f(&mut s);
        s.validate()
    };
    assert_eq!(good().validate(), Ok(()));
    assert_eq!(with(&|s| s.degree = 0), Err(SplineError::DegreeOutOfRange));
    assert_eq!(with(&|s| s.degree = 26), Err(SplineError::DegreeOutOfRange));
    assert_eq!(
        with(&|s| s.degree = 4),
        Err(SplineError::TooFewControlPoints)
    );
    assert_eq!(
        with(&|s| {
            s.knots.pop();
        }),
        Err(SplineError::WrongKnotCount)
    );
    assert_eq!(
        with(&|s| s.knots[4] = -1.0),
        Err(SplineError::KnotsDecrease)
    );
    assert_eq!(
        with(&|s| s.knots[1] = f64::NAN),
        Err(SplineError::NonFinite)
    );
    assert_eq!(
        with(&|s| s.control_points[1].x = f64::INFINITY),
        Err(SplineError::NonFinite)
    );
    assert_eq!(
        with(&|s| s.control_points[1].x = 2e12),
        Err(SplineError::CoordinateOutOfRange)
    );
    assert_eq!(
        with(&|s| s.knots = vec![0.0; 8]),
        Err(SplineError::KnotMultiplicity)
    );
    assert_eq!(
        with(&|s| s.weights = Some(vec![1.0; 3])),
        Err(SplineError::WrongWeightCount)
    );
    assert_eq!(
        with(&|s| s.weights = Some(vec![1.0, 0.0, 1.0, 1.0])),
        Err(SplineError::NonPositiveWeight)
    );
    assert_eq!(
        with(&|s| s.weights = Some(vec![1.0, 1e13, 1.0, 1.0])),
        Err(SplineError::WeightOutOfRange)
    );
    assert_eq!(
        with(&|s| s.periodic = true),
        Err(SplineError::PeriodicNotClosed)
    );
    assert_eq!(
        with(&|s| s.closed = true),
        Err(SplineError::ClosureMismatch)
    );
    assert_eq!(
        with(&|s| s.control_points = vec![p(1.0, 1.0); 4]),
        Err(SplineError::Degenerate)
    );
    assert_eq!(
        with(&|s| s.knot_tolerance = -1.0),
        Err(SplineError::NonFinite)
    );
    // An inner knot may repeat up to the degree, not past it.
    let mut five = Spline::from_control_points(
        2,
        vec![
            p(0.0, 0.0),
            p(1.0, 1.0),
            p(2.0, 0.0),
            p(3.0, 1.0),
            p(4.0, 0.0),
        ],
    )
    .unwrap();
    five.knots = vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0];
    assert_eq!(five.validate(), Ok(()));
    five.knots = vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0];
    assert_eq!(five.validate(), Err(SplineError::KnotMultiplicity));
    // Knots within the tolerance of each other count as one.
    five.knots = vec![0.0, 0.0, 0.0, 0.5, 0.5 + 1e-9, 0.5 + 2e-9, 1.0, 1.0];
    assert_eq!(five.validate(), Err(SplineError::KnotMultiplicity));
    // Fit data needs two points and real tangents.
    let mut fitted = good();
    fitted.fit = Some(SplineFitData::exact(
        vec![p(0.0, 0.0)],
        KnotParameterization::Chord,
    ));
    assert_eq!(fitted.validate(), Err(SplineError::BadFitData));
    let mut data =
        SplineFitData::exact(vec![p(0.0, 0.0), p(1.0, 0.0)], KnotParameterization::Chord);
    data.start_tangent = Some(p(0.0, 0.0));
    fitted.fit = Some(data);
    assert_eq!(fitted.validate(), Err(SplineError::ZeroTangent));
}

#[test]
fn weights_of_one_are_dropped_and_a_closed_loop_is_accepted() {
    let s = Spline::new_nurbs(
        1,
        vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(0.0, 0.0)],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        Some(vec![1.0; 4]),
        true,
        false,
    )
    .unwrap();
    assert_eq!(s.weights, None);
    assert!(s.closed);
}

#[test]
fn transforms_commute_with_evaluation() {
    let mut rng = Rng(11);
    let uneven = Affine {
        xx: 2.0,
        xy: 0.5,
        yx: -0.25,
        yy: 0.75,
        tx: 3.0,
        ty: -4.0,
    };
    let a = p(1.0, 2.0);
    let b = p(-3.0, 5.0);
    for _ in 0..100 {
        let s = rng.spline();
        let (lo, hi) = s.domain();
        type Map = Box<dyn Fn(Point) -> Point>;
        let cases: [(Spline, Map); 5] = [
            (
                s.translated(3.0, -2.0),
                Box::new(|q: Point| q.translated(3.0, -2.0)),
            ),
            (
                s.rotated(a, 0.7),
                Box::new(move |q: Point| q.rotated(a, 0.7)),
            ),
            (
                s.scaled(a, -1.5),
                Box::new(move |q: Point| q.scaled(a, -1.5)),
            ),
            (s.mirrored(a, b), Box::new(move |q: Point| q.mirrored(a, b))),
            (
                s.transformed(&uneven),
                Box::new(move |q: Point| uneven.apply(q)),
            ),
        ];
        for (moved, map) in &cases {
            assert_eq!(moved.validate(), Ok(()));
            for i in 0..=10 {
                let u = lo + (hi - lo) * f64::from(i) / 10.0;
                assert!(near(moved.point_at(u), map(s.point_at(u)), 1e-9));
            }
        }
    }
}

#[test]
fn derivatives_match_finite_differences() {
    let mut rng = Rng(3);
    for _ in 0..100 {
        let s = rng.spline();
        let (lo, hi) = s.domain();
        for i in 1..10 {
            let u = lo + (hi - lo) * (f64::from(i) / 10.0 + 0.013);
            // Away from knots, where the curve is smooth.
            if s.knots.iter().any(|k| (k - u).abs() < 1e-4) {
                continue;
            }
            let h = 1e-6;
            let (a, b) = (s.point_at(u - h), s.point_at(u + h));
            let fd = p((b.x - a.x) / (2.0 * h), (b.y - a.y) / (2.0 * h));
            let d = s.derivative_at(u);
            let scale = 1.0 + d.x.hypot(d.y);
            assert!(near(d, fd, 1e-4 * scale), "{d:?} {fd:?}");
        }
    }
}

#[test]
fn every_outline_point_is_on_the_curve_and_every_chord_close_to_it() {
    let mut rng = Rng(5);
    for _ in 0..50 {
        let s = rng.spline();
        let tolerance = 0.05;
        let points = s.flatten_params(tolerance, MAX_PLOT_POINTS).unwrap();
        for (u, q) in &points {
            assert!(near(s.point_at(*u), *q, 1e-9));
        }
        for w in points.windows(2) {
            for f in [0.25, 0.5, 0.75] {
                let u = w[0].0 + (w[1].0 - w[0].0) * f;
                let on = s.point_at(u);
                let off = flatten::to_segment(on, w[0].1, w[1].1);
                assert!(off <= tolerance + 1e-9, "{off}");
            }
        }
    }
    // Asking for more than allowed is refused, not met coarsely.
    let big =
        Spline::from_control_points(3, vec![p(0.0, 0.0), p(1e9, 1e9), p(2e9, -1e9), p(3e9, 0.0)])
            .unwrap();
    assert_eq!(big.flatten(1e-6), Err(SplineError::ResourceLimit));
    assert!(big.outline(1e-6).len() > 100);
}

#[test]
fn the_nearest_point_is_as_good_as_a_dense_search() {
    let mut rng = Rng(13);
    for _ in 0..40 {
        let s = rng.spline();
        let (lo, hi) = s.domain();
        let dense: Vec<Point> = (0..=100_000)
            .map(|i| s.point_at(lo + (hi - lo) * f64::from(i) / 100_000.0))
            .collect();
        for _ in 0..5 {
            let q = p(rng.range(-60.0, 60.0), rng.range(-60.0, 60.0));
            let (found, u) = s.nearest(q);
            assert!(near(s.point_at(u), found, 1e-9));
            let best = dense
                .iter()
                .map(|d| distance(*d, q))
                .fold(f64::INFINITY, f64::min);
            assert!(
                distance(found, q) <= best + 1e-6,
                "{} vs {best}",
                distance(found, q)
            );
        }
    }
}

#[test]
fn a_bezier_piece_of_each_span_traces_the_curve() {
    let mut rng = Rng(17);
    for _ in 0..50 {
        let s = rng.spline();
        for k in s.spans() {
            let (u0, u1) = (s.knots[k], s.knots[k + 1]);
            let c = s.bezier(k);
            for t in [0.0, 0.3, 0.7, 1.0] {
                let (h, _) = eval::bezier_at(&c, t);
                let q = eval::local(h);
                let q = p(q.x + s.origin().x, q.y + s.origin().y);
                assert!(near(q, s.point_at(u0 + (u1 - u0) * t), 1e-9));
            }
        }
    }
}

#[test]
fn a_long_spline_seen_from_afar_is_followed_in_few_points_and_still_within_tolerance() {
    // 2,000 control points on a wiggle 0.1 apart: at a tolerance of 2 most
    // runs of spans fit in a box that small.
    let points: Vec<Point> = (0..2000)
        .map(|i| {
            let x = f64::from(i) * 0.1;
            p(x, (x / 7.0).sin() * 20.0 + (x * 5.0).sin() * 0.3)
        })
        .collect();
    let s = Spline::from_control_points(3, points).unwrap();
    let tolerance = 2.0;
    let outline = s.flatten_params(tolerance, MAX_POINTS).unwrap();
    assert!(outline.len() < 400, "{} points", outline.len());
    for (u, q) in &outline {
        assert!(near(s.point_at(*u), *q, 1e-9));
    }
    for w in outline.windows(2) {
        for f in [0.1, 0.3, 0.5, 0.7, 0.9] {
            let on = s.point_at(w[0].0 + (w[1].0 - w[0].0) * f);
            assert!(flatten::to_segment(on, w[0].1, w[1].1) <= tolerance + 1e-9);
        }
    }
}
