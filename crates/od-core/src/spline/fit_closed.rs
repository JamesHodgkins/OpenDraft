//! A closed cubic spline through fit points, as SPLINE's Fit method makes it
//! when the curve is closed.
//!
//! AutoCAD stores it clamped, like the open fit: the points followed by the
//! first point again give `n + 1` knots' worth of equations, and `n + 3`
//! control points, the first and last of them at the first point. The two
//! extra equations (in place of the natural ends) make the first and second
//! derivatives at the start equal those at the end, so the curve is smooth
//! where it closes.
//!
//! The matrix is banded once the unknowns are taken alternately from the two
//! ends inwards, so the same band solver as the open fit is used.

use super::fit::{basis, parameters, solve, Band};
use super::MAX_CONTROL_POINTS;
use super::{check_point, distance, KnotParameterization, Spline, SplineError, SplineFitData};
use crate::Point;

const DEGREE: usize = 3;
/// How far from the diagonal the reordered matrix reaches (at most 6).
const REACH: usize = 9;

impl Spline {
    /// The closed cubic spline through `points` (at least three, none
    /// repeating the one before), smooth where it closes. A last point on the
    /// first, as files store a closed fit, is taken as the closing point and
    /// left out. Keeps the points as its fit data.
    pub fn from_closed_fit_points(
        mut points: Vec<Point>,
        parameterization: KnotParameterization,
    ) -> Result<Self, SplineError> {
        for point in &points {
            check_point(*point)?;
        }
        let scale = points
            .iter()
            .fold(1.0f64, |m, q| m.max(q.x.abs()).max(q.y.abs()));
        let near = 1e-10 * scale;
        if points.len() > 3 && distance(points[0], points[points.len() - 1]) <= near {
            points.pop();
        }
        let count = points.len();
        if count < 3 {
            return Err(SplineError::BadFitData);
        }
        if count + 3 > MAX_CONTROL_POINTS {
            return Err(SplineError::ResourceLimit);
        }
        let mut ring = points.clone();
        ring.push(points[0]);
        if let Some(i) = (1..ring.len()).find(|&i| distance(ring[i], ring[i - 1]) <= near) {
            return Err(SplineError::RepeatedPoint(i));
        }
        let (params, span) = parameters(&ring, parameterization)?;
        let m = ring.len();
        let size = m + 2;
        let last = size - 1;
        let mut knots = vec![0.0; DEGREE + 1];
        knots.extend_from_slice(&params[1..m - 1]);
        knots.extend([1.0; DEGREE + 1]);

        // Unknowns are control points 1..last; the ends are the first point.
        // Control point `j` is the `pos(j)`th unknown, from the two ends in.
        let pos = |j: usize| 2 * (j.min(last - j) - 1) + usize::from(j > last - j);
        let first = ring[0];
        let mut band = Band::new(m, REACH, REACH);
        let mut b = vec![[0.0; 2]; m];

        // Equal first derivatives: (Q1 - Q0)/u4 = (Q_last - Q_last-1)/d4.
        let (u4, u5) = (knots[4], knots[5]);
        let (d4, d5) = (1.0 - knots[last], 1.0 - knots[last - 1]);
        band.add(0, pos(1), 1.0 / u4)?;
        band.add(0, pos(last - 1), 1.0 / d4)?;
        let rhs = 1.0 / u4 + 1.0 / d4;
        b[0] = [first.x * rhs, first.y * rhs];

        // Equal second derivatives, divided through by the start's factor.
        let ratio = (u4 * u4 * u5) / (d4 * d4 * d5);
        band.add(1, pos(1), -(u4 + u5))?;
        band.add(1, pos(2), u4)?;
        band.add(1, pos(last - 1), ratio * (d4 + d5))?;
        band.add(1, pos(last - 2), -ratio * d4)?;
        let rhs = ratio * d5 - u5;
        b[1] = [first.x * rhs, first.y * rhs];

        // Through each inner point, at its knot; the row sits at the
        // unknown it is centred on.
        for (i, q) in ring.iter().enumerate().take(m - 1).skip(1) {
            let k = i + DEGREE;
            let row = pos(i + 1);
            b[row] = [q.x, q.y];
            for (t, v) in basis(&knots, k, knots[k]).iter().enumerate() {
                let column = i + t;
                if column == 0 || column == last {
                    b[row][0] -= v * first.x;
                    b[row][1] -= v * first.y;
                } else {
                    band.add(row, pos(column), *v)?;
                }
            }
        }

        let solved = solve(&band, &b)?;
        let control: Vec<Point> = (0..size)
            .map(|j| {
                if j == 0 || j == last {
                    first
                } else {
                    let v = solved[pos(j)];
                    Point::new(v[0], v[1])
                }
            })
            .collect();
        let mut spline = Spline::new_nurbs(DEGREE as u8, control, knots, None, true, true)?;
        let allowed = (1e-9 * scale).max(64.0 * f64::EPSILON * scale);
        let misses = params
            .iter()
            .zip(&ring)
            .any(|(t, q)| distance(spline.point_at(*t), *q) > allowed);
        if misses {
            return Err(SplineError::Degenerate);
        }
        spline.knots.iter_mut().for_each(|k| *k *= span);
        spline.fit = Some(SplineFitData::exact(points, parameterization));
        Ok(spline)
    }
}
