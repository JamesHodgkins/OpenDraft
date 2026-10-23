//! The native format's description of one entity, kept apart from the rest
//! of the file so both stay small.

use od_core::{Ellipse, Entity, Point};
use serde::{Deserialize, Serialize};

use super::block::{insert_entity, Blocks};
use super::dimension::DimensionDto;
use super::hatch::HatchDto;
use super::leader::LeaderDto;
use super::mleader::MLeaderDto;
use super::spline::SplineDto;
use super::text::MTextDto;
use crate::FileError;

pub(super) type PointDto = [f64; 2];

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum EntityDto {
    Point {
        at: PointDto,
    },
    Line {
        start: PointDto,
        end: PointDto,
    },
    Circle {
        center: PointDto,
        radius: f64,
    },
    Arc {
        center: PointDto,
        radius: f64,
        start: f64,
        end: f64,
    },
    /// Every field is required, even for a full ellipse, so a damaged arc
    /// can never read as a full ellipse.
    Ellipse {
        center: PointDto,
        major_axis: PointDto,
        radius_ratio: f64,
        start: f64,
        end: f64,
    },
    /// The control frame exactly, with any fit data.
    Spline(SplineDto),
    Rectangle {
        min: PointDto,
        max: PointDto,
    },
    Polyline {
        points: Vec<PointDto>,
        #[serde(default)]
        closed: bool,
        /// One bulge per point (see `Entity::Polyline`), left out when every
        /// segment is straight.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        bulges: Vec<f64>,
        /// One `[start, end]` width per point (see `Entity::Polyline`), left
        /// out when every segment is a hairline.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        widths: Vec<[f64; 2]>,
    },
    Dimension(DimensionDto),
    /// Boundary loops, island style and fill; the strokes are not stored.
    Hatch(HatchDto),
    #[serde(rename = "mtext")]
    MText(MTextDto),
    #[serde(rename = "mleader")]
    MLeader(MLeaderDto),
    Leader(LeaderDto),
    /// A block reference; `block` is the id of a definition in the
    /// drawing's `blocks` list.
    Insert {
        block: u64,
        insertion: PointDto,
        #[serde(default = "unit_scale")]
        scale: [f64; 2],
        #[serde(default)]
        rotation: f64,
    },
}

fn unit_scale() -> [f64; 2] {
    [1.0, 1.0]
}

pub(super) fn invalid<T>(why: impl Into<String>) -> Result<T, FileError> {
    Err(FileError::Invalid(why.into()))
}

pub(super) fn point(p: Point) -> PointDto {
    [p.x, p.y]
}

pub(super) fn from_point(p: PointDto) -> Result<Point, FileError> {
    if p[0].is_finite() && p[1].is_finite() {
        Ok(Point::new(p[0], p[1]))
    } else {
        invalid("a coordinate is not a finite number")
    }
}

