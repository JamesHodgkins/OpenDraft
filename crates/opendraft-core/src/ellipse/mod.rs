//! Ellipses and elliptical arcs, as AutoCAD's `ELLIPSE` entity.
//!
//! An ellipse is its centre, the vector from the centre to the end of its
//! major axis, the ratio of the minor radius to the major one, and a
//! counter-clockwise interval of the *parameter* `t` in
//!
//! ```text
//! p(t) = center + major_axis·cos(t) + minor_axis·sin(t)
//! ```
//!
//! where the minor axis is the major one turned a quarter counter-clockwise
//! and scaled by the ratio. `t = 0` is the end of the major axis. The
//! parameter is not the angle seen from the centre (it is the eccentric
//! anomaly); [`Ellipse::parameter_at_angle`] converts. A full ellipse runs
//! from 0 to 2π. These values are the authority: points, outlines and bounds
//! are worked out from them and never stored.
//!
//! [`Ellipse::new`] is where values are checked and put in canonical form,
//! so every `Ellipse` built through it (and every transform of one) is valid.

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::fmt;

use crate::{Bounds, Point};

mod cross;
mod edit;
mod flatten;
pub(crate) mod join;
mod nearest;
mod transform;

/// The smallest minor-to-major radius ratio, as in AutoCAD.
pub const MIN_RATIO: f64 = 1e-6;
/// The smallest parameter span of an elliptical arc, as in AutoCAD.
pub const MIN_SWEEP: f64 = 1e-6;
/// The largest coordinate or radius accepted (as for imported drawings).
pub const MAX_COORDINATE: f64 = 1e12;
/// Parameters this close are the same place; a span this close to a full
/// turn is a full ellipse.
const PARAM_EPS: f64 = 1e-9;

/// A full ellipse or an elliptical arc in the drawing plane. See the module
/// notes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    pub center: Point,
    /// From the centre to the end of the major axis; its length is the major
    /// radius.
    pub major_axis: Point,
    /// Minor radius over major radius, in `[MIN_RATIO, 1]`.
    pub radius_ratio: f64,
    /// Start parameter, in `[0, 2π)`.
    pub start: f64,
    /// End parameter: more than `start`, at most `start + 2π`.
    pub end: f64,
}

/// Why values make no ellipse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EllipseError {
    /// A value is not a finite number, or is beyond [`MAX_COORDINATE`].
    NonFinite,
    NullMajorAxis,
    RatioOutOfRange,
    /// The arc is shorter than [`MIN_SWEEP`].
    EmptySweep,
    /// The arc turns more than once.
    SweepTooLarge,
    /// Stored values that [`Ellipse::new`] would have put another way (a
    /// start outside `[0, 2π)`, a ratio over 1).
    NotCanonical,
    /// Following the curve to the tolerance asked for needs more work than
    /// allowed.
    ResourceLimit,
}

impl fmt::Display for EllipseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "a value is not a finite number or is too large",
            Self::NullMajorAxis => "the axis has no length",
            Self::RatioOutOfRange => {
                "the minor-to-major radius ratio must be between 0.000001 and 1"
            }
            Self::EmptySweep => "those values make no elliptical arc",
            Self::SweepTooLarge => "the arc turns more than once",
            Self::NotCanonical => "the arc's parameters are not in canonical form",
            Self::ResourceLimit => "the curve needs more points than allowed",
        })
    }
}

impl std::error::Error for EllipseError {}

fn sane(v: f64) -> bool {
    v.is_finite() && v.abs() <= MAX_COORDINATE
}

/// Below this a length is zero, for coordinates of this size.
fn epsilon(scale: f64) -> f64 {
    64.0 * f64::EPSILON * scale.max(1.0)
}

