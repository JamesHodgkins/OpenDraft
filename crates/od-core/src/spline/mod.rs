//! Splines, as AutoCAD's `SPLINE` entity: a planar non-uniform rational
//! B-spline (NURBS).
//!
//! A spline is its degree `p`, control points `P_i`, a full knot vector
//! (`control_points.len() + p + 1` values) and optional positive weights
//! `w_i` (none means every weight is 1):
//!
//! ```text
//! C(u) = Σ N_i,p(u)·w_i·P_i / Σ N_i,p(u)·w_i,   u in [U_p, U_n+1]
//! ```
//!
//! These values are the authority. A fit spline also keeps the points it was
//! fitted through ([`SplineFitData`]), as AutoCAD does, but the control
//! frame always decides the curve: nothing is refitted on loading, drawing
//! or saving. Points, outlines, bounds and lengths are worked out from the
//! control frame and never stored.
//!
//! [`Spline::new_nurbs`], [`Spline::from_fit_points`] and
//! [`Spline::from_control_points`] check what they build; files and edits
//! call [`Spline::validate`] on anything else. Evaluating a valid spline
//! cannot fail: its weights are positive, so the denominator is too.

use std::fmt;

use crate::{Bounds, Point};

mod eval;
mod fit;
mod fit_closed;
mod flatten;
mod query;
mod snap;
mod transform;

pub use flatten::{MAX_PLOT_POINTS, MAX_POINTS};

/// The highest degree accepted, as AutoCAD's database.
pub const MAX_DEGREE: u8 = 25;
/// The highest degree the SPLINE command offers.
pub const MAX_COMMAND_DEGREE: u8 = 10;
/// The most control points a spline may have.
pub const MAX_CONTROL_POINTS: usize = 1_000_000;
/// The largest coordinate accepted (as for imported drawings).
pub const MAX_COORDINATE: f64 = 1e12;
/// The range a weight must lie in.
pub const WEIGHT_RANGE: (f64, f64) = (1e-12, 1e12);
/// AutoCAD's default knot and control-point tolerances (DXF 42 and 43).
pub const DEFAULT_KNOT_TOLERANCE: f64 = 1e-7;
pub const DEFAULT_CONTROL_TOLERANCE: f64 = 1e-7;
/// AutoCAD's default fit tolerance (DXF 44).
pub const DEFAULT_FIT_TOLERANCE: f64 = 1e-10;

/// How a fit spline spaces its knots between fit points (`SPLKNOTS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnotParameterization {
    /// By the distance between points.
    Chord,
    /// By the square root of that distance (centripetal).
    SqrtChord,
    /// Evenly.
    Uniform,
}

/// The points a spline was fitted through, kept as AutoCAD keeps them.
#[derive(Clone, Debug, PartialEq)]
pub struct SplineFitData {
    pub points: Vec<Point>,
    /// How far the curve may pass from the points; 0 passes through them.
    pub tolerance: f64,
    /// Direction vectors at the ends, when given.
    pub start_tangent: Option<Point>,
    pub end_tangent: Option<Point>,
    /// How the knots were spaced, when known.
    pub parameterization: Option<KnotParameterization>,
}

/// A spline. See the module notes.
#[derive(Clone, Debug, PartialEq)]
pub struct Spline {
    pub degree: u8,
    pub control_points: Vec<Point>,
    /// The full knot vector, non-decreasing.
    pub knots: Vec<f64>,
    /// One positive weight per control point; `None` when all are 1.
    pub weights: Option<Vec<f64>>,
    pub closed: bool,
    /// Periodic (smooth all the way round); implies `closed`.
    pub periodic: bool,
    /// Knots closer than this count as one (DXF 42).
    pub knot_tolerance: f64,
    /// DXF 43, kept as read.
    pub control_point_tolerance: f64,
    pub fit: Option<SplineFitData>,
}

/// Why values make no spline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplineError {
    /// A value is not a finite number.
    NonFinite,
    /// A coordinate is beyond [`MAX_COORDINATE`].
    CoordinateOutOfRange,
    DegreeOutOfRange,
    /// Fewer than `degree + 1` control points.
    TooFewControlPoints,
    WrongKnotCount,
    KnotsDecrease,
    /// A knot repeats more often than the degree allows.
    KnotMultiplicity,
    /// The curve's parameter range has no length.
    EmptyDomain,
    WrongWeightCount,
    NonPositiveWeight,
    WeightOutOfRange,
    PeriodicNotClosed,
    /// Marked closed but the ends do not meet.
    ClosureMismatch,
    BadFitData,
    ZeroTangent,
    /// Fit point `n` (counting from 0) repeats the one before it.
    RepeatedPoint(usize),
    /// Every control point is in the same place, or the fit has no
    /// solution.
    Degenerate,
    /// More points or work than allowed.
    ResourceLimit,
}