impl EntityDto {
    pub(super) fn from_entity(entity: &Entity) -> Result<Self, FileError> {
        let dto = match entity {
            Entity::Point { at } => EntityDto::Point { at: point(*at) },
            Entity::Line { start, end } => EntityDto::Line {
                start: point(*start),
                end: point(*end),
            },
            Entity::Circle { center, radius } => EntityDto::Circle {
                center: point(*center),
                radius: *radius,
            },
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => EntityDto::Arc {
                center: point(*center),
                radius: *radius,
                start: *start,
                end: *end,
            },
            Entity::Ellipse(e) => {
                if let Err(why) = e.validate() {
                    return invalid(format!("an ellipse is not valid: {why}"));
                }
                EntityDto::Ellipse {
                    center: point(e.center),
                    major_axis: point(e.major_axis),
                    radius_ratio: e.radius_ratio,
                    start: e.start,
                    end: e.end,
                }
            }
            Entity::Spline(s) => EntityDto::Spline(SplineDto::from_spline(s)?),
            Entity::Rectangle { min, max } => EntityDto::Rectangle {
                min: point(*min),
                max: point(*max),
            },
            Entity::Dimension(d) => EntityDto::Dimension(DimensionDto::from_dimension(d)),
            Entity::MText(m) => EntityDto::MText(MTextDto::from_mtext(m)),
            Entity::MLeader(m) => EntityDto::MLeader(MLeaderDto::from_mleader(m)?),
            Entity::Leader(l) => EntityDto::Leader(LeaderDto::from_leader(l)?),
            Entity::Hatch(h) => EntityDto::Hatch(HatchDto::from_hatch(h)?),
            Entity::Insert(i) => {
                if !i.is_valid() {
                    return invalid("a block insert has a zero or non-finite scale");
                }
                // Its block is written in the block table; nothing more to check.
                return Ok(EntityDto::Insert {
                    block: i.block.id.0,
                    insertion: point(i.insertion),
                    scale: [i.scale_x, i.scale_y],
                    rotation: i.rotation,
                });
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => EntityDto::Polyline {
                points: points.iter().map(|p| point(*p)).collect(),
                closed: *closed,
                bulges: bulges.clone(),
                widths: widths.iter().map(|&(a, b)| [a, b]).collect(),
            },
        };
        // Refuse to write what could not be read back.
        dto.to_entity(&Blocks::new())?;
        Ok(dto)
    }

    /// The entity, with inserts resolved in `blocks` (definitions already
    /// read).
    pub(super) fn to_entity(&self, blocks: &Blocks) -> Result<Entity, FileError> {
        Ok(match self {
            EntityDto::Insert {
                block,
                insertion,
                scale,
                rotation,
            } => insert_entity(blocks, *block, *insertion, *scale, *rotation)?,
            EntityDto::Point { at } => Entity::Point {
                at: from_point(*at)?,
            },
            EntityDto::Line { start, end } => Entity::Line {
                start: from_point(*start)?,
                end: from_point(*end)?,
            },
            EntityDto::Circle { center, radius } => {
                if !(radius.is_finite() && *radius > 0.0) {
                    return invalid("a circle has no positive radius");
                }
                Entity::Circle {
                    center: from_point(*center)?,
                    radius: *radius,
                }
            }
            EntityDto::Arc {
                center,
                radius,
                start,
                end,
            } => {
                if !(radius.is_finite() && *radius > 0.0) {
                    return invalid("an arc has no positive radius");
                }
                if !(start.is_finite() && end.is_finite()) {
                    return invalid("an arc angle is not a finite number");
                }
                Entity::Arc {
                    center: from_point(*center)?,
                    radius: *radius,
                    start: *start,
                    end: *end,
                }
            }
            EntityDto::Ellipse {
                center,
                major_axis,
                radius_ratio,
                start,
                end,
            } => Entity::Ellipse(
                Ellipse::new(
                    from_point(*center)?,
                    from_point(*major_axis)?,
                    *radius_ratio,
                    *start,
                    *end,
                )
                .or_else(|why| invalid(format!("an ellipse is not valid: {why}")))?,
            ),
            EntityDto::Spline(s) => Entity::Spline(Box::new(s.to_spline()?)),
            EntityDto::Dimension(d) => Entity::Dimension(Box::new(d.to_dimension()?)),
            EntityDto::MText(m) => Entity::MText(Box::new(m.to_mtext()?)),
            EntityDto::MLeader(m) => Entity::MLeader(Box::new(m.to_mleader()?)),
            EntityDto::Leader(l) => Entity::Leader(Box::new(l.to_leader()?)),
            EntityDto::Hatch(h) => Entity::Hatch(Box::new(h.to_hatch()?)),
            EntityDto::Rectangle { min, max } => Entity::Rectangle {
                min: from_point(*min)?,
                max: from_point(*max)?,
            },
            EntityDto::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => {
                if points.len() < 2 {
                    return invalid("a polyline has fewer than two points");
                }
                if !bulges.is_empty() && bulges.len() != points.len() {
                    return invalid("a polyline needs one bulge for each point, or none");
                }
                if bulges.iter().any(|b| !b.is_finite()) {
                    return invalid("a polyline bulge is not a finite number");
                }
                if !widths.is_empty() && widths.len() != points.len() {
                    return invalid("a polyline needs one width pair for each point, or none");
                }
                if widths
                    .iter()
                    .any(|w| !w.iter().all(|x| x.is_finite() && *x >= 0.0))
                {
                    return invalid("a polyline width is negative or not a finite number");
                }
                Entity::Polyline {
                    points: points
                        .iter()
                        .map(|p| from_point(*p))
                        .collect::<Result<_, _>>()?,
                    closed: *closed,
                    bulges: bulges.clone(),
                    widths: widths.iter().map(|w| (w[0], w[1])).collect(),
                }
            }
        })
    }
}
