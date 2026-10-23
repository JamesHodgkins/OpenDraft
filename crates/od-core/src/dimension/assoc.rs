//! Associative dimensions: a dimension whose defining points follow the
//! objects they were taken from. Each [`Assoc`] ties one defining point
//! (by its place in [`Dimension::points`]) to a point of an object, and
//! [`follow`] says what a dimension becomes when objects change; the
//! command layer applies it in the same undo step as the edit.
//!
//! A dimension edited on its own (moved, or a defining point dragged away
//! from its object) lets go of the points it no longer shares, and one whose
//! object is erased or no longer has that point lets go too, keeping where
//! the point was. Nothing is stored in the drawing but these ties: the
//! dimension's own points are the authority and the ties only refresh them.

use super::sketch::dist;
use super::{DimKind, Dimension};
use crate::{arc, Document, Entity, EntityId, Point};

/// Which point of an object a dimension point is tied to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AssocPoint {
    /// The start of a line or arc, or a point object.
    Start,
    /// The end of a line or arc.
    End,
    /// The centre of a circle, arc or ellipse.
    Center,
    /// A vertex of a polyline (or a corner of a rectangle).
    Vertex(usize),
    /// The point of a circle or arc in the direction the dimension point
    /// already lies from its centre: a radius dimension's chord point.
    Circumference,
}

/// A defining point and the object point it follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Assoc {
    /// The place in [`Dimension::points`].
    pub point: usize,
    pub target: EntityId,
    pub at: AssocPoint,
}

const TOLERANCE: f64 = 1e-7;

fn near(a: Point, b: Point) -> bool {
    dist(a, b) <= TOLERANCE * (1.0 + a.x.abs().max(a.y.abs()))
}

/// The point of `entity` that `at` names; `current` is where the dimension
/// point lies now (it gives the direction of a circumference point).
pub fn point_of(entity: &Entity, at: AssocPoint, current: Point) -> Option<Point> {
    match (entity, at) {
        (Entity::Line { start, .. }, AssocPoint::Start) => Some(*start),
        (Entity::Line { end, .. }, AssocPoint::End) => Some(*end),
        (Entity::Point { at }, AssocPoint::Start) => Some(*at),
        (
            Entity::Arc {
                center,
                radius,
                start,
                ..
            },
            AssocPoint::Start,
        ) => Some(arc::point_at(*center, *radius, *start)),
        (
            Entity::Arc {
                center,
                radius,
                end,
                ..
            },
            AssocPoint::End,
        ) => Some(arc::point_at(*center, *radius, *end)),
        (Entity::Circle { center, .. } | Entity::Arc { center, .. }, AssocPoint::Center) => {
            Some(*center)
        }
        (Entity::Ellipse(e), AssocPoint::Center) => Some(e.center),
        (
            Entity::Circle { center, radius } | Entity::Arc { center, radius, .. },
            AssocPoint::Circumference,
        ) => {
            let (dx, dy) = (current.x - center.x, current.y - center.y);
            let length = dx.hypot(dy);
            let (ux, uy) = if length > 1e-12 {
                (dx / length, dy / length)
            } else {
                (1.0, 0.0)
            };
            Some(Point::new(center.x + ux * radius, center.y + uy * radius))
        }
        (Entity::Polyline { points, .. }, AssocPoint::Vertex(i)) => points.get(i).copied(),
        (Entity::Rectangle { min, max }, AssocPoint::Vertex(i)) => [
            Point::new(min.x, min.y),
            Point::new(max.x, min.y),
            Point::new(max.x, max.y),
            Point::new(min.x, max.y),
        ]
        .get(i)
        .copied(),
        _ => None,
    }
}

/// The points of an object a dimension point might have been taken from.
fn candidates(entity: &Entity) -> Vec<(AssocPoint, Point)> {
    let mut found = Vec::new();
    for at in [AssocPoint::Start, AssocPoint::End, AssocPoint::Center] {
        if let Some(p) = point_of(entity, at, Point::default()) {
            found.push((at, p));
        }
    }
    let count = match entity {
        Entity::Polyline { points, .. } => points.len(),
        Entity::Rectangle { .. } => 4,
        _ => 0,
    };
    for i in 0..count {
        if let Some(p) = point_of(entity, AssocPoint::Vertex(i), Point::default()) {
            found.push((AssocPoint::Vertex(i), p));
        }
    }
    found
}

