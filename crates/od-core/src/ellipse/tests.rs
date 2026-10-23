use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use crate::block::Affine;

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(super) fn near(a: Point, b: Point, tol: f64) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= tol
}

/// A 10-by-4 ellipse about (1, 2), tilted 30 degrees.
fn tilted() -> Ellipse {
    let r = PI / 6.0;
    Ellipse::full(p(1.0, 2.0), p(5.0 * r.cos(), 5.0 * r.sin()), 0.4).unwrap()
}

fn arc_of(e: Ellipse, start: f64, end: f64) -> Ellipse {
    Ellipse::new(e.center, e.major_axis, e.radius_ratio, start, end).unwrap()
}

/// Distance from `p` to the segment `a`-`b`.
fn off_chord(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

/// A small repeatable pseudo-random source, so the checks need no crate.
pub(super) struct Rng(pub(super) u64);

impl Rng {
    pub(super) fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(super) fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }

    pub(super) fn ellipse(&mut self) -> Ellipse {
        let center = p(self.range(-50.0, 50.0), self.range(-50.0, 50.0));
        let (a, angle) = (self.range(0.5, 20.0), self.range(0.0, TAU));
        let ratio = [1.0, 0.5, 0.1, 1e-3][(self.next() * 4.0) as usize % 4];
        let axis = p(a * angle.cos(), a * angle.sin());
        if self.next() < 0.3 {
            Ellipse::full(center, axis, ratio).unwrap()
        } else {
            let start = self.range(0.0, TAU);
            Ellipse::new(center, axis, ratio, start, start + self.range(0.1, 6.0)).unwrap()
        }
    }
}

#[test]
fn the_axis_ends_are_at_the_quarter_parameters() {
    let e = tilted();
    let c = e.center;
    let (a, b) = (e.major_axis, e.minor_axis());
    assert!(near(e.point_at(0.0), p(c.x + a.x, c.y + a.y), 1e-12));
    assert!(near(e.point_at(FRAC_PI_2), p(c.x + b.x, c.y + b.y), 1e-12));
    assert!(near(e.point_at(PI), p(c.x - a.x, c.y - a.y), 1e-12));
    assert!(near(
        e.point_at(3.0 * FRAC_PI_2),
        p(c.x - b.x, c.y - b.y),
        1e-12
    ));
    assert!((e.minor_radius() - 2.0).abs() < 1e-12);
}

#[test]
fn new_puts_parameters_in_canonical_form() {
    let e = tilted();
    let a = arc_of(e, -FRAC_PI_2, FRAC_PI_2);
    assert!((a.start - 3.0 * FRAC_PI_2).abs() < 1e-12 && (a.sweep() - PI).abs() < 1e-12);
    let wrapped = arc_of(e, 5.0, 1.0);
    assert!(
        (wrapped.sweep() - (1.0 + TAU - 5.0)).abs() < 1e-12,
        "the end goes round"
    );
    let whole = arc_of(e, 3.0, 3.0 + TAU);
    assert!(whole.is_full() && whole.start == 0.0 && whole.end == TAU);
    assert!(whole.validate().is_ok() && a.validate().is_ok());
}

#[test]
fn new_refuses_what_cannot_be_an_ellipse() {
    let (c, axis) = (p(0.0, 0.0), p(1.0, 0.0));
    let new = |c, axis, ratio, s, e| Ellipse::new(c, axis, ratio, s, e).unwrap_err();
    assert_eq!(
        new(c, p(0.0, 0.0), 0.5, 0.0, TAU),
        EllipseError::NullMajorAxis
    );
    assert_eq!(new(c, axis, 0.0, 0.0, TAU), EllipseError::RatioOutOfRange);
    assert_eq!(new(c, axis, 1.5, 0.0, TAU), EllipseError::RatioOutOfRange);
    assert_eq!(
        new(c, axis, f64::NAN, 0.0, TAU),
        EllipseError::RatioOutOfRange
    );
    assert_eq!(
        new(p(f64::NAN, 0.0), axis, 0.5, 0.0, TAU),
        EllipseError::NonFinite
    );
    assert_eq!(
        new(p(2e12, 0.0), axis, 0.5, 0.0, TAU),
        EllipseError::NonFinite
    );
    assert_eq!(new(c, axis, 0.5, 1.0, 1.0), EllipseError::EmptySweep);
    assert_eq!(new(c, axis, 0.5, 1.0, 1.0 + 1e-7), EllipseError::EmptySweep);
    assert_eq!(
        new(c, axis, 0.5, 0.0, 3.0 * TAU),
        EllipseError::SweepTooLarge
    );
    assert!(
        Ellipse::new(c, axis, 1.0 + 1e-12, 0.0, TAU).is_ok(),
        "rounding"
    );
    assert!(Ellipse::new(c, axis, MIN_RATIO, 0.0, TAU).is_ok());
    let mut bad = Ellipse::full(c, axis, 0.5).unwrap();
    bad.start = -1.0;
    bad.end = 1.0;
    assert_eq!(bad.validate(), Err(EllipseError::NotCanonical));
}

