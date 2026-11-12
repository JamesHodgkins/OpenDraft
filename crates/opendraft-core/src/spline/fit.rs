//! A cubic spline through fit points, as SPLINE's Fit method makes it with
//! a fit tolerance of 0.
//!
//! As in AutoCAD the result is always cubic, even through two or three
//! points. The knots sit at the points' parameters (by chord, square root of
//! chord, or evenly), clamped at both ends, so `n` points give `n + 2`
//! control points: `n` equations put the curve through the points and two
//! more give it natural ends (no curvature at the start and end), as AutoCAD
//! does when no end tangents are given. The system is banded and solved with
//! partial pivoting. Knots are scaled as AutoCAD stores them: the chord
//! length, the sum of the square roots of the chords, or one per span.

use super::MAX_CONTROL_POINTS;
use super::{check_point, distance, KnotParameterization, Spline, SplineError, SplineFitData};
use crate::Point;

const DEGREE: usize = 3;

impl Spline {
    /// The cubic spline through `points` (at least two, none repeating the
    /// one before), with natural ends. Keeps the points as its fit data.
    pub fn from_fit_points(
        points: Vec<Point>,
        parameterization: KnotParameterization,
    ) -> Result<Self, SplineError> {
        let n = points.len();
        if n < 2 {
            return Err(SplineError::BadFitData);
        }
        if n + 2 > MAX_CONTROL_POINTS {
            return Err(SplineError::ResourceLimit);
        }
        for point in &points {
            check_point(*point)?;
        }
        let scale = points
            .iter()
            .fold(1.0f64, |m, q| m.max(q.x.abs()).max(q.y.abs()));
        let near = 1e-10 * scale;
        if let Some(i) = (1..n).find(|&i| distance(points[i], points[i - 1]) <= near) {
            return Err(SplineError::RepeatedPoint(i));
        }
        let (params, span) = parameters(&points, parameterization)?;
        let mut knots = vec![0.0; DEGREE + 1];
        knots.extend_from_slice(&params[1..n - 1]);
        knots.extend([1.0; DEGREE + 1]);
        let solved = solve(&interpolation(&knots, n), &rhs(&points))?;
        let control: Vec<Point> = solved.iter().map(|v| Point::new(v[0], v[1])).collect();
        let mut spline = Spline::new_nurbs(DEGREE as u8, control, knots, None, false, false)?;
        // The solve can lose precision on badly spaced points; check it.
        let allowed = (1e-9 * scale).max(64.0 * f64::EPSILON * scale);
        let misses = params
            .iter()
            .zip(&points)
            .any(|(t, q)| distance(spline.point_at(*t), *q) > allowed);
        if misses {
            return Err(SplineError::Degenerate);
        }
        spline.knots.iter_mut().for_each(|k| *k *= span);
        spline.fit = Some(SplineFitData::exact(points, parameterization));
        Ok(spline)
    }

    /// Whether moving a fit point can refit the curve the way it was made:
    /// cubic, open, through its points exactly, with no end tangents.
    /// Otherwise edits work on the control points and drop the fit data.
    pub fn is_refittable(&self) -> bool {
        self.fit.as_ref().is_some_and(|fit| {
            self.degree == DEGREE as u8
                && !self.closed
                && fit.tolerance == 0.0
                && fit.start_tangent.is_none()
                && fit.end_tangent.is_none()
        })
    }

    /// The spline refitted through `points`, keeping how the knots were
    /// spaced (by chord when that is not known).
    pub fn refitted(&self, points: Vec<Point>) -> Result<Self, SplineError> {
        let how = self
            .fit
            .as_ref()
            .and_then(|f| f.parameterization)
            .unwrap_or(KnotParameterization::Chord);
        Self::from_fit_points(points, how)
    }
}

/// The parameter of each point in `[0, 1]`, strictly increasing, and the
/// length of the whole parameter range as AutoCAD stores it (the chord
/// length, the sum of the square roots of the chords, or the number of
/// spans); knots are multiplied by it once the curve is solved.
pub(super) fn parameters(
    points: &[Point],
    how: KnotParameterization,
) -> Result<(Vec<f64>, f64), SplineError> {
    let steps: Vec<f64> = points
        .windows(2)
        .map(|w| {
            let d = distance(w[0], w[1]);
            match how {
                KnotParameterization::Chord => d,
                KnotParameterization::SqrtChord => d.sqrt(),
                KnotParameterization::Uniform => 1.0,
            }
        })
        .collect();
    let total: f64 = steps.iter().sum();
    if !(total.is_finite() && total > 0.0) {
        return Err(SplineError::Degenerate);
    }
    let mut params = Vec::with_capacity(points.len());
    let mut run = 0.0;
    params.push(0.0);
    for step in &steps[..steps.len() - 1] {
        run += step;
        params.push(run / total);
    }
    params.push(1.0);
    if params.windows(2).any(|w| w[1] <= w[0]) {
        return Err(SplineError::Degenerate);
    }
    Ok((params, total))
}