/// What the defining points of `dimension` are tied to, found by their
/// coordinates: a point that sits exactly on an object's end, centre or
/// vertex (as a snapped pick does) follows it. Angular dimensions of two
/// lines are left alone: their vertex is worked out from both lines.
fn find(dimension: &Dimension, document: &Document) -> Vec<Assoc> {
    if matches!(dimension.kind, DimKind::Angular { lines: true, .. }) {
        return Vec::new();
    }
    let points = dimension.points();
    let radial = matches!(dimension.kind, DimKind::Radial { .. });
    let mut found = Vec::new();
    for (index, point) in points.iter().enumerate() {
        // A jog's points are free.
        if matches!(dimension.kind, DimKind::Radial { .. }) && index > 1 {
            continue;
        }
        let tie = |target: EntityId, at: AssocPoint| Assoc {
            point: index,
            target,
            at,
        };
        let skip = match &dimension.kind {
            // The dimension line's place and the text are not on anything.
            DimKind::Linear { .. } => index > 1,
            DimKind::ArcLength { .. } | DimKind::Angular { .. } => index == 3,
            DimKind::Ordinate { .. } => index == 1,
            DimKind::Radial { .. } => false,
        };
        if skip {
            continue;
        }
        for object in document.objects() {
            if matches!(object.entity, Entity::Dimension(_)) {
                continue;
            }
            if radial && index == 1 {
                if let Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } =
                    &object.entity
                {
                    let r = dist(*center, *point);
                    if (r - radius.abs()).abs() <= TOLERANCE * (1.0 + r)
                        && points.first().is_some_and(|c| near(*c, *center))
                    {
                        found.push(tie(object.id, AssocPoint::Circumference));
                        break;
                    }
                }
                continue;
            }
            if let Some((at, _)) = candidates(&object.entity)
                .into_iter()
                .find(|(_, p)| near(*p, *point))
            {
                found.push(tie(object.id, at));
                break;
            }
        }
    }
    found
}

impl Dimension {
    /// This dimension tied to the objects of `document` its points sit on.
    pub fn associated(&self, document: &Document) -> Dimension {
        Dimension {
            assoc: find(self, document),
            ..self.clone()
        }
    }

    /// Without any ties.
    pub fn detached(&self) -> Dimension {
        Dimension {
            assoc: Vec::new(),
            ..self.clone()
        }
    }
}

