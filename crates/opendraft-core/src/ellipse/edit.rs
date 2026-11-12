//! Editing an ellipse: its grips, cutting it at points, and mapping it
//! through an affine map (a block reference with uneven scales).

use std::f64::consts::TAU;

use super::{dot, left, vector, Ellipse, EllipseError, PARAM_EPS};
use crate::block::Affine;
use crate::{Grip, GripKind, Point};

impl Ellipse {
    /// The grips, by index: 0 the centre (moves it), 1 and 2 the ends of the
    /// major axis, 3 and 4 the ends of the minor axis (each resizes), and on
    /// an arc 5 and 6 its start and end.
    pub fn grips(&self) -> Vec<Grip> {
        let c = self.center;
        let (a, b) = (self.major_axis, self.minor_axis());
        let grip = |at, kind| Grip { at, kind };
        let mut grips = vec![
            grip(c, GripKind::Move),
            grip(Point::new(c.x + a.x, c.y + a.y), GripKind::Resize),
            grip(Point::new(c.x - a.x, c.y - a.y), GripKind::Resize),
            grip(Point::new(c.x + b.x, c.y + b.y), GripKind::Resize),
            grip(Point::new(c.x - b.x, c.y - b.y), GripKind::Resize),
        ];
        if !self.is_full() {
            grips.push(grip(self.start_point(), GripKind::Stretch));
            grips.push(grip(self.end_point(), GripKind::Stretch));
        }
        grips
    }

