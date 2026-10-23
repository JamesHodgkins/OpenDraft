//! The native format's description of a hatch: its boundary loops, island
//! style and fill definition. Nothing derived (strokes, areas) is stored.

use od_core::hatch::{
    BoundarySource, HatchBoundary, HatchEdge, HatchEntity, HatchLoop, HatchLoopFlags,
    HatchPolyline, HatchSpline, HatchStyle,
};
use od_core::EntityId;
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use super::hatch_fill::FillDto;
use crate::FileError;

#[derive(Serialize, Deserialize)]
pub(super) struct HatchDto {
    #[serde(default, skip_serializing_if = "is_zero")]
    elevation: f64,
    loops: Vec<LoopDto>,
    /// `normal`, `outer` or `ignore`.
    style: String,
    fill: FillDto,
    #[serde(default)]
    associative: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    seeds: Vec<PointDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pixel_size: Option<f64>,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Serialize, Deserialize)]
struct LoopDto {
    flags: u32,
    boundary: BoundaryDto,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sources: Vec<SourceDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SourceDto {
    Entity { id: u64 },
    External { scheme: String, value: String },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BoundaryDto {
    Polyline {
        vertices: Vec<PointDto>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        bulges: Vec<f64>,
        closed: bool,
    },
    Edges {
        edges: Vec<EdgeDto>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EdgeDto {
    Line {
        start: PointDto,
        end: PointDto,
    },
    Arc {
        center: PointDto,
        radius: f64,
        start: f64,
        end: f64,
        ccw: bool,
    },
    Ellipse {
        center: PointDto,
        major_axis: PointDto,
        ratio: f64,
        start: f64,
        end: f64,
        ccw: bool,
    },
    Spline {
        degree: u8,
        #[serde(default)]
        periodic: bool,
        knots: Vec<f64>,
        control_points: Vec<PointDto>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        weights: Vec<f64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fit_points: Vec<PointDto>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start_tangent: Option<PointDto>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_tangent: Option<PointDto>,
    },
}

fn style_name(s: HatchStyle) -> &'static str {
    match s {
        HatchStyle::Normal => "normal",
        HatchStyle::Outer => "outer",
        HatchStyle::Ignore => "ignore",
    }
}

fn points(dtos: &[PointDto]) -> Result<Vec<od_core::Point>, FileError> {
    dtos.iter().map(|p| from_point(*p)).collect()
}

impl HatchDto {
    pub(super) fn from_hatch(h: &HatchEntity) -> Result<Self, FileError> {
        h.validate()
            .or_else(|why| invalid(format!("a hatch is not valid: {why}")))?;
        Ok(Self {
            elevation: h.elevation,
            loops: h.loops.iter().map(LoopDto::from_loop).collect(),
            style: style_name(h.style).to_owned(),
            fill: FillDto::from_fill(&h.fill),
            associative: h.associative,
            seeds: h.seed_points.iter().map(|p| point(*p)).collect(),
            pixel_size: h.source_pixel_size,
        })
    }

    pub(super) fn to_hatch(&self) -> Result<HatchEntity, FileError> {
        let style = match self.style.as_str() {
            "normal" => HatchStyle::Normal,
            "outer" => HatchStyle::Outer,
            "ignore" => HatchStyle::Ignore,
            other => return invalid(format!("a hatch has unknown island style \"{other}\"")),
        };
        let hatch = HatchEntity {
            elevation: self.elevation,
            loops: self
                .loops
                .iter()
                .map(LoopDto::to_loop)
                .collect::<Result<_, _>>()?,
            style,
            fill: self.fill.to_fill()?,
            associative: self.associative,
            seed_points: points(&self.seeds)?,
            source_pixel_size: self.pixel_size,
        };
        hatch
            .validate()
            .or_else(|why| invalid(format!("a hatch is not valid: {why}")))?;
        Ok(hatch)
    }
}

impl LoopDto {
    fn from_loop(l: &HatchLoop) -> Self {
        let boundary = match &l.boundary {
            HatchBoundary::Polyline(p) => BoundaryDto::Polyline {
                vertices: p.vertices.iter().map(|v| point(*v)).collect(),
                bulges: p.bulges.clone(),
                closed: p.closed,
            },
            HatchBoundary::Edges(edges) => BoundaryDto::Edges {
                edges: edges.iter().map(EdgeDto::from_edge).collect(),
            },
        };
        Self {
            flags: l.flags.0,
            boundary,
            sources: l
                .sources
                .iter()
                .map(|s| match s {
                    BoundarySource::Entity(id) => SourceDto::Entity { id: id.0 },
                    BoundarySource::External { scheme, value } => SourceDto::External {
                        scheme: scheme.clone(),
                        value: value.clone(),
                    },
                })
                .collect(),
        }
    }

    fn to_loop(&self) -> Result<HatchLoop, FileError> {
        let boundary = match &self.boundary {
            BoundaryDto::Polyline {
                vertices,
                bulges,
                closed,
            } => HatchBoundary::Polyline(HatchPolyline {
                vertices: points(vertices)?,
                bulges: bulges.clone(),
                closed: *closed,
            }),
            BoundaryDto::Edges { edges } => HatchBoundary::Edges(
                edges
                    .iter()
                    .map(EdgeDto::to_edge)
                    .collect::<Result<_, _>>()?,
            ),
        };
        Ok(HatchLoop {
            flags: HatchLoopFlags(self.flags),
            boundary,
            sources: self
                .sources
                .iter()
                .map(|s| match s {
                    SourceDto::Entity { id } => BoundarySource::Entity(EntityId(*id)),
                    SourceDto::External { scheme, value } => BoundarySource::External {
                        scheme: scheme.clone(),
                        value: value.clone(),
                    },
                })
                .collect(),
        })
    }
}

impl EdgeDto {
    fn from_edge(e: &HatchEdge) -> Self {
        match e {
            HatchEdge::Line { start, end } => EdgeDto::Line {
                start: point(*start),
                end: point(*end),
            },
            HatchEdge::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                counter_clockwise,
            } => EdgeDto::Arc {
                center: point(*center),
                radius: *radius,
                start: *start_angle,
                end: *end_angle,
                ccw: *counter_clockwise,
            },
            HatchEdge::EllipticArc {
                center,
                major_axis,
                minor_major_ratio,
                start_param,
                end_param,
                counter_clockwise,
            } => EdgeDto::Ellipse {
                center: point(*center),
                major_axis: point(*major_axis),
                ratio: *minor_major_ratio,
                start: *start_param,
                end: *end_param,
                ccw: *counter_clockwise,
            },
            HatchEdge::Spline(s) => EdgeDto::Spline {
                degree: s.degree,
                periodic: s.periodic,
                knots: s.knots.clone(),
                control_points: s.control_points.iter().map(|p| point(*p)).collect(),
                weights: s.weights.clone(),
                fit_points: s.fit_points.iter().map(|p| point(*p)).collect(),
                start_tangent: s.start_tangent.map(point),
                end_tangent: s.end_tangent.map(point),
            },
        }
    }

    fn to_edge(&self) -> Result<HatchEdge, FileError> {
        Ok(match self {
            EdgeDto::Line { start, end } => HatchEdge::Line {
                start: from_point(*start)?,
                end: from_point(*end)?,
            },
            EdgeDto::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => HatchEdge::CircularArc {
                center: from_point(*center)?,
                radius: *radius,
                start_angle: *start,
                end_angle: *end,
                counter_clockwise: *ccw,
            },
            EdgeDto::Ellipse {
                center,
                major_axis,
                ratio,
                start,
                end,
                ccw,
            } => HatchEdge::EllipticArc {
                center: from_point(*center)?,
                major_axis: from_point(*major_axis)?,
                minor_major_ratio: *ratio,
                start_param: *start,
                end_param: *end,
                counter_clockwise: *ccw,
            },
            EdgeDto::Spline {
                degree,
                periodic,
                knots,
                control_points,
                weights,
                fit_points,
                start_tangent,
                end_tangent,
            } => HatchEdge::Spline(HatchSpline {
                degree: *degree,
                periodic: *periodic,
                knots: knots.clone(),
                control_points: points(control_points)?,
                weights: weights.clone(),
                fit_points: points(fit_points)?,
                start_tangent: start_tangent.map(from_point).transpose()?,
                end_tangent: end_tangent.map(from_point).transpose()?,
            }),
        })
    }
}
