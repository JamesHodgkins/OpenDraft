//! Lengths and nearest points. Bounded: each does at most a fixed amount of
//! work, and a length that cannot be found within it is an error, never a
//! guess.

use super::eval::{bezier_at, velocity};
use super::flatten::{to_segment, MAX_POINTS};
use super::{distance, epsilon, Spline, SplineError};
use crate::Point;

/// Most pieces the length integration may look at.
const MAX_LENGTH_WORK: usize = 100_000;
/// How many times a piece of the length integration may be halved.
const MAX_LENGTH_DEPTH: u32 = 20;
/// How many stretches of the outline the nearest-point search refines.
const NEAREST_CANDIDATES: usize = 8;
/// Golden-section steps per stretch: enough to shrink it by 1e-13.
const NEAREST_STEPS: usize = 64;

/// Gauss-Kronrod 7-15 nodes on [-1, 1] (the positive half, the last 0),
/// their Kronrod weights, and the Gauss weights of every other node.
const GK_NODES: [f64; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_4,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_5,
    0.0,
];
const GK_WEIGHTS: [f64; 8] = [
    0.022_935_322_010_529_225,
    0.063_092_092_629_978_55,
    0.104_790_010_322_250_18,
    0.140_653_259_715_525_92,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_4,
    0.204_432_940_075_298_9,
    0.209_482_141_084_727_83,
];
const GAUSS_WEIGHTS: [f64; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_7,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

/// The integral of `f` from `a` to `b` by the 15-point Kronrod rule, and
/// how far the 7-point Gauss rule within it differs (an error estimate).
fn kronrod(f: impl Fn(f64) -> f64, a: f64, b: f64) -> (f64, f64) {
    let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
    let centre = f(mid);
    let (mut kronrod, mut gauss) = (GK_WEIGHTS[7] * centre, GAUSS_WEIGHTS[3] * centre);
    for i in 0..7 {
        let pair = f(mid - half * GK_NODES[i]) + f(mid + half * GK_NODES[i]);
        kronrod += GK_WEIGHTS[i] * pair;
        if i % 2 == 1 {
            gauss += GAUSS_WEIGHTS[i / 2] * pair;
        }
    }
    (kronrod * half, ((kronrod - gauss) * half).abs())
}

/// A stretch of the curve between two parameters, and its length.
#[derive(Clone, Copy, Debug)]
struct Piece {
    u0: f64,
    u1: f64,
    length: f64,
}

impl Spline {
    /// How fast the curve runs at `u`.
    fn speed(&self, u: f64) -> f64 {
        let d = self.derivative_at(u);
        d.x.hypot(d.y)
    }

    /// The curve cut into pieces with their lengths, by adaptive
    /// Gauss-Kronrod integration of the speed over each span: a span is
    /// halved until the estimate agrees with itself to 1e-10.
    fn pieces(&self) -> Result<Vec<Piece>, SplineError> {
        let mut out = Vec::new();
        let mut work = 0usize;
        let floor = epsilon(self.scale());
        for k in self.spans() {
            let (u0, u1) = (self.knots[k], self.knots[k + 1]);
            let c = self.bezier(k);
            // Speed per unit of the span's own parameter t in [0, 1].
            let speed = |t: f64| {
                let (h, dh) = bezier_at(&c, t);
                let v = velocity(h, dh);
                v.x.hypot(v.y)
            };
            let mut stack = vec![(0.0, 1.0, 0u32)];
            while let Some((t0, t1, depth)) = stack.pop() {
                work += 1;
                if work > MAX_LENGTH_WORK {
                    return Err(SplineError::ResourceLimit);
                }
                let (length, error) = kronrod(speed, t0, t1);
                if error <= 1e-10 * length.max(floor) || depth >= MAX_LENGTH_DEPTH {
                    out.push(Piece {
                        u0: u0 + (u1 - u0) * t0,
                        u1: u0 + (u1 - u0) * t1,
                        length,
                    });
                } else {
                    let mid = (t0 + t1) / 2.0;
                    stack.push((mid, t1, depth + 1));
                    stack.push((t0, mid, depth + 1));
                }
            }
        }
        if !out.iter().all(|p| p.length.is_finite()) {
            return Err(SplineError::NonFinite);
        }
        Ok(out)
    }

    /// The length of the curve.
    pub fn length(&self) -> Result<f64, SplineError> {
        Ok(self.pieces()?.iter().map(|p| p.length).sum())
    }

    /// The point `s` along the curve from its start (clamped to the curve).
    pub fn point_at_length(&self, s: f64) -> Result<Point, SplineError> {
        let pieces = self.pieces()?;
        let mut run = 0.0;
        for piece in &pieces {
            if run + piece.length < s {
                run += piece.length;
                continue;
            }
            // The piece is smooth enough for one rule over any part of it;
            // find where the remaining length runs out by halving.
            let want = s - run;
            let (mut lo, mut hi) = (piece.u0, piece.u1);
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if kronrod(|u| self.speed(u), piece.u0, mid).0 < want {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Ok(self.point_at((lo + hi) / 2.0));
        }
        Ok(self.end_point())
    }

    /// The point halfway along the curve (what the Midpoint snap finds).
    pub fn mid_point(&self) -> Result<Point, SplineError> {
        self.point_at_length(self.length()? / 2.0)
    }

    /// The point of the curve nearest `p`, and its parameter. The outline
    /// at a fraction of the curve's size picks the stretches that can hold
    /// the answer (at most [`NEAREST_CANDIDATES`]); a golden-section search
    /// along each finds it. Equal distances go to the lower parameter.
    pub fn nearest(&self, p: Point) -> (Point, f64) {
        let (a, b) = self.domain();
        let start = (self.start_point(), a);
        if !self.well_formed() {
            return start;
        }
        let box_ = self.bounds();
        let size = (box_.max.x - box_.min.x).hypot(box_.max.y - box_.min.y);
        let tolerance = (size * 1e-3).max(epsilon(self.scale()));
        let samples = self
            .flatten_params(tolerance, MAX_POINTS)
            .unwrap_or_else(|_| self.coarse());
        let gaps: Vec<f64> = samples
            .windows(2)
            .map(|w| to_segment(p, w[0].1, w[1].1))
            .collect();
        let closest = gaps.iter().copied().fold(f64::INFINITY, f64::min);
        let mut candidates: Vec<usize> = (0..gaps.len())
            .filter(|&i| gaps[i] <= closest + 2.0 * tolerance)
            .collect();
        candidates.sort_by(|&i, &j| gaps[i].total_cmp(&gaps[j]).then(i.cmp(&j)));
        candidates.truncate(NEAREST_CANDIDATES);
        let mut best = start;
        let mut best_gap = distance(start.0, p);
        let end = (self.end_point(), b);
        let mut consider = |(q, u): (Point, f64)| {
            let gap = distance(q, p);
            if gap < best_gap || (gap == best_gap && u < best.1) {
                best = (q, u);
                best_gap = gap;
            }
        };
        consider(end);
        for i in candidates {
            consider(self.closest_between(p, samples[i].0, samples[i + 1].0));
        }
        best
    }

    /// The point nearest `p` between parameters `lo` and `hi`, by
    /// golden-section search.
    fn closest_between(&self, p: Point, mut lo: f64, mut hi: f64) -> (Point, f64) {
        let gap = |u: f64| distance(self.point_at(u), p);
        let ratio = (5f64.sqrt() - 1.0) / 2.0;
        let mut x1 = hi - ratio * (hi - lo);
        let mut x2 = lo + ratio * (hi - lo);
        let (mut f1, mut f2) = (gap(x1), gap(x2));
        for _ in 0..NEAREST_STEPS {
            if f1 <= f2 {
                hi = x2;
                (x2, f2) = (x1, f1);
                x1 = hi - ratio * (hi - lo);
                f1 = gap(x1);
            } else {
                lo = x1;
                (x1, f1) = (x2, f2);
                x2 = lo + ratio * (hi - lo);
                f2 = gap(x2);
            }
        }
        let u = (lo + hi) / 2.0;
        (self.point_at(u), u)
    }

    /// Distance from `p` to the curve.
    pub fn distance_to(&self, p: Point) -> f64 {
        distance(self.nearest(p).0, p)
    }
}
