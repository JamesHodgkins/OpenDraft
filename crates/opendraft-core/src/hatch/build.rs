//! Boundary loops from selected objects (HATCH's Select Objects).
//!
//! Circles, rectangles, closed polylines, whole ellipses and closed splines
//! are loops by themselves. Lines, arcs, open polylines, elliptical arcs and
//! open splines are joined end to end into loops; a chain that does not close
//! is an error, and the user is told which objects do not.

use std::f64::consts::TAU;

use super::{
    BoundarySource, HatchBoundary, HatchEdge, HatchLoop, HatchLoopFlags, HatchPolyline, HatchSpline,
};
use crate::{bulge, Entity, Point, Spline};

/// Why objects make no boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// None of the objects can be a boundary.
    Nothing,
    /// A chain of objects that does not come back to where it began; the
    /// number of its objects.
    Open(usize),
    /// More loops than a hatch may have.
    TooMany,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nothing => write!(f, "none of the objects can bound a hatch"),
            Self::Open(n) => write!(
                f,
                "{n} of the objects do not form a closed boundary (the ends do not meet)"
            ),
            Self::TooMany => write!(f, "too many boundaries"),
        }
    }
}

fn edge_of_spline(s: &Spline) -> HatchEdge {
    HatchEdge::Spline(HatchSpline {
        degree: s.degree,
        periodic: s.periodic,
        knots: s.knots.clone(),
        control_points: s.control_points.clone(),
        weights: s.weights.clone().unwrap_or_default(),
        fit_points: s.fit.as_ref().map_or_else(Vec::new, |f| f.points.clone()),
        start_tangent: s.fit.as_ref().and_then(|f| f.start_tangent),
        end_tangent: s.fit.as_ref().and_then(|f| f.end_tangent),
    })
}

fn polyline_loop(vertices: Vec<Point>, bulges: Vec<f64>) -> HatchLoop {
    HatchLoop {
        flags: HatchLoopFlags(HatchLoopFlags::EXTERNAL | HatchLoopFlags::POLYLINE),
        boundary: HatchBoundary::Polyline(HatchPolyline {
            vertices,
            bulges,
            closed: true,
        }),
        sources: Vec::new(),
    }
}

fn edges_loop(edges: Vec<HatchEdge>) -> HatchLoop {
    HatchLoop {
        flags: HatchLoopFlags(HatchLoopFlags::EXTERNAL),
        boundary: HatchBoundary::Edges(edges),
        sources: Vec::new(),
    }
}

/// The edges a polyline's segments are.
fn segment_edges(points: &[Point], bulges: &[f64], closed: bool) -> Vec<HatchEdge> {
    let n = points.len();
    (0..bulge::segment_count(n, closed))
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            match bulge::arc_of(a, b, bulge::at(bulges, i)) {
                None => HatchEdge::Line { start: a, end: b },
                Some(s) if s.forward => HatchEdge::CircularArc {
                    center: s.center,
                    radius: s.radius,
                    start_angle: s.start,
                    end_angle: s.end,
                    counter_clockwise: true,
                },
                Some(s) => HatchEdge::CircularArc {
                    center: s.center,
                    radius: s.radius,
                    start_angle: s.end,
                    end_angle: s.start,
                    counter_clockwise: false,
                },
            }
        })
        .collect()
}

impl HatchEdge {
    /// The same edge run the other way.
    pub fn reversed(&self) -> HatchEdge {
        match self {
            HatchEdge::Line { start, end } => HatchEdge::Line {
                start: *end,
                end: *start,
            },
            HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                counter_clockwise,
            } => HatchEdge::CircularArc {
                center: *center,
                radius: *radius,
                start_angle: *end_angle,
                end_angle: *start_angle,
                counter_clockwise: !counter_clockwise,
            },
            HatchEdge::EllipticArc {
                center,
                major_axis,
                minor_major_ratio,
                start_param,
                end_param,
                counter_clockwise,
            } => HatchEdge::EllipticArc {
                center: *center,
                major_axis: *major_axis,
                minor_major_ratio: *minor_major_ratio,
                start_param: *end_param,
                end_param: *start_param,
                counter_clockwise: !counter_clockwise,
            },
            HatchEdge::Spline(s) => {
                let span = s.knots.first().copied().unwrap_or(0.0)
                    + s.knots.last().copied().unwrap_or(0.0);
                let negate = |t: Point| Point::new(-t.x, -t.y);
                HatchEdge::Spline(HatchSpline {
                    degree: s.degree,
                    periodic: s.periodic,
                    knots: s.knots.iter().rev().map(|k| span - k).collect(),
                    control_points: s.control_points.iter().rev().copied().collect(),
                    weights: s.weights.iter().rev().copied().collect(),
                    fit_points: s.fit_points.iter().rev().copied().collect(),
                    start_tangent: s.end_tangent.map(negate),
                    end_tangent: s.start_tangent.map(negate),
                })
            }
        }
    }
}

/// A loop and the indices of the objects it was made from.
pub type Sourced = (HatchLoop, Vec<usize>);

/// A closed chain of edges and the indices of the objects it joins.
type Chain = (Vec<HatchEdge>, Vec<usize>);