    /// The ellipse with grip `index` dragged to `to` (see
    /// [`grips`](Self::grips)), or `None` when that leaves no ellipse.
    ///
    /// An end of the major axis goes where it is dragged, turning the
    /// ellipse, and the minor radius stays; an end of the minor axis sets the
    /// minor radius to its distance across the major axis. Either may make
    /// the other axis the longer one. An arc's end moves to the point of the
    /// ellipse nearest where it is dragged.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<Ellipse> {
        let c = self.center;
        let off = vector(to.x - c.x, to.y - c.y);
        match index {
            0 => Some(self.translated(off.x, off.y)),
            1 | 2 => {
                let sign = if index == 1 { 1.0 } else { -1.0 };
                let axis = vector(off.x * sign, off.y * sign);
                // The parameters are kept, measured from the new axis.
                Ellipse::from_axes(c, axis, self.minor_radius(), self.start, self.end).ok()
            }
            3 | 4 => {
                let a = self.major_radius();
                let across = dot(off, left(self.major_axis)).abs() / a;
                Ellipse::from_axes(c, self.major_axis, across, self.start, self.end).ok()
            }
            5 | 6 if !self.is_full() => {
                let (_, t) = self.nearest_on_whole(to);
                let (start, end) = if index == 5 {
                    (t, self.end)
                } else {
                    (self.start, t)
                };
                let end = if end <= start { end + TAU } else { end };
                Ellipse::new(c, self.major_axis, self.radius_ratio, start, end).ok()
            }
            _ => None,
        }
    }

    /// The nearest point on the whole ellipse, ignoring where an arc ends.
    pub(crate) fn nearest_on_whole(&self, p: Point) -> (Point, f64) {
        let whole = Ellipse {
            start: 0.0,
            end: TAU,
            ..*self
        };
        whole.nearest(p)
    }

    /// The pieces left when the curve is cut at `cuts` (each moved to the
    /// nearest point of the curve), in order along it; `None` when nothing
    /// divides it. Cuts at an arc's ends and repeats are ignored. A full
    /// ellipse needs two cuts, as a circle does.
    pub fn split_at(&self, cuts: &[Point]) -> Option<Vec<Ellipse>> {
        let sweep = self.sweep();
        let mut at: Vec<f64> = cuts
            .iter()
            .map(|c| (self.nearest_on_whole(*c).1 - self.start).rem_euclid(TAU))
            .filter(|d| self.is_full() || (*d > PARAM_EPS && *d < sweep - PARAM_EPS))
            .collect();
        at.sort_by(f64::total_cmp);
        at.dedup_by(|a, b| (*a - *b).abs() < PARAM_EPS);
        let piece = |from: f64, to: f64| {
            Ellipse::new(
                self.center,
                self.major_axis,
                self.radius_ratio,
                self.start + from,
                self.start + to,
            )
            .ok()
        };
        if self.is_full() {
            // The first and last can be one place seen from either side of 0.
            if at.len() > 1 && at[0] + TAU - at[at.len() - 1] < PARAM_EPS {
                at.pop();
            }
            if at.len() < 2 {
                return None;
            }
            let n = at.len();
            return (0..n)
                .map(|i| {
                    let to = if i + 1 == n { at[0] + TAU } else { at[i + 1] };
                    piece(at[i], to)
                })
                .collect();
        }
        if at.is_empty() {
            return None;
        }
        let mut marks = vec![0.0];
        marks.extend(at);
        marks.push(sweep);
        marks.windows(2).map(|w| piece(w[0], w[1])).collect()
    }

    /// The ellipse under `map`, exactly. Any map that does not squash the
    /// plane flat takes an ellipse to an ellipse: its axes come from the
    /// mapped axes, and a parameter `t` becomes `t + δ`, or `δ - t` when the
    /// map reflects.
    pub fn mapped(&self, map: &Affine) -> Result<Ellipse, EllipseError> {
        let linear = |v: Point| vector(map.xx * v.x + map.xy * v.y, map.yx * v.x + map.yy * v.y);
        let (a, b) = (linear(self.major_axis), linear(self.minor_axis()));
        let det = map.determinant();
        if !det.is_finite() || det == 0.0 {
            return Err(EllipseError::NullMajorAxis);
        }
        // S = a·aᵀ + b·bᵀ: its eigenvalues are the squared semi-axes.
        let sxx = a.x * a.x + b.x * b.x;
        let syy = a.y * a.y + b.y * b.y;
        let sxy = a.x * a.y + b.x * b.y;
        let mean = (sxx + syy) / 2.0;
        let spread = ((sxx - syy) / 2.0).hypot(sxy);
        let big = mean + spread;
        // The product of the eigenvalues is (det · |a| · |b|)², so the small
        // one needs no subtraction.
        let area = det * self.major_radius() * self.minor_radius();
        let small = area * area / big;
        if !(big.is_finite() && small.is_finite() && big > 0.0) {
            return Err(EllipseError::NonFinite);
        }
        let direction = if spread <= 1e-12 * big {
            a
        } else if sxx >= syy {
            vector(big - syy, sxy)
        } else {
            vector(sxy, big - sxx)
        };
        let length = direction.x.hypot(direction.y);
        let mut unit = vector(direction.x / length, direction.y / length);
        let along = dot(unit, a);
        if along < 0.0 || (along == 0.0 && (unit.x < 0.0 || (unit.x == 0.0 && unit.y < 0.0))) {
            unit = vector(-unit.x, -unit.y);
        }
        let major = big.sqrt();
        let center = map.apply(self.center);
        let axis = vector(unit.x * major, unit.y * major);
        let ratio = (small.sqrt() / major).min(1.0);
        let whole = Ellipse::full(center, axis, ratio)?;
        if self.is_full() {
            return Ok(whole);
        }
        let delta = whole.parameter_of(Point::new(center.x + a.x, center.y + a.y));
        let (start, end) = if det > 0.0 {
            (self.start + delta, self.end + delta)
        } else {
            (delta - self.end, delta - self.start)
        };
        Ellipse::new(center, axis, ratio, start, end)
    }

    /// A circle (`start == end`) or circular arc as an ellipse.
    pub fn circular(
        center: Point,
        radius: f64,
        start: f64,
        end: f64,
    ) -> Result<Self, EllipseError> {
        let axis = vector(radius.abs(), 0.0);
        let sweep = crate::arc::sweep(start, end);
        if sweep >= TAU - PARAM_EPS {
            return Ellipse::full(center, axis, 1.0);
        }
        Ellipse::new(center, axis, 1.0, start, start + sweep)
    }
}