/// The cubic B-spline basis functions that are not zero at `u` in span `k`
/// (`N_k-3 .. N_k`), by the triangular scheme.
pub(super) fn basis(knots: &[f64], k: usize, u: f64) -> [f64; DEGREE + 1] {
    let mut n = [0.0; DEGREE + 1];
    let mut left = [0.0; DEGREE + 1];
    let mut right = [0.0; DEGREE + 1];
    n[0] = 1.0;
    for j in 1..=DEGREE {
        left[j] = u - knots[k + 1 - j];
        right[j] = knots[k + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let gap = right[r + 1] + left[j - r];
            let temp = if gap > 0.0 { n[r] / gap } else { 0.0 };
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    n
}

/// The system for `n` points, one row per equation.
fn interpolation(knots: &[f64], n: usize) -> Band {
    let size = n + 2;
    let mut band = Band::new(size, 1, 2);
    // Through the first and last points.
    band.set(0, 0, 1.0);
    band.set(size - 1, size - 1, 1.0);
    // No curvature at the start: with clamped knots C''(0) is a multiple of
    // U5·P0 - (U4 + U5)·P1 + U4·P2.
    let (u4, u5) = (knots[4], knots[5]);
    band.set(1, 0, u5);
    band.set(1, 1, -(u4 + u5));
    band.set(1, 2, u4);
    // And at the end, measured back from 1.
    let last = size - 1;
    let (d4, d5) = (1.0 - knots[last], 1.0 - knots[last - 1]);
    band.set(n, last, d5);
    band.set(n, last - 1, -(d4 + d5));
    band.set(n, last - 2, d4);
    // Through each inner point, at its knot.
    for i in 1..n - 1 {
        let k = i + DEGREE;
        let values = basis(knots, k, knots[k]);
        for (j, v) in values.iter().enumerate() {
            band.set(i + 1, k - DEGREE + j, *v);
        }
    }
    band
}

fn rhs(points: &[Point]) -> Vec<[f64; 2]> {
    let n = points.len();
    let mut b = vec![[0.0; 2]; n + 2];
    b[0] = [points[0].x, points[0].y];
    b[n + 1] = [points[n - 1].x, points[n - 1].y];
    for i in 1..n - 1 {
        b[i + 1] = [points[i].x, points[i].y];
    }
    b
}

/// A square band matrix: row `r` holds columns `r - kl ..= r + ku + kl`,
/// room for the fill that pivoting among the next `kl` rows brings.
#[derive(Clone)]
pub(super) struct Band {
    size: usize,
    kl: usize,
    width: usize,
    data: Vec<f64>,
}

impl Band {
    pub(super) fn new(size: usize, kl: usize, ku: usize) -> Self {
        let width = 2 * kl + ku + 1;
        Self {
            size,
            kl,
            width,
            data: vec![0.0; size * width],
        }
    }

    fn slot(&self, r: usize, c: usize) -> Option<usize> {
        let at = (c + self.kl).checked_sub(r)?;
        (at < self.width && r < self.size && c < self.size).then_some(r * self.width + at)
    }

    fn get(&self, r: usize, c: usize) -> f64 {
        self.slot(r, c).map_or(0.0, |i| self.data[i])
    }

    fn set(&mut self, r: usize, c: usize, v: f64) {
        if let Some(i) = self.slot(r, c) {
            self.data[i] = v;
        }
    }

    /// Adds `v` to an entry; an entry outside the band is an error.
    pub(super) fn add(&mut self, r: usize, c: usize, v: f64) -> Result<(), SplineError> {
        let i = self.slot(r, c).ok_or(SplineError::Degenerate)?;
        self.data[i] += v;
        Ok(())
    }

    /// The last column row `r` can hold.
    fn reach(&self, r: usize) -> usize {
        (r + self.width - 1 - self.kl).min(self.size - 1)
    }
}

/// Solves `band · x = b` for both coordinates by Gaussian elimination with
/// partial pivoting; the lowest row wins a tie. A pivot that vanishes next
/// to its row is a system with no single answer.
pub(super) fn solve(band: &Band, b: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, SplineError> {
    let mut a = band.clone();
    let mut b = b.to_vec();
    let n = a.size;
    for j in 0..n {
        let last_row = (j + a.kl).min(n - 1);
        let mut pivot = j;
        for r in j + 1..=last_row {
            if a.get(r, j).abs() > a.get(pivot, j).abs() {
                pivot = r;
            }
        }
        let reach = a.reach(j);
        if pivot != j {
            for c in j..=reach {
                let (x, y) = (a.get(j, c), a.get(pivot, c));
                a.set(j, c, y);
                a.set(pivot, c, x);
            }
            b.swap(j, pivot);
        }
        let norm = (j..=reach).fold(0.0f64, |m, c| m.max(a.get(j, c).abs()));
        let head = a.get(j, j);
        if !(norm > 0.0 && head.abs() > 1e-14 * norm) {
            return Err(SplineError::Degenerate);
        }
        for r in j + 1..=last_row {
            let factor = a.get(r, j) / head;
            if factor == 0.0 {
                continue;
            }
            for c in j..=reach {
                let v = a.get(r, c) - factor * a.get(j, c);
                a.set(r, c, v);
            }
            b[r] = [b[r][0] - factor * b[j][0], b[r][1] - factor * b[j][1]];
        }
    }
    let mut x = vec![[0.0; 2]; n];
    for j in (0..n).rev() {
        let mut s = b[j];
        for (c, known) in x.iter().enumerate().take(a.reach(j) + 1).skip(j + 1) {
            let v = a.get(j, c);
            s = [s[0] - v * known[0], s[1] - v * known[1]];
        }
        let head = a.get(j, j);
        x[j] = [s[0] / head, s[1] / head];
    }
    if x.iter().all(|v| v[0].is_finite() && v[1].is_finite()) {
        Ok(x)
    } else {
        Err(SplineError::NonFinite)
    }
}
