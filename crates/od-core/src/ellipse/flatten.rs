//! Following an ellipse with straight pieces, and measuring its length.
//! Both are derived and bounded: a call does at most a fixed amount of work
//! and says so rather than returning something less accurate than asked.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::{epsilon, Ellipse, EllipseError};
use crate::Point;

/// Most points one flattening may give.
pub const MAX_POINTS: usize = 8192;
/// Most pieces the length integration may look at.
const MAX_WORK: usize = 16384;
/// Points of the coarse outline used when the tolerance cannot be met.
const COARSE_POINTS: usize = 2048;

impl Ellipse {
    /// The parameters where the arc is cut into pieces of at most a quarter
    /// turn, at the ends of the axes it passes: start, those, end.
    fn quarter_marks(&self) -> Vec<f64> {
        let mut marks = vec![self.start];
        let first = (self.start / FRAC_PI_2).floor() as i64 + 1;
        let mut k = first;
        loop {
            let t = k as f64 * FRAC_PI_2;
            if t >= self.end - 1e-12 {
                break;
            }
            marks.push(t);
            k += 1;
        }
        marks.push(self.end);
        marks
    }

    /// Points along the curve, in parameter order from start to end (a full
    /// ellipse ends where it began), such that no straight piece between
    /// two of them strays more than `tolerance` from the curve. A tolerance
    /// that is not a positive number means the smallest the coordinates can
    /// tell. `ResourceLimit` when that would take more than
    /// [`MAX_POINTS`] points.
    ///
    /// The steps are even in the parameter, sized from a bound rather than
    /// by trial: the curve's second derivative is never longer than the
    /// major radius `a`, so a chord over a parameter step `h` strays at most
    /// `a·h²/8`. For a circle that is the fewest pieces possible; a flat
    /// ellipse gets as many as the circle round its major axis would.
    pub fn flatten(&self, tolerance: f64) -> Result<Vec<Point>, EllipseError> {
        let a = self.major_radius();
        let scale = self.center.x.abs().max(self.center.y.abs()).max(a);
        let tolerance = if tolerance.is_finite() && tolerance > 0.0 {
            tolerance
        } else {
            epsilon(scale)
        };
        // At least eight pieces a turn, however small, so it stays round.
        let step = (8.0 * tolerance / a).sqrt().min(FRAC_PI_2 / 2.0);
        let sweep = self.sweep();
        let pieces = (sweep / step).ceil();
        if !pieces.is_finite() || pieces >= MAX_POINTS as f64 {
            return Err(EllipseError::ResourceLimit);
        }
        let pieces = (pieces as usize).max(1);
        let h = sweep / pieces as f64;
        // Turning (cos, sin) on by h each time saves a sine and cosine a point.
        let (sin_h, cos_h) = h.sin_cos();
        let (mut sin, mut cos) = self.start.sin_cos();
        let (major, minor) = (self.major_axis, self.minor_axis());
        let mut out = Vec::with_capacity(pieces + 1);
        for _ in 0..pieces {
            out.push(Point::new(
                self.center.x + major.x * cos + minor.x * sin,
                self.center.y + major.y * cos + minor.y * sin,
            ));
            (sin, cos) = (sin * cos_h + cos * sin_h, cos * cos_h - sin * sin_h);
        }
        out.push(if self.is_full() {
            out[0]
        } else {
            self.end_point()
        });
        Ok(out)
    }

    /// Like [`flatten`](Self::flatten), but never fails: when the tolerance
    /// cannot be met it gives an even outline of a fixed number of points.
    /// For the screen, where the next frame tries again; plots and files
    /// use `flatten` and report the failure.
    pub fn outline(&self, tolerance: f64) -> Vec<Point> {
        self.flatten(tolerance).unwrap_or_else(|_| {
            let sweep = self.sweep();
            (0..=COARSE_POINTS)
                .map(|i| self.point_at(self.start + sweep * i as f64 / COARSE_POINTS as f64))
                .collect()
        })
    }

    /// The length of the curve (the circumference of a full ellipse),
    /// integrated piece by piece to about twelve significant figures.
    pub fn length(&self) -> f64 {
        let speed = |t: f64| {
            let d = self.tangent_at(t);
            d.x.hypot(d.y)
        };
        let simpson = |t0: f64, t1: f64| {
            (t1 - t0) / 6.0 * (speed(t0) + 4.0 * speed((t0 + t1) / 2.0) + speed(t1))
        };
        let mut total = 0.0;
        let mut work = 0usize;
        let tolerance = 1e-13 * self.major_radius() * TAU;
        for pair in self.quarter_marks().windows(2) {
            let mut stack = vec![(pair[0], pair[1], simpson(pair[0], pair[1]), 0u32)];
            while let Some((t0, t1, whole, depth)) = stack.pop() {
                work += 1;
                let mid = (t0 + t1) / 2.0;
                let (left, right) = (simpson(t0, mid), simpson(mid, t1));
                let error = (left + right - whole).abs();
                if error <= 15.0 * tolerance || depth >= 40 || work > MAX_WORK {
                    total += left + right + (left + right - whole) / 15.0;
                } else {
                    stack.push((mid, t1, right, depth + 1));
                    stack.push((t0, mid, left, depth + 1));
                }
            }
        }
        total
    }
}
