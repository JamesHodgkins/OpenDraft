//! Boundary loops as geometry: exact bounds, and closed polygons for the
//! evaluator, with arcs and curves followed within a tolerance.

use std::f64::consts::TAU;

use super::{HatchBoundary, HatchEdge, HatchError, HatchLoop, HatchSpline};
use crate::{arc, bulge, Bounds, Ellipse, Point, Spline};

/// Most points one curve of a boundary may be followed with.
const MAX_CURVE_POINTS: usize = 8192;

impl HatchSpline {
    /// The spline this edge describes, or `None` if it could not exist.
    pub fn to_spline(&self) -> Option<Spline> {
        let weights = (!self.weights.is_empty()).then(|| self.weights.clone());
        Spline::new_nurbs(
            self.degree,
            self.control_points.clone(),
            self.knots.clone(),
            weights,
            self.periodic,
            self.periodic,
        )
        .ok()
    }
}

impl HatchEdge {
    /// Where the edge begins, in the direction the loop runs.
    pub fn start_point(&self) -> Option<Point> {
        Some(match self {
            HatchEdge::Line { start, .. } => *start,
            HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                ..
            } => arc::point_at(*center, *radius, *start_angle),
            HatchEdge::EllipticArc { .. } => {
                let (e, forward) = self.ellipse()?;
                if forward {
                    e.start_point()
                } else {
                    e.end_point()
                }
            }
            HatchEdge::Spline(s) => s.to_spline()?.start_point(),
        })
    }

    /// Where the edge ends.
    pub fn end_point(&self) -> Option<Point> {
        Some(match self {
            HatchEdge::Line { end, .. } => *end,
            HatchEdge::CircularArc {
                center,
                radius,
                end_angle,
                ..
            } => arc::point_at(*center, *radius, *end_angle),
            HatchEdge::EllipticArc { .. } => {
                let (e, forward) = self.ellipse()?;
                if forward {
                    e.end_point()
                } else {
                    e.start_point()
                }
            }
            HatchEdge::Spline(s) => s.to_spline()?.end_point(),
        })
    }

    /// An elliptic edge as a counter-clockwise ellipse, and whether the
    /// edge runs the same way (otherwise it runs from the ellipse's end back
    /// to its start).
    fn ellipse(&self) -> Option<(Ellipse, bool)> {
        let HatchEdge::EllipticArc {
            center,
            major_axis,
            minor_major_ratio,
            start_param,
            end_param,
            counter_clockwise,
        } = self
        else {
            return None;
        };
        let (from, to) = if *counter_clockwise {
            (*start_param, *end_param)
        } else {
            (*end_param, *start_param)
        };
        // Equal parameters are a whole turn, as for circular arcs.
        let to = if (to - from).rem_euclid(TAU) < 1e-12 {
            from + TAU
        } else {
            from + (to - from).rem_euclid(TAU)
        };
        let ellipse = Ellipse::new(*center, *major_axis, *minor_major_ratio, from, to).ok()?;
        Some((ellipse, *counter_clockwise))
    }

    /// The edge followed within `tolerance`, first point to last.
    pub fn flatten(&self, tolerance: f64) -> Option<Vec<Point>> {
        match self {
            HatchEdge::Line { start, end } => Some(vec![*start, *end]),
            HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                counter_clockwise,
            } => {
                if !radius.is_finite() || *radius <= 0.0 {
                    return None;
                }
                Some(if *counter_clockwise {
                    arc::flatten(*center, *radius, *start_angle, *end_angle, tolerance)
                } else {
                    let mut points =
                        arc::flatten(*center, *radius, *end_angle, *start_angle, tolerance);
                    points.reverse();
                    points
                })
            }
            HatchEdge::EllipticArc { .. } => {
                let (ellipse, forward) = self.ellipse()?;
                let mut points = ellipse.flatten(tolerance).ok()?;
                if !forward {
                    points.reverse();
                }
                Some(points)
            }
            HatchEdge::Spline(s) => s
                .to_spline()?
                .flatten_within(tolerance, MAX_CURVE_POINTS)
                .ok(),
        }
    }

    /// The box around the edge (a spline's is its control points').
    pub fn bounds(&self) -> Option<Bounds> {
        match self {
            HatchEdge::Line { start, end } => Some(Bounds::from_points(*start, *end)),
            HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                counter_clockwise,
            } => {
                let (a, b) = if *counter_clockwise {
                    (*start_angle, *end_angle)
                } else {
                    (*end_angle, *start_angle)
                };
                Some(arc::bounds(*center, radius.abs(), a, b))
            }
            HatchEdge::EllipticArc { .. } => Some(self.ellipse()?.0.bounds()),
            HatchEdge::Spline(s) => points_bounds(&s.control_points),
        }
    }
}

