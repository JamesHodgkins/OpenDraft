//! The native format's description of one entity, kept apart from the rest
//! of the file so both stay small.

use od_core::{DimStyle, Ellipse, Entity, Point};
use serde::{Deserialize, Serialize};

use super::block::{insert_entity, Blocks};
use super::dimension::DimensionDto;
use super::dimension_style::StylePool;
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
    Xline {
        origin: PointDto,
        direction: PointDto,
    },
    Ray {
        origin: PointDto,
        direction: PointDto,
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

/// A direction as stored, made a unit vector; a line must point somewhere.
fn unit_direction(p: PointDto) -> Result<Point, FileError> {
    od_core::infinite::unit(from_point(p)?).map_or_else(|| invalid("a line has no direction"), Ok)
}

impl EntityDto {
    pub(super) fn from_entity(entity: &Entity, styles: &mut StylePool) -> Result<Self, FileError> {
        let dto = match entity {
            Entity::Point { at } => EntityDto::Point { at: point(*at) },
            Entity::Xline { origin, direction } => EntityDto::Xline {
                origin: point(*origin),
                direction: point(*direction),
            },
            Entity::Ray { origin, direction } => EntityDto::Ray {
                origin: point(*origin),
                direction: point(*direction),
            },
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
            Entity::Dimension(d) => EntityDto::Dimension(DimensionDto::from_dimension(d, styles)),
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
        dto.to_entity(&Blocks::new(), styles.styles())?;
        Ok(dto)
    }

    /// The entity, with inserts resolved in `blocks` (definitions already
    /// read).
    pub(super) fn to_entity(
        &self,
        blocks: &Blocks,
        dim_styles: &[DimStyle],
    ) -> Result<Entity, FileError> {
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
            EntityDto::Xline { origin, direction } => Entity::Xline {
                origin: from_point(*origin)?,
                direction: unit_direction(*direction)?,
            },
            EntityDto::Ray { origin, direction } => Entity::Ray {
                origin: from_point(*origin)?,
                direction: unit_direction(*direction)?,
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
            EntityDto::Dimension(d) => Entity::Dimension(Box::new(d.to_dimension(dim_styles)?)),
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

#[cfg(test)]
mod tests {
    use od_core::{Document, Entity, Point};

    use crate::native::tests::roundtrip;

    #[test]
    fn points_construction_lines_and_rays_survive_a_round_trip() {
        let mut document = Document::default();
        document.add(Entity::Point {
            at: Point::new(1.5, -2.0),
        });
        document.add(Entity::Xline {
            origin: Point::new(0.0, 1.0),
            direction: Point::new(0.6, 0.8),
        });
        document.add(Entity::Ray {
            origin: Point::new(3.0, 3.0),
            direction: Point::new(-1.0, 0.0),
        });
        let back = roundtrip(&document);
        assert_eq!(back.objects().len(), 3);
        for (a, b) in back.objects().iter().zip(document.objects()) {
            assert_eq!(a.entity, b.entity);
        }
    }

    #[test]
    fn a_direction_is_made_a_unit_and_a_zero_one_is_refused() {
        let line = |d: &str| format!(r#"{{"type": "ray", "origin": [0, 0], "direction": {d}}}"#);
        let ok: super::EntityDto = serde_json::from_str(&line("[3, 4]")).unwrap();
        let blocks = crate::native::block::Blocks::new();
        let Entity::Ray { direction, .. } = ok.to_entity(&blocks, &[]).unwrap() else {
            panic!("a ray");
        };
        assert!((direction.x - 0.6).abs() < 1e-12 && (direction.y - 0.8).abs() < 1e-12);
        let zero: super::EntityDto = serde_json::from_str(&line("[0, 0]")).unwrap();
        assert!(zero.to_entity(&blocks, &[]).is_err());
    }
}