impl fmt::Display for SplineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => f.write_str("a value is not a finite number"),
            Self::CoordinateOutOfRange => f.write_str("a coordinate is too large"),
            Self::DegreeOutOfRange => write!(f, "the degree must be 1 to {MAX_DEGREE}"),
            Self::TooFewControlPoints => {
                f.write_str("there are too few control points for the degree")
            }
            Self::WrongKnotCount => f.write_str("the number of knots does not match"),
            Self::KnotsDecrease => f.write_str("the knots decrease"),
            Self::KnotMultiplicity => f.write_str("a knot repeats too often"),
            Self::EmptyDomain => f.write_str("the knots leave no curve"),
            Self::WrongWeightCount => f.write_str("the number of weights does not match"),
            Self::NonPositiveWeight => f.write_str("a weight is not positive"),
            Self::WeightOutOfRange => f.write_str("a weight is too small or too large"),
            Self::PeriodicNotClosed => f.write_str("a periodic spline must be closed"),
            Self::ClosureMismatch => f.write_str("it is closed but its ends do not meet"),
            Self::BadFitData => f.write_str("its fit data is not valid"),
            Self::ZeroTangent => f.write_str("an end tangent has no length"),
            Self::RepeatedPoint(n) => write!(f, "point {} repeats the one before it", n + 1),
            Self::Degenerate => f.write_str("the points do not make a curve"),
            Self::ResourceLimit => f.write_str("the curve needs more work than allowed"),
        }
    }
}

impl std::error::Error for SplineError {}