#[test]
fn a_longer_second_axis_becomes_the_major_one_keeping_the_points() {
    let c = p(1.0, 1.0);
    // First axis 2 along x, the other 5: the drawn points are fixed by them.
    let first = p(2.0, 0.0);
    let drawn = |t: f64| p(c.x + 2.0 * t.cos(), c.y + 5.0 * t.sin());
    let e = Ellipse::from_axes(c, first, 5.0, 0.3, 2.0).unwrap();
    assert!((e.major_radius() - 5.0).abs() < 1e-12 && (e.radius_ratio - 0.4).abs() < 1e-12);
    assert!(near(e.start_point(), drawn(0.3), 1e-12));
    assert!(near(e.end_point(), drawn(2.0), 1e-12));
    let whole = Ellipse::from_axes(c, first, 5.0, 0.0, TAU).unwrap();
    assert!(whole.is_full());
}

#[test]
fn angles_and_parameters_convert_both_ways() {
    for ratio in [1.0, 0.5, 1e-3, 1e-6] {
        let e = Ellipse::full(p(3.0, -1.0), p(0.0, 4.0), ratio).unwrap();
        for i in 0..16 {
            let angle = (f64::from(i) + 0.25) * TAU / 16.0;
            let t = e.parameter_at_angle(angle);
            let back = e.angle_at(t);
            let diff = (back - angle).rem_euclid(TAU);
            assert!(diff.min(TAU - diff) < 1e-9, "ratio {ratio} angle {angle}");
            // The point at that parameter lies on the ray.
            let q = e.point_at(t);
            let seen = (q.y - e.center.y).atan2(q.x - e.center.x);
            let diff = (seen - angle).rem_euclid(TAU);
            assert!(diff.min(TAU - diff) < 1e-6, "ratio {ratio}");
            assert!((e.parameter_of(q) - t).abs() < 1e-9 || ratio < 1e-3);
        }
    }
}

#[test]
fn bounds_hold_the_curve_and_touch_it() {
    let mut rng = Rng(7);
    for _ in 0..200 {
        let e = rng.ellipse();
        let b = e.bounds();
        let (mut lo, mut hi) = (p(f64::MAX, f64::MAX), p(f64::MIN, f64::MIN));
        for i in 0..=20_000 {
            let q = e.point_at(e.start + e.sweep() * f64::from(i) / 20_000.0);
            assert!(q.x >= b.min.x - 1e-9 && q.x <= b.max.x + 1e-9);
            assert!(q.y >= b.min.y - 1e-9 && q.y <= b.max.y + 1e-9);
            lo = p(lo.x.min(q.x), lo.y.min(q.y));
            hi = p(hi.x.max(q.x), hi.y.max(q.y));
        }
        let slack = 1e-4 * e.major_radius();
        assert!(lo.x - b.min.x < slack && b.max.x - hi.x < slack, "{e:?}");
        assert!(lo.y - b.min.y < slack && b.max.y - hi.y < slack, "{e:?}");
    }
}

#[test]
fn the_nearest_point_is_no_farther_than_a_dense_search() {
    let mut rng = Rng(11);
    for _ in 0..300 {
        let e = rng.ellipse();
        let q = p(
            e.center.x + rng.range(-30.0, 30.0),
            e.center.y + rng.range(-30.0, 30.0),
        );
        let (found, t) = e.nearest(q);
        assert!(e.contains(t), "on the arc");
        assert!(near(found, e.point_at(t), 1e-9));
        let best = (0..=50_000)
            .map(|i| e.point_at(e.start + e.sweep() * f64::from(i) / 50_000.0))
            .map(|s| (s.x - q.x).hypot(s.y - q.y))
            .fold(f64::INFINITY, f64::min);
        let got = (found.x - q.x).hypot(found.y - q.y);
        assert!(got <= best + 1e-7, "{e:?} {q:?}: {got} > {best}");
    }
}

#[test]
fn the_nearest_point_handles_the_flattest_ellipse() {
    let e = Ellipse::full(p(0.0, 0.0), p(100.0, 0.0), MIN_RATIO).unwrap();
    // Across the middle of the flat side, the nearest point is almost
    // straight below (the curve's own y pulls it 2e-6 along).
    let (q, _) = e.nearest(p(50.0, 3.0));
    assert!((q.x - 50.0).abs() < 1e-5 && q.y.abs() < 1e-3, "{q:?}");
    assert!((e.distance_to(p(150.0, 0.0)) - 50.0).abs() < 1e-9);
}