/// The dimension `d` as it should be after an edit, or `None` when it is as
/// it should be. `edited` is whether `d` itself was changed by the edit;
/// `touched` says whether an object was changed or erased by it; `lookup`
/// finds the objects now.
pub fn follow(
    d: &Dimension,
    lookup: &dyn Fn(EntityId) -> Option<Entity>,
    edited: bool,
    touched: &dyn Fn(EntityId) -> bool,
) -> Option<Dimension> {
    if d.assoc.is_empty() {
        return None;
    }
    let mut out = d.clone();
    let mut kept = Vec::new();
    let mut changed = false;
    for tie in &d.assoc {
        let Some(current) = out.points().get(tie.point).copied() else {
            changed = true;
            continue;
        };
        let want = lookup(tie.target).and_then(|e| point_of(&e, tie.at, current));
        match want {
            None => changed = true,
            Some(w) if near(current, w) => kept.push(*tie),
            Some(w) if touched(tie.target) || !edited => {
                // The object moved: the point follows. (A dimension that
                // was not edited and does not match is out of date.)
                if let Some(next) = out.with_grip_moved(tie.point, w) {
                    out = next;
                    changed = true;
                }
                kept.push(*tie);
            }
            // The dimension moved away on its own.
            Some(_) => changed = true,
        }
    }
    if kept.len() != d.assoc.len() {
        changed = true;
    }
    out.assoc = kept;
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DimStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn document_with(entities: &[Entity]) -> (Document, Vec<EntityId>) {
        let mut doc = Document::default();
        let ids = entities.iter().map(|e| doc.add(e.clone())).collect();
        (doc, ids)
    }

    fn aligned(a: Point, b: Point) -> Dimension {
        Dimension::aligned(a, b, p(a.x, a.y + 5.0), &DimStyle::default())
    }

    #[test]
    fn points_on_the_ends_of_objects_are_tied_to_them() {
        let (doc, ids) = document_with(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let d = aligned(p(0.0, 0.0), p(10.0, 0.0)).associated(&doc);
        assert_eq!(
            d.assoc,
            [
                Assoc {
                    point: 0,
                    target: ids[0],
                    at: AssocPoint::Start
                },
                Assoc {
                    point: 1,
                    target: ids[0],
                    at: AssocPoint::End
                },
            ]
        );
        let loose = aligned(p(0.0, 0.5), p(10.0, 0.0)).associated(&doc);
        assert_eq!(loose.assoc.len(), 1, "only the point that is on an end");
    }

    #[test]
    fn a_radius_dimension_follows_the_centre_and_the_radius() {
        let circle = |c: Point, r: f64| Entity::Circle {
            center: c,
            radius: r,
        };
        let (doc, ids) = document_with(&[circle(p(0.0, 0.0), 5.0)]);
        let d = Dimension::radius(p(0.0, 0.0), p(3.0, 4.0), &DimStyle::default()).associated(&doc);
        assert_eq!(d.assoc.len(), 2);
        let now = |id: EntityId| (id == ids[0]).then(|| circle(p(10.0, 0.0), 8.0));
        let next = follow(&d, &now, false, &|id| id == ids[0]).unwrap();
        assert!((next.measurement().unwrap() - 8.0).abs() < 1e-9);
        assert_eq!(next.points()[0], p(10.0, 0.0));
        assert_eq!(next.assoc.len(), 2);
    }

    #[test]
    fn a_dimension_follows_a_moved_object_and_ties_hold() {
        let (doc, ids) = document_with(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let d = aligned(p(0.0, 0.0), p(10.0, 0.0)).associated(&doc);
        let moved = |id: EntityId| (id == ids[0]).then(|| line(p(0.0, 0.0), p(25.0, 0.0)));
        let next = follow(&d, &moved, false, &|id| id == ids[0]).unwrap();
        assert_eq!(next.measurement(), Some(25.0));
        assert_eq!(next.assoc, d.assoc);
        // Nothing to do when it already matches.
        let same = |id: EntityId| (id == ids[0]).then(|| line(p(0.0, 0.0), p(10.0, 0.0)));
        assert!(follow(&d, &same, false, &|id| id == ids[0]).is_none());
    }

    #[test]
    fn a_dimension_moved_alone_lets_go_and_one_moved_with_its_object_does_not() {
        let (doc, ids) = document_with(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let d = aligned(p(0.0, 0.0), p(10.0, 0.0)).associated(&doc);
        let alone = d.translated(0.0, 30.0);
        let there = |id: EntityId| (id == ids[0]).then(|| line(p(0.0, 0.0), p(10.0, 0.0)));
        let freed = follow(&alone, &there, true, &|_| false).unwrap();
        assert!(freed.assoc.is_empty());
        assert_eq!(freed.points()[0], p(0.0, 30.0), "where it was put");
        let together = |id: EntityId| (id == ids[0]).then(|| line(p(0.0, 30.0), p(10.0, 30.0)));
        assert!(follow(&alone, &together, true, &|id| id == ids[0]).is_none());
    }

    #[test]
    fn an_erased_object_frees_its_points_and_keeps_the_dimension() {
        let (doc, _) = document_with(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let d = aligned(p(0.0, 0.0), p(10.0, 0.0)).associated(&doc);
        let gone = |_: EntityId| None;
        let freed = follow(&d, &gone, false, &|_| true).unwrap();
        assert!(freed.assoc.is_empty());
        assert_eq!(freed.measurement(), Some(10.0));
    }

    #[test]
    fn polyline_vertices_and_rectangle_corners_are_found() {
        let poly = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let (doc, ids) = document_with(&[poly]);
        let d = aligned(p(4.0, 0.0), p(4.0, 3.0)).associated(&doc);
        assert_eq!(d.assoc[0].at, AssocPoint::Vertex(1));
        assert_eq!(d.assoc[1].at, AssocPoint::Vertex(2));
        assert_eq!(d.assoc[0].target, ids[0]);
        let rect = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(6.0, 2.0),
        };
        assert_eq!(
            point_of(&rect, AssocPoint::Vertex(2), p(0.0, 0.0)),
            Some(p(6.0, 2.0))
        );
    }

    #[test]
    fn the_angle_between_two_lines_is_not_tied() {
        let (doc, _) = document_with(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let d = Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 10.0),
            p(5.0, 5.0),
            true,
            &DimStyle::default(),
        )
        .associated(&doc);
        assert!(d.assoc.is_empty());
    }
}
