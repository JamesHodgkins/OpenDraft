//! Moving, turning, mirroring and scaling a hatch. A map that keeps shapes
//! (a similarity) takes every kind of edge and every pattern line to the same
//! kind, exactly. Uneven scaling would turn circles into ellipses and is
//! refused. Sources and the associative flag are kept: whether the hatch still
//! follows them is for `follow` to decide once the whole edit is known.

use super::{
    HatchBoundary, HatchEdge, HatchEntity, HatchError, HatchFill, HatchLoop, HatchPolyline,
    HatchSpline, PatternLine,
};
use crate::{Affine, Point};

/// The linear part of a map applied to a vector.
fn vector(map: &Affine, v: Point) -> Point {
    Point::new(map.xx * v.x + map.xy * v.y, map.yx * v.x + map.yy * v.y)
}

/// The direction (radians) an angle turns into.
fn direction(map: &Affine, angle: f64) -> f64 {
    let v = vector(map, Point::new(angle.cos(), angle.sin()));
    v.y.atan2(v.x)
}

struct Map {
    map: Affine,
    scale: f64,
    mirrored: bool,
}

impl Map {
    fn point(&self, p: Point) -> Point {
        self.map.apply(p)
    }

    fn edge(&self, edge: &HatchEdge) -> HatchEdge {
        match edge {
            HatchEdge::Line { start, end } => HatchEdge::Line {
                start: self.point(*start),
                end: self.point(*end),
            },
            HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                counter_clockwise,
            } => HatchEdge::CircularArc {
                center: self.point(*center),
                radius: radius * self.scale,
                start_angle: direction(&self.map, *start_angle),
                end_angle: direction(&self.map, *end_angle),
                counter_clockwise: *counter_clockwise != self.mirrored,
            },
            HatchEdge::EllipticArc {
                center,
                major_axis,
                minor_major_ratio,
                start_param,
                end_param,
                counter_clockwise,
            } => {
                // A reflection runs the ellipse's parameter the other way.
                let (start_param, end_param) = if self.mirrored {
                    (-start_param, -end_param)
                } else {
                    (*start_param, *end_param)
                };
                HatchEdge::EllipticArc {
                    center: self.point(*center),
                    major_axis: vector(&self.map, *major_axis),
                    minor_major_ratio: *minor_major_ratio,
                    start_param,
                    end_param,
                    counter_clockwise: *counter_clockwise != self.mirrored,
                }
            }
            HatchEdge::Spline(s) => HatchEdge::Spline(HatchSpline {
                degree: s.degree,
                periodic: s.periodic,
                knots: s.knots.clone(),
                control_points: s.control_points.iter().map(|p| self.point(*p)).collect(),
                weights: s.weights.clone(),
                fit_points: s.fit_points.iter().map(|p| self.point(*p)).collect(),
                start_tangent: s.start_tangent.map(|t| vector(&self.map, t)),
                end_tangent: s.end_tangent.map(|t| vector(&self.map, t)),
            }),
        }
    }

    fn hatch_loop(&self, l: &HatchLoop) -> HatchLoop {
        let boundary = match &l.boundary {
            HatchBoundary::Polyline(p) => HatchBoundary::Polyline(HatchPolyline {
                vertices: p.vertices.iter().map(|v| self.point(*v)).collect(),
                // A reflection turns every arc the other way round.
                bulges: if self.mirrored {
                    p.bulges.iter().map(|b| -b).collect()
                } else {
                    p.bulges.clone()
                },
                closed: p.closed,
            }),
            HatchBoundary::Edges(edges) => {
                HatchBoundary::Edges(edges.iter().map(|e| self.edge(e)).collect())
            }
        };
        HatchLoop {
            flags: l.flags,
            boundary,
            sources: l.sources.clone(),
        }
    }

    fn line(&self, l: &PatternLine) -> PatternLine {
        let offset = vector(&self.map, Point::new(l.offset[0], l.offset[1]));
        PatternLine {
            angle: direction(&self.map, l.angle),
            base: self.point(l.base),
            offset: [offset.x, offset.y],
            dashes: l.dashes.iter().map(|d| d * self.scale).collect(),
        }
    }
}

impl HatchEntity {
    /// The hatch under `map`, which must keep shapes.
    pub fn transformed(&self, map: &Affine) -> Result<HatchEntity, HatchError> {
        let similarity = map
            .similarity()
            .ok_or(HatchError::Unsupported("be scaled unevenly"))?;
        let m = Map {
            map: *map,
            scale: similarity.scale,
            mirrored: similarity.mirrored,
        };
        let fill = match &self.fill {
            HatchFill::Pattern(p) => HatchFill::Pattern(super::HatchPattern {
                angle: direction(map, p.angle),
                scale_or_spacing: p.scale_or_spacing * m.scale,
                origin: m.point(p.origin),
                lines: p.lines.iter().map(|l| m.line(l)).collect(),
                ..p.clone()
            }),
            HatchFill::Solid => HatchFill::Solid,
            HatchFill::Gradient(g) => HatchFill::Gradient(super::HatchGradient {
                angle: direction(map, g.angle),
                ..g.clone()
            }),
        };
        Ok(HatchEntity {
            elevation: self.elevation,
            loops: self.loops.iter().map(|l| m.hatch_loop(l)).collect(),
            style: self.style,
            fill,
            associative: self.associative,
            seed_points: self.seed_points.iter().map(|p| m.point(*p)).collect(),
            source_pixel_size: self.source_pixel_size.map(|s| s * m.scale),
        })
    }

    fn mapped(&self, map: Affine) -> HatchEntity {
        // The maps below are all similarities, which cannot fail.
        self.transformed(&map).unwrap_or_else(|_| self.clone())
    }

    pub fn translated(&self, dx: f64, dy: f64) -> HatchEntity {
        self.mapped(Affine::translation(dx, dy))
    }

    pub fn rotated(&self, center: Point, angle: f64) -> HatchEntity {
        let map = Affine::translation(-center.x, -center.y)
            .then(&Affine::rotation(angle))
            .then(&Affine::translation(center.x, center.y));
        self.mapped(map)
    }

    pub fn mirrored(&self, a: Point, b: Point) -> HatchEntity {
        self.mapped(Affine::reflection(a, b))
    }

    /// Enlarged by a positive `factor` about `base`.
    pub fn scaled(&self, base: Point, factor: f64) -> HatchEntity {
        let map = Affine::translation(-base.x, -base.y)
            .then(&Affine::scaling(factor, factor))
            .then(&Affine::translation(base.x, base.y));
        self.mapped(map)
    }
}