/// Below this a length is zero, for coordinates of this size.
fn epsilon(scale: f64) -> f64 {
    64.0 * f64::EPSILON * scale.max(1.0)
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn check_point(p: Point) -> Result<(), SplineError> {
    if !(p.x.is_finite() && p.y.is_finite()) {
        Err(SplineError::NonFinite)
    } else if p.x.abs() > MAX_COORDINATE || p.y.abs() > MAX_COORDINATE {
        Err(SplineError::CoordinateOutOfRange)
    } else {
        Ok(())
    }
}

impl Spline {
    /// A spline from its control frame, checked. Weights that are all 1 are
    /// dropped; the tolerances are AutoCAD's defaults and there is no fit
    /// data.
    pub fn new_nurbs(
        degree: u8,
        control_points: Vec<Point>,
        knots: Vec<f64>,
        weights: Option<Vec<f64>>,
        closed: bool,
        periodic: bool,
    ) -> Result<Self, SplineError> {
        let weights = weights.filter(|w| w.iter().any(|v| *v != 1.0));
        let spline = Self {
            degree,
            control_points,
            knots,
            weights,
            closed,
            periodic,
            knot_tolerance: DEFAULT_KNOT_TOLERANCE,
            control_point_tolerance: DEFAULT_CONTROL_TOLERANCE,
            fit: None,
        };
        spline.validate()?;
        Ok(spline)
    }

    /// An open spline of `degree` with these control points, its knots even
    /// and clamped so it starts at the first point and ends at the last (what
    /// SPLINE's CV method makes).
    pub fn from_control_points(degree: u8, points: Vec<Point>) -> Result<Self, SplineError> {
        let p = usize::from(degree);
        let count = points.len();
        if !(1..=MAX_DEGREE).contains(&degree) {
            return Err(SplineError::DegreeOutOfRange);
        }
        if count <= p {
            return Err(SplineError::TooFewControlPoints);
        }
        let spans = count - p;
        let mut knots = vec![0.0; p + 1];
        knots.extend((1..spans).map(|i| i as f64 / spans as f64));
        knots.extend(std::iter::repeat_n(1.0, p + 1));
        Self::new_nurbs(degree, points, knots, None, false, false)
    }

    /// Every rule a spline must keep; see [`SplineError`].
    pub fn validate(&self) -> Result<(), SplineError> {
        let p = usize::from(self.degree);
        let count = self.control_points.len();
        if !(1..=MAX_DEGREE).contains(&self.degree) {
            return Err(SplineError::DegreeOutOfRange);
        }
        if count <= p {
            return Err(SplineError::TooFewControlPoints);
        }
        if count > MAX_CONTROL_POINTS {
            return Err(SplineError::ResourceLimit);
        }
        if self.knots.len() != count + p + 1 {
            return Err(SplineError::WrongKnotCount);
        }
        let tolerances = [self.knot_tolerance, self.control_point_tolerance];
        if !tolerances.iter().all(|t| t.is_finite() && *t >= 0.0) {
            return Err(SplineError::NonFinite);
        }
        for point in &self.control_points {
            check_point(*point)?;
        }
        self.validate_knots()?;
        if let Some(weights) = &self.weights {
            if weights.len() != count {
                return Err(SplineError::WrongWeightCount);
            }
            for w in weights {
                if !w.is_finite() {
                    return Err(SplineError::NonFinite);
                }
                if *w <= 0.0 {
                    return Err(SplineError::NonPositiveWeight);
                }
                if !(WEIGHT_RANGE.0..=WEIGHT_RANGE.1).contains(w) {
                    return Err(SplineError::WeightOutOfRange);
                }
            }
        }
        let first = self.control_points[0];
        let scale = self.scale();
        if self
            .control_points
            .iter()
            .all(|q| distance(*q, first) <= epsilon(scale))
        {
            return Err(SplineError::Degenerate);
        }
        if self.periodic && !self.closed {
            return Err(SplineError::PeriodicNotClosed);
        }
        if self.closed {
            let gap = distance(self.start_point(), self.end_point());
            if gap > self.control_point_tolerance.max(1e-9 * scale) {
                return Err(SplineError::ClosureMismatch);
            }
        }
        if let Some(fit) = &self.fit {
            fit.validate()?;
        }
        Ok(())
    }

    fn validate_knots(&self) -> Result<(), SplineError> {
        let p = usize::from(self.degree);
        let knots = &self.knots;
        if !knots.iter().all(|k| k.is_finite()) {
            return Err(SplineError::NonFinite);
        }
        if knots.windows(2).any(|w| w[1] < w[0]) {
            return Err(SplineError::KnotsDecrease);
        }
        let tolerance = self.knot_tolerance;
        let (first, last) = (knots[0], knots[knots.len() - 1]);
        // Runs of knots within the tolerance of the run's first count as one.
        let mut start = 0;
        while start < knots.len() {
            let mut end = start + 1;
            while end < knots.len() && knots[end] - knots[start] <= tolerance {
                end += 1;
            }
            let at_end = knots[start] - first <= tolerance || last - knots[end - 1] <= tolerance;
            let allowed = if at_end { p + 1 } else { p };
            if end - start > allowed {
                return Err(SplineError::KnotMultiplicity);
            }
            start = end;
        }
        let count = self.control_points.len();
        if knots[count] - knots[p] <= tolerance {
            return Err(SplineError::EmptyDomain);
        }
        Ok(())
    }

    /// The size of the coordinates, for tolerances.
    fn scale(&self) -> f64 {
        self.control_points
            .iter()
            .fold(0.0f64, |m, q| m.max(q.x.abs()).max(q.y.abs()))
    }

    pub fn is_rational(&self) -> bool {
        self.weights.is_some()
    }

    /// The weight of control point `i` (1 when there are none).
    pub fn weight(&self, i: usize) -> f64 {
        self.weights
            .as_ref()
            .and_then(|w| w.get(i).copied())
            .unwrap_or(1.0)
    }

    /// The parameter range the curve runs over, `[U_p, U_n+1]`.
    pub fn domain(&self) -> (f64, f64) {
        let p = usize::from(self.degree);
        let count = self.control_points.len();
        match (self.knots.get(p), self.knots.get(count)) {
            (Some(a), Some(b)) => (*a, *b),
            _ => (0.0, 0.0),
        }
    }

    pub fn start_point(&self) -> Point {
        self.point_at(self.domain().0)
    }

    pub fn end_point(&self) -> Point {
        self.point_at(self.domain().1)
    }

    /// The box around the control points, which holds the curve (its
    /// weights are positive). Safe for culling; not tight.
    pub fn bounds(&self) -> Bounds {
        let first = self.control_points.first().copied().unwrap_or_default();
        self.control_points
            .iter()
            .fold(Bounds::from_points(first, first), |b, q| {
                b.union(Bounds::from_points(*q, *q))
            })
    }
}

impl SplineFitData {
    /// Fit data for points passed through exactly.
    pub fn exact(points: Vec<Point>, parameterization: KnotParameterization) -> Self {
        Self {
            points,
            tolerance: 0.0,
            start_tangent: None,
            end_tangent: None,
            parameterization: Some(parameterization),
        }
    }

    pub fn validate(&self) -> Result<(), SplineError> {
        if self.points.len() < 2 || self.points.len() > MAX_CONTROL_POINTS {
            return Err(SplineError::BadFitData);
        }
        for point in &self.points {
            check_point(*point)?;
        }
        if !(self.tolerance.is_finite() && self.tolerance >= 0.0) {
            return Err(SplineError::BadFitData);
        }
        for tangent in [self.start_tangent, self.end_tangent].into_iter().flatten() {
            if !(tangent.x.is_finite() && tangent.y.is_finite()) {
                return Err(SplineError::NonFinite);
            }
            if tangent.x == 0.0 && tangent.y == 0.0 {
                return Err(SplineError::ZeroTangent);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod fit_tests;
#[cfg(test)]
mod tests;
