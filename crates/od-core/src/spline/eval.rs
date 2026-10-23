//! Evaluating a spline: the span a parameter falls in, De Boor's algorithm
//! in homogeneous coordinates, and each span as a rational Bézier piece,
//! from which derivatives, outlines and lengths are worked out.
//!
//! Points are taken relative to the first control point (`origin`) so that
//! a curve far from 0 keeps its precision. A Bézier piece's control points
//! are the blossom of its span at the span's two knots, so extracting one
//! looks only at that span's `p + 1` control points. Pieces and the De Boor
//! working points live on the stack: drawing evaluates thousands a frame.

use super::{Spline, MAX_DEGREE};
use crate::Point;

/// A homogeneous point `(w·x, w·y, w)`, relative to the spline's origin.
pub(super) type H = [f64; 3];

/// The most control points of one span or Bézier piece.
const ORDER: usize = MAX_DEGREE as usize + 1;

fn lerp(a: H, b: H, t: f64) -> H {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The point a homogeneous point stands for, relative to the origin.
pub(super) fn local(h: H) -> Point {
    Point::new(h[0] / h[2], h[1] / h[2])
}

/// A rational Bézier piece at `t` in `[0, 1]`: the homogeneous point and its
/// derivative with respect to `t`.
pub(super) fn bezier_at(c: &[H], t: f64) -> (H, H) {
    let n = c.len().min(ORDER);
    let p = n.saturating_sub(1);
    if p == 0 {
        return (c.first().copied().unwrap_or([0.0, 0.0, 1.0]), [0.0; 3]);
    }
    let mut d = [[0.0; 3]; ORDER];
    d[..n].copy_from_slice(&c[..n]);
    for r in 1..p {
        for j in 0..=p - r {
            d[j] = lerp(d[j], d[j + 1], t);
        }
    }
    let (a, b) = (d[0], d[1]);
    let m = p as f64;
    (
        lerp(a, b, t),
        [m * (b[0] - a[0]), m * (b[1] - a[1]), m * (b[2] - a[2])],
    )
}

/// A rational Bézier piece cut in two at its middle, into `left` and
/// `right` (cleared first). `c` is used as working space.
pub(super) fn halve(c: &mut [H], left: &mut Vec<H>, right: &mut Vec<H>) {
    let n = c.len();
    left.clear();
    right.clear();
    for r in 0..n {
        left.push(c[0]);
        right.push(c[n - 1 - r]);
        for j in 0..n - 1 - r {
            c[j] = lerp(c[j], c[j + 1], 0.5);
        }
    }
    right.reverse();
}

/// The derivative of the projected curve from a homogeneous point and its
/// derivative (the quotient rule).
pub(super) fn velocity(h: H, dh: H) -> Point {
    let c = local(h);
    Point::new((dh[0] - dh[2] * c.x) / h[2], (dh[1] - dh[2] * c.y) / h[2])
}

impl Spline {
    /// Whether the arrays fit together, so evaluation can index them. A
    /// spline that passed [`validate`](Self::validate) always does.
    pub(super) fn well_formed(&self) -> bool {
        let p = usize::from(self.degree);
        let count = self.control_points.len();
        p >= 1
            && count > p
            && self.knots.len() == count + p + 1
            && self.weights.as_ref().is_none_or(|w| w.len() == count)
    }

    /// Where local coordinates are measured from.
    pub(super) fn origin(&self) -> Point {
        self.control_points.first().copied().unwrap_or_default()
    }

    fn homogeneous(&self, i: usize) -> H {
        let o = self.origin();
        let q = self.control_points.get(i).copied().unwrap_or(o);
        let w = self.weight(i);
        [w * (q.x - o.x), w * (q.y - o.y), w]
    }

    /// The spans with any length, in order: each `k` has `U_k < U_k+1`.
    pub(super) fn spans(&self) -> impl Iterator<Item = usize> + '_ {
        let p = usize::from(self.degree);
        let count = if self.well_formed() {
            self.control_points.len()
        } else {
            p
        };
        (p..count).filter(move |&k| self.knots[k] < self.knots[k + 1])
    }

    /// The span holding `u`, clamped to the domain: the `k` in `p..n+1` with
    /// `U_k <= u < U_k+1`. The domain's end belongs to the last span with
    /// any length. Assumes [`well_formed`](Self::well_formed).
    fn span(&self, u: f64) -> usize {
        let p = usize::from(self.degree);
        let count = self.control_points.len();
        let knots = &self.knots;
        if u >= knots[count] {
            let mut k = count - 1;
            while k > p && knots[k] >= knots[k + 1] {
                k -= 1;
            }
            return k;
        }
        if u <= knots[p] {
            let mut k = p;
            while k < count - 1 && knots[k + 1] <= knots[k] {
                k += 1;
            }
            return k;
        }
        // knots[lo] <= u < knots[hi] throughout.
        let (mut lo, mut hi) = (p, count);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if knots[mid] <= u {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// De Boor's algorithm on span `k` with the arguments `xs` (all the
    /// same for a point; the span's ends for a blossom), working in `d`
    /// (at least `p + 1` long).
    fn de_boor(&self, k: usize, xs: impl Fn(usize) -> f64, d: &mut [H]) -> H {
        let p = usize::from(self.degree);
        let knots = &self.knots;
        for (j, slot) in d.iter_mut().enumerate().take(p + 1) {
            *slot = self.homogeneous(k - p + j);
        }
        for r in 1..=p {
            let x = xs(r);
            for j in (r..=p).rev() {
                let i = k - p + j;
                let gap = knots[i + p + 1 - r] - knots[i];
                let alpha = if gap > 0.0 { (x - knots[i]) / gap } else { 0.0 };
                d[j] = lerp(d[j - 1], d[j], alpha);
            }
        }
        d[p]
    }

    /// The point at parameter `u`, clamped to the domain.
    pub fn point_at(&self, u: f64) -> Point {
        let o = self.origin();
        if !self.well_formed() {
            return o;
        }
        let (a, b) = self.domain();
        let u = u.clamp(a, b);
        let mut d = [[0.0; 3]; ORDER];
        let c = local(self.de_boor(self.span(u), |_| u, &mut d));
        Point::new(o.x + c.x, o.y + c.y)
    }

    /// The derivative of the curve at `u` (clamped to the domain): the
    /// direction it runs, as long as the parameter's speed.
    pub fn derivative_at(&self, u: f64) -> Point {
        if !self.well_formed() {
            return Point::default();
        }
        let (a, b) = self.domain();
        let u = u.clamp(a, b);
        let k = self.span(u);
        let (u0, u1) = (self.knots[k], self.knots[k + 1]);
        let (h, dh) = bezier_at(&self.bezier(k), (u - u0) / (u1 - u0));
        let v = velocity(h, dh);
        Point::new(v.x / (u1 - u0), v.y / (u1 - u0))
    }

    /// The unit direction the curve runs at `u`, or `None` where it stops.
    pub fn tangent_at(&self, u: f64) -> Option<Point> {
        let d = self.derivative_at(u);
        let len = d.x.hypot(d.y);
        (len > 0.0 && len.is_finite()).then(|| Point::new(d.x / len, d.y / len))
    }

    /// Span `k` (from `U_k` to `U_k+1`, which must differ) as a rational
    /// Bézier piece: `p + 1` homogeneous control points about the origin,
    /// added to `out`; `scratch` (at least `p + 1` long) is working space.
    pub(super) fn bezier_into(&self, k: usize, scratch: &mut [H], out: &mut Vec<H>) {
        let p = usize::from(self.degree);
        let (a, b) = (self.knots[k], self.knots[k + 1]);
        for i in 0..=p {
            out.push(self.de_boor(k, |r| if r <= p - i { a } else { b }, scratch));
        }
    }

    /// [`bezier_into`](Self::bezier_into) on its own.
    pub(super) fn bezier(&self, k: usize) -> Vec<H> {
        let n = usize::from(self.degree) + 1;
        let mut out = Vec::with_capacity(n);
        self.bezier_into(k, &mut vec![[0.0; 3]; n], &mut out);
        out
    }
}