/// Joins `pieces` (edges, each with the index of its object) end to end
/// into closed chains.
fn chain(pieces: Vec<(HatchEdge, usize)>, tolerance: f64) -> Result<Vec<Chain>, BuildError> {
    let near = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y) <= tolerance;
    let mut pool: Vec<Option<(HatchEdge, usize)>> = pieces.into_iter().map(Some).collect();
    let mut loops = Vec::new();
    for first in 0..pool.len() {
        let Some((head, from)) = pool[first].take() else {
            continue;
        };
        let (Some(origin), Some(mut end)) = (head.start_point(), head.end_point()) else {
            return Err(BuildError::Open(1));
        };
        let mut run = vec![head];
        let mut used = vec![from];
        while !near(end, origin) {
            let next = pool.iter().position(|e| {
                e.as_ref().is_some_and(|(e, _)| {
                    e.start_point().is_some_and(|s| near(s, end))
                        || e.end_point().is_some_and(|s| near(s, end))
                })
            });
            let Some(i) = next else {
                return Err(BuildError::Open(run.len()));
            };
            let Some((edge, object)) = pool[i].take() else {
                return Err(BuildError::Open(run.len()));
            };
            let edge = if edge.start_point().is_some_and(|s| near(s, end)) {
                edge
            } else {
                edge.reversed()
            };
            let Some(e) = edge.end_point() else {
                return Err(BuildError::Open(run.len()));
            };
            end = e;
            run.push(edge);
            if !used.contains(&object) {
                used.push(object);
            }
        }
        loops.push((run, used));
    }
    Ok(loops)
}

/// Boundary loops from `entities`, joining pieces that meet within
/// `tolerance`. Kinds that cannot bound a hatch (text, dimensions, blocks,
/// other hatches, open lines of no length) are passed over; the second value
/// is how many.
pub fn loops_from_entities(
    entities: &[Entity],
    tolerance: f64,
) -> Result<(Vec<HatchLoop>, usize), BuildError> {
    let (loops, ignored) = loops_and_sources(entities, tolerance)?;
    Ok((loops.into_iter().map(|(l, _)| l).collect(), ignored))
}

/// Like [`loops_from_entities`], with each loop's objects as indices into
/// `entities`.
pub fn loops_and_sources(
    entities: &[Entity],
    tolerance: f64,
) -> Result<(Vec<Sourced>, usize), BuildError> {
    let mut loops: Vec<Sourced> = Vec::new();
    let mut pieces: Vec<(HatchEdge, usize)> = Vec::new();
    let mut ignored = 0;
    for (at, entity) in entities.iter().enumerate() {
        match entity {
            Entity::Circle { center, radius } => loops.push((
                edges_loop(vec![HatchEdge::CircularArc {
                    center: *center,
                    radius: *radius,
                    start_angle: 0.0,
                    end_angle: TAU,
                    counter_clockwise: true,
                }]),
                vec![at],
            )),
            Entity::Rectangle { min, max } => loops.push((
                polyline_loop(
                    vec![
                        *min,
                        Point::new(max.x, min.y),
                        *max,
                        Point::new(min.x, max.y),
                    ],
                    Vec::new(),
                ),
                vec![at],
            )),
            Entity::Polyline {
                points,
                closed: true,
                bulges,
                ..
            } if points.len() >= 3 => {
                loops.push((polyline_loop(points.clone(), bulges.clone()), vec![at]));
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } if points.len() >= 2 => pieces.extend(
                segment_edges(points, bulges, *closed)
                    .into_iter()
                    .map(|e| (e, at)),
            ),
            Entity::Line { start, end } => pieces.push((
                HatchEdge::Line {
                    start: *start,
                    end: *end,
                },
                at,
            )),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => pieces.push((
                HatchEdge::CircularArc {
                    center: *center,
                    radius: *radius,
                    start_angle: *start,
                    end_angle: *end,
                    counter_clockwise: true,
                },
                at,
            )),
            Entity::Ellipse(e) => {
                let edge = HatchEdge::EllipticArc {
                    center: e.center,
                    major_axis: e.major_axis,
                    minor_major_ratio: e.radius_ratio,
                    start_param: e.start,
                    end_param: e.end,
                    counter_clockwise: true,
                };
                if e.is_full() {
                    loops.push((edges_loop(vec![edge]), vec![at]));
                } else {
                    pieces.push((edge, at));
                }
            }
            Entity::Spline(s) => {
                let edge = edge_of_spline(s);
                if s.closed {
                    loops.push((edges_loop(vec![edge]), vec![at]));
                } else {
                    pieces.push((edge, at));
                }
            }
            _ => ignored += 1,
        }
    }
    loops.extend(
        chain(pieces, tolerance)?
            .into_iter()
            .map(|(edges, used)| (edges_loop(edges), used)),
    );
    if loops.is_empty() {
        return Err(BuildError::Nothing);
    }
    if loops.len() > super::validate::MAX_LOOPS {
        return Err(BuildError::TooMany);
    }
    Ok((loops, ignored))
}

impl HatchLoop {
    /// This loop remembering the objects it was made from.
    pub fn with_sources(mut self, sources: Vec<BoundarySource>) -> Self {
        self.sources = sources;
        self
    }
}
