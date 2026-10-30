//! Exact geometry answers from core, so a model does not have to redo CAD
//! arithmetic from sampled points: `geometry.measure` and
//! `geometry.intersections`.

use std::f64::consts::PI;

use od_core::{bulge, Entity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{BoundsDto, PointDto};
use crate::WireId;

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeasureArgs {
    pub document_id: String,
    /// At most 1000.
    pub entity_ids: Vec<WireId>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntersectArgs {
    pub document_id: String,
    /// Two to 200 entities; every pair whose bounds overlap is tested.
    pub entity_ids: Vec<WireId>,
}

/// How a number was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exactness {
    /// A closed formula.
    Exact,
    /// Integrated or flattened numerically, to about 12 significant figures.
    Numeric,
    /// Only the bounds are defined for this kind.
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub entity_id: WireId,
    pub kind: String,
    pub bounds: BoundsDto,
    /// Length of a curve, or the perimeter of a closed shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// Area enclosed by a closed shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<PointDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// How far an arc turns, in degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_degrees: Option<f64>,
    /// Of an open curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<PointDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<PointDto>,
    pub exactness: Exactness,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeasureResult {
    pub revision: WireId,
    /// One slot per requested id; `null` for an id that does not exist.
    pub measurements: Vec<Option<Measurement>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntersectionPoint {
    pub entity_a: WireId,
    pub entity_b: WireId,
    pub point: PointDto,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntersectResult {
    pub revision: WireId,
    /// Pairs whose bounds overlap and so were tested.
    pub pairs_tested: usize,
    pub points: Vec<IntersectionPoint>,
    /// Entities with no outline the crossing code knows (splines, text,
    /// blocks, dimensions, hatches); they were skipped, not found clear.
    pub unsupported: Vec<WireId>,
    /// Coincident overlapping segments report no points.
    pub notes: Vec<String>,
}

fn pt(p: od_core::Point) -> PointDto {
    p.into()
}

fn base(id: WireId, entity: &Entity) -> Measurement {
    Measurement {
        entity_id: id,
        kind: entity.kind().wire_name().to_owned(),
        bounds: entity.bounds().into(),
        length: None,
        area: None,
        center: None,
        radius: None,
        sweep_degrees: None,
        start: None,
        end: None,
        exactness: Exactness::Exact,
        notes: Vec::new(),
    }
}

/// What can be said exactly about one entity.
pub fn measure(id: WireId, entity: &Entity) -> Measurement {
    let mut m = base(id, entity);
    match entity {
        Entity::Point { at } => m.start = Some(pt(*at)),
        Entity::Xline { origin, .. } => {
            m.start = Some(pt(*origin));
            m.notes.push("A construction line has no ends.".to_owned());
        }
        Entity::Ray { origin, .. } => {
            m.start = Some(pt(*origin));
            m.notes.push("A ray has no far end.".to_owned());
        }
        Entity::Line { start, end } => {
            m.length = entity.curve_length();
            m.start = Some(pt(*start));
            m.end = Some(pt(*end));
        }
        Entity::Circle { center, radius } => {
            let r = radius.abs();
            m.length = Some(2.0 * PI * r);
            m.area = Some(PI * r * r);
            m.center = Some(pt(*center));
            m.radius = Some(r);
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            m.length = entity.curve_length();
            m.center = Some(pt(*center));
            m.radius = Some(radius.abs());
            m.sweep_degrees = Some(od_core::arc::sweep(*start, *end).to_degrees());
            m.start = Some(pt(od_core::arc::point_at(*center, radius.abs(), *start)));
            m.end = Some(pt(od_core::arc::point_at(*center, radius.abs(), *end)));
        }
        Entity::Rectangle { min, max } => {
            let (w, h) = (max.x - min.x, max.y - min.y);
            m.length = Some(2.0 * (w + h));
            m.area = Some(w * h);
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => {
            m.length = Some(bulge::path_length(points, bulges, *closed));
            if *closed {
                m.area = Some(bulge::path_area(points, bulges));
                m.notes.push(
                    "For a self-crossing outline the area is the algebraic sum of its loops."
                        .to_owned(),
                );
            } else if let (Some(first), Some(last)) = (points.first(), points.last()) {
                m.start = Some(pt(*first));
                m.end = Some(pt(*last));
            }
        }
        Entity::Ellipse(e) => {
            m.exactness = Exactness::Numeric;
            m.length = Some(e.length());
            m.center = Some(pt(e.center));
            if e.is_full() {
                m.area = Some(e.area());
            } else {
                m.start = Some(pt(e.start_point()));
                m.end = Some(pt(e.end_point()));
            }
        }
        Entity::Spline(s) => {
            m.exactness = Exactness::Numeric;
            match s.length() {
                Ok(length) => m.length = Some(length),
                Err(_) => m.notes.push("The length could not be computed.".to_owned()),
            }
            if !s.closed {
                m.start = Some(pt(s.start_point()));
                m.end = Some(pt(s.end_point()));
            }
        }
        Entity::MText(_)
        | Entity::Insert(_)
        | Entity::Dimension(_)
        | Entity::MLeader(_)
        | Entity::Leader(_)
        | Entity::Hatch(_) => {
            m.exactness = Exactness::Unsupported;
            m.notes
                .push("Only the bounds are defined for this kind.".to_owned());
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use od_core::Point;

    use super::*;

    fn of(entity: Entity) -> Measurement {
        measure(WireId(1), &entity)
    }

    fn near(a: Option<f64>, b: f64) {
        let a = a.expect("a value");
        assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
    }

    #[test]
    fn simple_shapes_have_closed_form_answers() {
        near(
            of(Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(3.0, 4.0),
            })
            .length,
            5.0,
        );
        let circle = of(Entity::Circle {
            center: Point::new(1.0, 1.0),
            radius: 2.0,
        });
        near(circle.length, 4.0 * PI);
        near(circle.area, 4.0 * PI);
        near(circle.radius, 2.0);
        let rect = of(Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(4.0, 3.0),
        });
        near(rect.length, 14.0);
        near(rect.area, 12.0);
        let quarter = of(Entity::Arc {
            center: Point::new(0.0, 0.0),
            radius: 2.0,
            start: 0.0,
            end: PI / 2.0,
        });
        near(quarter.length, PI);
        near(quarter.sweep_degrees, 90.0);
        let end = quarter.end.unwrap();
        assert!((end.x).abs() < 1e-12 && (end.y - 2.0).abs() < 1e-12);
    }

    #[test]
    fn polylines_count_arcs_and_only_closed_ones_have_an_area() {
        let square = |closed| Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(2.0, 2.0),
                Point::new(0.0, 2.0),
            ],
            closed,
            bulges: vec![],
            widths: vec![],
        };
        let closed = of(square(true));
        near(closed.length, 8.0);
        near(closed.area, 4.0);
        let open = of(square(false));
        near(open.length, 6.0);
        assert!(open.area.is_none() && open.start.is_some() && open.end.is_some());
        // A bulge of 1 is a half circle: with the chord back, its area is half a circle's.
        let half = of(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
            closed: true,
            bulges: vec![1.0, 0.0],
            widths: vec![],
        });
        near(half.area, PI / 2.0);
    }

    #[test]
    fn ellipses_are_numeric_and_say_so() {
        let e = od_core::Ellipse::new(
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            0.5,
            0.0,
            std::f64::consts::TAU,
        )
        .unwrap();
        let m = of(Entity::Ellipse(e));
        assert_eq!(m.exactness, Exactness::Numeric);
        near(m.area, PI * 2.0 * 1.0);
        // Ramanujan's approximation for a = 2, b = 1.
        let ramanujan = PI * (9.0 - 35.0_f64.sqrt());
        assert!((m.length.unwrap() - ramanujan).abs() < 1e-3);
    }

    #[test]
    fn kinds_without_a_length_say_only_their_bounds_are_known() {
        let text = od_core::MText::new(Point::new(0.0, 0.0), 2.5, "Hi", od_core::TextStyleId(0));
        let m = of(Entity::MText(Box::new(text)));
        assert_eq!(m.exactness, Exactness::Unsupported);
        assert!(m.length.is_none() && m.area.is_none() && !m.notes.is_empty());
    }
}