#[test]
fn flattening_stays_within_the_tolerance() {
    let mut rng = Rng(3);
    for _ in 0..100 {
        let e = rng.ellipse();
        let tolerance = e.major_radius() * 1e-3;
        let points = e.flatten(tolerance).unwrap();
        assert!(near(points[0], e.start_point(), 1e-12));
        assert!(near(points[points.len() - 1], e.end_point(), 1e-9));
        if e.is_full() {
            assert_eq!(points[0], points[points.len() - 1], "a ring closes");
        }
        // Every point of the curve is close to the path.
        for i in 0..=2000 {
            let q = e.point_at(e.start + e.sweep() * f64::from(i) / 2000.0);
            let off = points
                .windows(2)
                .map(|w| off_chord(q, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            assert!(off <= tolerance * 1.01, "{e:?}");
        }
    }
}

#[test]
fn flattening_is_bounded_and_says_so() {
    let e = Ellipse::full(p(0.0, 0.0), p(1e9, 0.0), 0.5).unwrap();
    assert_eq!(e.flatten(1e-6), Err(EllipseError::ResourceLimit));
    let coarse = e.outline(1e-6);
    assert!(coarse.len() > 100 && coarse.len() <= flatten::MAX_POINTS);
    let ordinary = Ellipse::full(p(0.0, 0.0), p(100.0, 0.0), 0.5).unwrap();
    assert!(ordinary.flatten(0.5).unwrap().len() < 128);
}

#[test]
fn moves_turns_scales_and_mirrors_keep_the_curve() {
    let e = arc_of(tilted(), 0.5, 4.0);
    let sample = |e: &Ellipse| -> Vec<Point> {
        (0..=8)
            .map(|i| e.point_at(e.start + e.sweep() * f64::from(i) / 8.0))
            .collect()
    };
    let moved = e.translated(3.0, -2.0);
    for (a, b) in sample(&e).iter().zip(sample(&moved)) {
        assert!(near(a.translated(3.0, -2.0), b, 1e-9));
    }
    let turned = e.rotated(p(5.0, 5.0), 1.0);
    for (a, b) in sample(&e).iter().zip(sample(&turned)) {
        assert!(near(a.rotated(p(5.0, 5.0), 1.0), b, 1e-9));
    }
    let scaled = e.scaled(p(-1.0, 0.0), 2.5);
    for (a, b) in sample(&e).iter().zip(sample(&scaled)) {
        assert!(near(a.scaled(p(-1.0, 0.0), 2.5), b, 1e-9));
    }
    // A reflection runs the other way: the old end is the new start.
    let (m1, m2) = (p(0.0, 1.0), p(2.0, 4.0));
    let mirrored = e.mirrored(m1, m2);
    for (a, b) in sample(&e).iter().zip(sample(&mirrored).iter().rev()) {
        assert!(near(a.mirrored(m1, m2), *b, 1e-9));
    }
    let back = mirrored.mirrored(m1, m2);
    for (a, b) in sample(&e).iter().zip(sample(&back)) {
        assert!(near(*a, b, 1e-9));
    }
    assert!(mirrored.validate().is_ok() && back.validate().is_ok());
}

#[test]
fn any_affine_map_takes_an_ellipse_to_the_mapped_curve() {
    let mut rng = Rng(5);
    for _ in 0..300 {
        let e = rng.ellipse();
        let map = Affine {
            xx: rng.range(-3.0, 3.0),
            xy: rng.range(-3.0, 3.0),
            yx: rng.range(-3.0, 3.0),
            yy: rng.range(-3.0, 3.0),
            tx: rng.range(-10.0, 10.0),
            ty: rng.range(-10.0, 10.0),
        };
        if map.determinant().abs() < 0.1 {
            continue;
        }
        let Ok(m) = e.mapped(&map) else {
            continue; // squashed below the smallest ratio
        };
        assert!(m.validate().is_ok());
        let size = m.major_radius();
        for i in 0..=16 {
            let q = map.apply(e.point_at(e.start + e.sweep() * f64::from(i) / 16.0));
            assert!(m.distance_to(q) < 1e-7 * size.max(1.0), "{e:?} {map:?}");
        }
        if !e.is_full() {
            let (s, t) = (map.apply(e.start_point()), map.apply(e.end_point()));
            let (s, t) = if map.determinant() > 0.0 {
                (s, t)
            } else {
                (t, s)
            };
            assert!(near(m.start_point(), s, 1e-7 * size.max(1.0)));
            assert!(near(m.end_point(), t, 1e-7 * size.max(1.0)));
        }
    }
}

#[test]
fn a_circle_under_an_uneven_scale_is_an_exact_ellipse() {
    let circle = Ellipse::circular(p(1.0, 0.0), 2.0, 0.0, 0.0).unwrap();
    let m = circle.mapped(&Affine::scaling(1.0, 3.0)).unwrap();
    assert!(m.is_full());
    assert!((m.major_radius() - 6.0).abs() < 1e-12 && (m.minor_radius() - 2.0).abs() < 1e-12);
    assert!(near(m.center, p(1.0, 0.0), 1e-12));
    let quarter = Ellipse::circular(p(0.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
    let m = quarter.mapped(&Affine::scaling(2.0, 1.0)).unwrap();
    assert!(near(m.start_point(), p(2.0, 0.0), 1e-12));
    assert!(near(m.end_point(), p(0.0, 1.0), 1e-12));
}

#[test]
fn cutting_keeps_the_span_and_a_full_ellipse_needs_two_cuts() {
    let e = tilted();
    assert_eq!(e.split_at(&[e.point_at(1.0)]), None);
    let pieces = e.split_at(&[e.point_at(1.0), e.point_at(4.0)]).unwrap();
    assert_eq!(pieces.len(), 2);
    let total: f64 = pieces.iter().map(Ellipse::sweep).sum();
    assert!((total - TAU).abs() < 1e-9);
    assert!(near(pieces[0].end_point(), pieces[1].start_point(), 1e-9));

    let a = arc_of(e, 5.0, 5.0 + 3.0);
    let pieces = a
        .split_at(&[a.point_at(5.5), a.point_at(7.0), a.start_point()])
        .unwrap();
    assert_eq!(pieces.len(), 3, "a cut at the end is ignored");
    assert!(near(pieces[0].start_point(), a.start_point(), 1e-9));
    assert!(near(pieces[2].end_point(), a.end_point(), 1e-9));
    let total: f64 = pieces.iter().map(Ellipse::sweep).sum();
    assert!((total - 3.0).abs() < 1e-9);
}

#[test]
fn grips_resize_turn_and_move_the_ends() {
    let e = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
    let g = e.grips();
    assert_eq!(g.len(), 5);
    assert_eq!(g[1].at, p(4.0, 0.0));
    assert_eq!(g[3].at, p(0.0, 2.0));
    let moved = e.with_grip_moved(0, p(1.0, 1.0)).unwrap();
    assert_eq!(moved.center, p(1.0, 1.0));
    // The major end dragged up turns the ellipse and keeps the minor radius.
    let turned = e.with_grip_moved(1, p(0.0, 6.0)).unwrap();
    assert!(near(turned.major_axis, p(0.0, 6.0), 1e-12));
    assert!((turned.minor_radius() - 2.0).abs() < 1e-12);
    // The minor end dragged past the major radius swaps the axes.
    let swapped = e.with_grip_moved(3, p(1.0, 5.0)).unwrap();
    assert!((swapped.major_radius() - 5.0).abs() < 1e-12);
    assert!((swapped.minor_radius() - 4.0).abs() < 1e-12);
    assert_eq!(e.with_grip_moved(3, p(3.0, 0.0)), None, "no minor radius");
    assert_eq!(e.with_grip_moved(5, p(1.0, 1.0)), None, "no such grip");

    let a = arc_of(e, 0.0, PI);
    assert_eq!(a.grips().len(), 7);
    let shorter = a.with_grip_moved(6, p(-1.0, 3.0)).unwrap();
    assert!(shorter.sweep() < PI && (shorter.start - 0.0).abs() < 1e-12);
}

#[test]
fn length_matches_known_values() {
    let circle = Ellipse::circular(p(0.0, 0.0), 3.0, 0.0, 0.0).unwrap();
    assert!((circle.length() - TAU * 3.0).abs() < 1e-9);
    // a = 2, b = 1: 4·2·E(√3/2) = 9.688448220547675...
    let e = Ellipse::full(p(0.0, 0.0), p(2.0, 0.0), 0.5).unwrap();
    assert!((e.length() - 9.688_448_220_547_675).abs() < 1e-9);
    let half = arc_of(e, 0.0, PI);
    assert!((half.length() - 9.688_448_220_547_675 / 2.0).abs() < 1e-9);
    let flat = Ellipse::full(p(0.0, 0.0), p(1.0, 0.0), MIN_RATIO).unwrap();
    assert!((flat.length() - 4.0).abs() < 1e-5, "{}", flat.length());
    assert!((e.area() - 2.0 * PI).abs() < 1e-12);
    // An arc's area is the segment cut off by its chord: half for a half.
    assert!((half.area() - PI).abs() < 1e-12);
    let quarter = arc_of(e, 0.0, FRAC_PI_2);
    assert!(
        (quarter.area() - (FRAC_PI_2 - 1.0)).abs() < 1e-12,
        "a·b·(π/2 - 1)/2"
    );
}