pub(super) fn points_bounds(points: &[Point]) -> Option<Bounds> {
    let (first, rest) = points.split_first()?;
    Some(
        rest.iter()
            .fold(Bounds::from_points(*first, *first), |b, p| {
                b.union(Bounds::from_points(*p, *p))
            }),
    )
}

impl HatchLoop {
    /// The box around the loop, exact for lines and arcs.
    pub fn bounds(&self) -> Option<Bounds> {
        match &self.boundary {
            HatchBoundary::Polyline(p) => {
                let n = p.vertices.len();
                (0..n)
                    .map(|i| {
                        let (a, b) = (p.vertices[i], p.vertices[(i + 1) % n]);
                        bulge::bounds(a, b, bulge::at(&p.bulges, i))
                    })
                    .reduce(Bounds::union)
            }
            HatchBoundary::Edges(edges) => edges
                .iter()
                .filter_map(HatchEdge::bounds)
                .reduce(Bounds::union),
        }
    }

    /// The loop as a polygon (not repeating its first point), curves followed
    /// within `tolerance`. `index` names the loop in an error. A loop whose
    /// end is further than `close` from its start is open.
    pub fn contour(
        &self,
        index: usize,
        tolerance: f64,
        close: f64,
    ) -> Result<Vec<Point>, HatchError> {
        let mut points = match &self.boundary {
            HatchBoundary::Polyline(p) => {
                if p.vertices.len() < 2 {
                    return Err(HatchError::BadLoop(index, "fewer than two vertices"));
                }
                // A boundary always closes itself, so even two vertices make
                // two segments (AutoCAD writes a circle as two bulge-1 arcs).
                let n = p.vertices.len();
                let mut path = Vec::with_capacity(n);
                for i in 0..n {
                    let (a, b) = (p.vertices[i], p.vertices[(i + 1) % n]);
                    let piece = bulge::flatten(a, b, bulge::at(&p.bulges, i), tolerance);
                    path.extend(piece.iter().take(piece.len() - 1).copied());
                }
                path
            }
            HatchBoundary::Edges(edges) => {
                if edges.is_empty() {
                    return Err(HatchError::BadLoop(index, "no edges"));
                }
                let mut path: Vec<Point> = Vec::new();
                for edge in edges {
                    let piece = edge
                        .flatten(tolerance)
                        .ok_or(HatchError::BadLoop(index, "an edge that could not exist"))?;
                    if let (Some(last), Some(first)) = (path.last(), piece.first()) {
                        if distance(*last, *first) > close {
                            return Err(HatchError::OpenLoop(index));
                        }
                        path.pop();
                    }
                    path.extend(piece);
                }
                path
            }
        };
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            if distance(*first, *last) <= close {
                points.pop();
            } else if matches!(self.boundary, HatchBoundary::Edges(_)) {
                return Err(HatchError::OpenLoop(index));
            }
        }
        if points.len() < 3 {
            return Err(HatchError::BadLoop(index, "fewer than three points"));
        }
        if points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite())) {
            return Err(HatchError::NonFinite);
        }
        Ok(points)
    }
}

pub(super) fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}
