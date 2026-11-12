//! What a hatch must satisfy to exist: finite numbers, counts within limits,
//! and edges that could be real. Cheap; it does not flatten anything.

use super::{
    HatchBoundary, HatchEdge, HatchEntity, HatchError, HatchFill, HatchLoop, HatchPattern,
    HatchSpline,
};
use crate::Point;

/// Most boundary loops one hatch may have.
pub const MAX_LOOPS: usize = 4096;
/// Most edges in one loop.
pub const MAX_EDGES: usize = 65_536;
/// Most polyline vertices or spline control points in one loop.
pub const MAX_VERTICES: usize = 1_000_000;
/// Most points and edges over a whole hatch.
pub const MAX_TOTAL: usize = 2_000_000;
/// Most definition lines in a pattern, dashes in a line, and seed points.
pub const MAX_PATTERN_LINES: usize = 64;
pub const MAX_DASHES: usize = 64;
pub const MAX_SEEDS: usize = 1024;
/// Most colour stops in a gradient.
pub const MAX_STOPS: usize = 8;
/// The largest coordinate a hatch may have.
const MAX_COORDINATE: f64 = 1e12;

fn sane(v: f64) -> bool {
    v.is_finite() && v.abs() <= MAX_COORDINATE
}

fn sane_point(p: Point) -> bool {
    sane(p.x) && sane(p.y)
}

impl HatchEntity {
    /// Every rule a hatch must keep.
    pub fn validate(&self) -> Result<(), HatchError> {
        if !sane(self.elevation) {
            return Err(HatchError::NonFinite);
        }
        if self.loops.len() > MAX_LOOPS {
            return Err(HatchError::TooLarge("boundaries"));
        }
        if self.seed_points.len() > MAX_SEEDS {
            return Err(HatchError::TooLarge("seed points"));
        }
        if !self.seed_points.iter().all(|p| sane_point(*p)) {
            return Err(HatchError::NonFinite);
        }
        let mut total = 0usize;
        for (i, l) in self.loops.iter().enumerate() {
            total = total.saturating_add(check_loop(i, l)?);
        }
        if total > MAX_TOTAL {
            return Err(HatchError::TooLarge("points and edges"));
        }
        match &self.fill {
            HatchFill::Pattern(p) => check_pattern(p),
            HatchFill::Solid => Ok(()),
            HatchFill::Gradient(g) => {
                if g.colors.len() > MAX_STOPS || !(sane(g.angle) && sane(g.shift) && sane(g.tint)) {
                    return Err(HatchError::NonFinite);
                }
                Ok(())
            }
        }
    }
}

fn check_loop(i: usize, l: &HatchLoop) -> Result<usize, HatchError> {
    match &l.boundary {
        HatchBoundary::Polyline(p) => {
            if p.vertices.len() > MAX_VERTICES {
                return Err(HatchError::TooLarge("vertices"));
            }
            if !(p.bulges.is_empty() || p.bulges.len() == p.vertices.len()) {
                return Err(HatchError::BadLoop(i, "bulges do not match the vertices"));
            }
            if !p.vertices.iter().all(|v| sane_point(*v)) || !p.bulges.iter().all(|b| sane(*b)) {
                return Err(HatchError::NonFinite);
            }
            Ok(p.vertices.len())
        }
        HatchBoundary::Edges(edges) => {
            if edges.len() > MAX_EDGES {
                return Err(HatchError::TooLarge("edges"));
            }
            let mut total = edges.len();
            for e in edges {
                total = total.saturating_add(check_edge(i, e)?);
            }
            Ok(total)
        }
    }
}

fn check_edge(i: usize, e: &HatchEdge) -> Result<usize, HatchError> {
    match e {
        HatchEdge::Line { start, end } => {
            if sane_point(*start) && sane_point(*end) {
                Ok(0)
            } else {
                Err(HatchError::NonFinite)
            }
        }
        HatchEdge::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            if !(sane_point(*center) && sane(*start_angle) && sane(*end_angle) && sane(*radius)) {
                return Err(HatchError::NonFinite);
            }
            if *radius <= 0.0 {
                return Err(HatchError::BadLoop(i, "an arc with no radius"));
            }
            Ok(0)
        }
        HatchEdge::EllipticArc {
            center,
            major_axis,
            minor_major_ratio,
            start_param,
            end_param,
            ..
        } => {
            let finite = sane_point(*center)
                && sane_point(*major_axis)
                && sane(*start_param)
                && sane(*end_param)
                && sane(*minor_major_ratio);
            if !finite {
                return Err(HatchError::NonFinite);
            }
            if *minor_major_ratio <= 0.0 || major_axis.x == 0.0 && major_axis.y == 0.0 {
                return Err(HatchError::BadLoop(i, "an ellipse with no size"));
            }
            Ok(0)
        }
        HatchEdge::Spline(s) => check_spline(i, s),
    }
}

fn check_spline(i: usize, s: &HatchSpline) -> Result<usize, HatchError> {
    let counts = [s.knots.len(), s.control_points.len(), s.fit_points.len()];
    if counts.iter().any(|c| *c > MAX_VERTICES) {
        return Err(HatchError::TooLarge("spline points"));
    }
    let finite = s.knots.iter().all(|k| sane(*k))
        && s.control_points.iter().all(|p| sane_point(*p))
        && s.fit_points.iter().all(|p| sane_point(*p))
        && s.weights.iter().all(|w| sane(*w))
        && [s.start_tangent, s.end_tangent]
            .iter()
            .flatten()
            .all(|t| sane_point(*t));
    if !finite {
        return Err(HatchError::NonFinite);
    }
    if !(s.weights.is_empty() || s.weights.len() == s.control_points.len()) {
        return Err(HatchError::BadLoop(i, "spline weights do not match"));
    }
    // One knot more than control points plus the degree, as in a SPLINE.
    if s.degree == 0 || s.knots.len() != s.control_points.len() + usize::from(s.degree) + 1 {
        return Err(HatchError::BadLoop(
            i,
            "spline knots do not match its control points",
        ));
    }
    Ok(counts.iter().sum())
}

fn check_pattern(p: &HatchPattern) -> Result<(), HatchError> {
    if p.lines.len() > MAX_PATTERN_LINES {
        return Err(HatchError::TooLarge("pattern lines"));
    }
    let numbers = [p.angle, p.scale_or_spacing, p.origin.x, p.origin.y];
    if !numbers.iter().all(|v| sane(*v)) {
        return Err(HatchError::NonFinite);
    }
    for l in &p.lines {
        if l.dashes.len() > MAX_DASHES {
            return Err(HatchError::TooLarge("dashes"));
        }
        let ok = sane(l.angle)
            && sane_point(l.base)
            && l.offset.iter().all(|v| sane(*v))
            && l.dashes.iter().all(|d| sane(*d));
        if !ok {
            return Err(HatchError::NonFinite);
        }
    }
    Ok(())
}