fn vector(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

/// `v` turned a quarter counter-clockwise.
fn left(v: Point) -> Point {
    vector(-v.y, v.x)
}

impl Ellipse {
    /// An ellipse or elliptical arc, checked and in canonical form: the start
    /// is brought into `[0, 2π)`, an end below the start goes round once
    /// more, and a span of a whole turn becomes a full ellipse (0 to 2π).
    /// A ratio a hair over 1 from rounding is taken as 1.
    pub fn new(
        center: Point,
        major_axis: Point,
        radius_ratio: f64,
        start: f64,
        end: f64,
    ) -> Result<Self, EllipseError> {
        let values = [center.x, center.y, major_axis.x, major_axis.y];
        if !values.iter().all(|v| sane(*v)) || !start.is_finite() || !end.is_finite() {
            return Err(EllipseError::NonFinite);
        }
        let a = major_axis.x.hypot(major_axis.y);
        let scale = center.x.abs().max(center.y.abs()).max(a);
        if a <= epsilon(scale) {
            return Err(EllipseError::NullMajorAxis);
        }
        if !(radius_ratio.is_finite() && (MIN_RATIO..=1.0 + PARAM_EPS).contains(&radius_ratio)) {
            return Err(EllipseError::RatioOutOfRange);
        }
        let mut span = end - start;
        if span.abs() > TAU + PARAM_EPS {
            return Err(EllipseError::SweepTooLarge);
        }
        if span.abs() < MIN_SWEEP {
            return Err(EllipseError::EmptySweep);
        }
        if span < 0.0 {
            span += TAU;
        }
        let (start, end) = if span >= TAU - PARAM_EPS {
            (0.0, TAU)
        } else if span <= MIN_SWEEP {
            return Err(EllipseError::EmptySweep);
        } else {
            let s = start.rem_euclid(TAU);
            let s = if s >= TAU { 0.0 } else { s };
            (s, s + span)
        };
        Ok(Self {
            center,
            major_axis,
            radius_ratio: radius_ratio.min(1.0),
            start,
            end,
        })
    }

    /// A full ellipse.
    pub fn full(center: Point, major_axis: Point, radius_ratio: f64) -> Result<Self, EllipseError> {
        Self::new(center, major_axis, radius_ratio, 0.0, TAU)
    }

    /// The ellipse with semi-axes `first` and the one a quarter turn
    /// counter-clockwise from it of length `other`, either of which may be
    /// the longer; `start` and `end` are parameters measured from `first`.
    /// When `other` is longer it becomes the major axis and the parameters
    /// move a quarter turn back, so the arc keeps the same points.
    pub fn from_axes(
        center: Point,
        first: Point,
        other: f64,
        start: f64,
        end: f64,
    ) -> Result<Self, EllipseError> {
        let length = first.x.hypot(first.y);
        if !(length.is_finite() && other.is_finite()) {
            return Err(EllipseError::NonFinite);
        }
        if length <= 0.0 {
            return Err(EllipseError::NullMajorAxis);
        }
        let other = other.abs();
        if other <= length {
            return Self::new(center, first, other / length, start, end);
        }
        let turned = left(first);
        let major = vector(turned.x / length * other, turned.y / length * other);
        // first·cos t + major·sin t = major·cos(t - π/2) - first·sin(t - π/2),
        // and the new minor axis (the major one turned a quarter) is -first.
        let shift = if end - start >= TAU - PARAM_EPS {
            0.0
        } else {
            FRAC_PI_2
        };
        Self::new(center, major, length / other, start - shift, end - shift)
    }

    pub fn major_radius(&self) -> f64 {
        self.major_axis.x.hypot(self.major_axis.y)
    }

    pub fn minor_radius(&self) -> f64 {
        self.major_radius() * self.radius_ratio
    }

    /// From the centre to the end of the minor axis (at `t = π/2`).
    pub fn minor_axis(&self) -> Point {
        let l = left(self.major_axis);
        vector(l.x * self.radius_ratio, l.y * self.radius_ratio)
    }

    /// Direction of the major axis, radians counter-clockwise from east.
    pub fn rotation(&self) -> f64 {
        self.major_axis.y.atan2(self.major_axis.x)
    }

    /// The parameter span, in `(0, 2π]`.
    pub fn sweep(&self) -> f64 {
        self.end - self.start
    }

    pub fn is_full(&self) -> bool {
        self.sweep() >= TAU - PARAM_EPS
    }

    pub fn point_at(&self, t: f64) -> Point {
        let (sin, cos) = t.sin_cos();
        let b = self.minor_axis();
        Point::new(
            self.center.x + self.major_axis.x * cos + b.x * sin,
            self.center.y + self.major_axis.y * cos + b.y * sin,
        )
    }

    /// The derivative of [`point_at`](Self::point_at): the direction the
    /// curve runs at `t`.
    pub fn tangent_at(&self, t: f64) -> Point {
        let (sin, cos) = t.sin_cos();
        let b = self.minor_axis();
        vector(
            -self.major_axis.x * sin + b.x * cos,
            -self.major_axis.y * sin + b.y * cos,
        )
    }

    pub fn start_point(&self) -> Point {
        self.point_at(self.start)
    }

    pub fn end_point(&self) -> Point {
        self.point_at(self.end)
    }

    /// The point halfway through the parameter span.
    pub fn mid_point(&self) -> Point {
        self.point_at(self.start + self.sweep() / 2.0)
    }

    /// Whether parameter `t` lies on the arc.
    pub fn contains(&self, t: f64) -> bool {
        (t - self.start).rem_euclid(TAU) <= self.sweep() + PARAM_EPS
    }

    /// The parameter of a point on the ellipse, in `[0, 2π)`. A point off it
    /// gets the parameter of the point in the same direction once the
    /// ellipse is stretched into a circle; use [`nearest`](Self::nearest) to
    /// project onto the curve first.
    pub fn parameter_of(&self, p: Point) -> f64 {
        let a = self.major_radius();
        let b = a * self.radius_ratio;
        let u = vector(self.major_axis.x / a, self.major_axis.y / a);
        let q = vector(p.x - self.center.x, p.y - self.center.y);
        (dot(q, left(u)) / b).atan2(dot(q, u) / a).rem_euclid(TAU)
    }

    /// The parameter where the ray from the centre at `angle` (radians
    /// counter-clockwise from east) meets the ellipse.
    pub fn parameter_at_angle(&self, angle: f64) -> f64 {
        let (y, x) = (angle - self.rotation()).sin_cos();
        (y / self.radius_ratio).atan2(x).rem_euclid(TAU)
    }

    /// The direction from the centre to the point at parameter `t`, radians
    /// counter-clockwise from east.
    pub fn angle_at(&self, t: f64) -> f64 {
        let (sin, cos) = t.sin_cos();
        (sin * self.radius_ratio).atan2(cos) + self.rotation()
    }

    /// Calls `f` with each point that decides the box: the ends of an arc,
    /// and the points where the curve runs level or upright, when the arc
    /// reaches them.
    fn each_extreme(&self, mut f: impl FnMut(Point)) {
        let full = self.is_full();
        if !full {
            f(self.start_point());
            f(self.end_point());
        }
        let (a, b) = (self.major_axis, self.minor_axis());
        // x(t) is extreme where -a.x·sin t + b.x·cos t = 0, and likewise y.
        for base in [b.x.atan2(a.x), b.y.atan2(a.y)] {
            for t in [base, base + PI] {
                if full || self.contains(t) {
                    f(self.point_at(t));
                }
            }
        }
    }

    /// The points that decide the box (see [`bounds`](Self::bounds)): what a
    /// selection window must hold to hold the whole curve.
    pub fn extreme_points(&self) -> Vec<Point> {
        let mut points = Vec::with_capacity(6);
        self.each_extreme(|p| points.push(p));
        points
    }

    /// The box around the curve, exactly.
    pub fn bounds(&self) -> Bounds {
        if self.is_full() {
            // x(t) = c.x + a.x·cos t + b.x·sin t swings by √(a.x² + b.x²).
            let (a, b) = (self.major_axis, self.minor_axis());
            let (w, h) = (a.x.hypot(b.x), a.y.hypot(b.y));
            let c = self.center;
            return Bounds::from_points(Point::new(c.x - w, c.y - h), Point::new(c.x + w, c.y + h));
        }
        let mut found: Option<Bounds> = None;
        self.each_extreme(|p| {
            let one = Bounds::from_points(p, p);
            found = Some(found.map_or(one, |b| b.union(one)));
        });
        found.unwrap_or_else(|| Bounds::from_points(self.center, self.center))
    }

    /// The area inside a full ellipse, or for an arc the area between it
    /// and its chord (what AREA measures for an open object). An affine map
    /// of a circular segment: `a·b·(Δt - sin Δt) / 2`.
    pub fn area(&self) -> f64 {
        let ab = self.major_radius() * self.minor_radius();
        if self.is_full() {
            PI * ab
        } else {
            ab * (self.sweep() - self.sweep().sin()) / 2.0
        }
    }
}

#[cfg(test)]
mod tests;
