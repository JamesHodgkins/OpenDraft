//! The snap points an entity carries by itself: its ends, middles, centres,
//! quadrants, insertion point and nodes. They do not depend on the pointer,
//! other objects or the command's last point.

use od_core::{arc, bulge, outline, Entity, Object, Point};

use super::{SnapKind, SnapPoint};
use crate::hit::segments;

fn mid(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// Every object-snap point an entity offers on its own.
pub fn candidates(entity: &Entity) -> Vec<SnapPoint> {
    let at = |point, kind| SnapPoint::new(point, kind);
    match entity {
        Entity::Dimension(d) => dimension_candidates(d, &d.picture()),
        // Every leader point and the landing's end, the middle of each
        // segment, and where the text is put.
        // Every vertex, and the middle of each segment.
        Entity::Leader(l) => {
            let mut found: Vec<SnapPoint> = l
                .vertices
                .iter()
                .map(|p| at(*p, SnapKind::Endpoint))
                .collect();
            found.extend(
                l.vertices
                    .windows(2)
                    .map(|w| at(mid(w[0], w[1]), SnapKind::Midpoint)),
            );
            found
        }
        Entity::MLeader(m) => {
            let mut found: Vec<SnapPoint> = m
                .snap_points()
                .into_iter()
                .map(|p| at(p, SnapKind::Endpoint))
                .collect();
            for line in &m.lines {
                found.extend(
                    line.windows(2)
                        .map(|w| at(mid(w[0], w[1]), SnapKind::Midpoint)),
                );
            }
            if let Some(t) = m.text() {
                found.push(at(t.text.insertion, SnapKind::Insertion));
            }
            found
        }
        // Nothing snaps to the lines of a hatch.
        Entity::Hatch(_) => Vec::new(),
        // The insertion point, then the snap points of what it draws.
        Entity::Insert(i) => {
            let mut found = vec![at(i.insertion, SnapKind::Insertion)];
            found.extend(i.picture().iter().flat_map(candidates));
            found
        }
        // Its text point, then the corners of its tag.
        Entity::AttributeDefinition(d) => candidates(&Entity::MText(Box::new(d.tag_view()))),
        // The insertion point and corners, then the edge middles.
        Entity::MText(m) => m
            .snap_points()
            .into_iter()
            .enumerate()
            .map(|(i, p)| match i {
                0 => at(p, SnapKind::Insertion),
                1..=4 => at(p, SnapKind::Endpoint),
                _ => at(p, SnapKind::Midpoint),
            })
            .collect(),
        Entity::Point { at: p } => vec![at(*p, SnapKind::Node)],
        Entity::Ray { origin, .. } => vec![at(*origin, SnapKind::Endpoint)],
        // Nothing of its own: it is met by intersection, nearest and the like.
        Entity::Xline { .. } => Vec::new(),
        Entity::Line { start, end } => vec![
            at(*start, SnapKind::Endpoint),
            at(*end, SnapKind::Endpoint),
            at(mid(*start, *end), SnapKind::Midpoint),
        ],
        Entity::Circle { center, radius } => {
            let mut found = vec![at(*center, SnapKind::Center)];
            found.extend(quadrants(*center, *radius, 0.0, 0.0).map(|q| at(q, SnapKind::Quadrant)));
            found
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let mut found = vec![
                at(arc::point_at(*center, *radius, *start), SnapKind::Endpoint),
                at(arc::point_at(*center, *radius, *end), SnapKind::Endpoint),
                at(
                    arc::midpoint(*center, *radius, *start, *end),
                    SnapKind::Midpoint,
                ),
                at(*center, SnapKind::Center),
            ];
            found.extend(
                quadrants(*center, *radius, *start, *end).map(|q| at(q, SnapKind::Quadrant)),
            );
            found
        }
        // A full ellipse has no ends; an arc's middle is halfway through its
        // parameters.
        Entity::Ellipse(e) => {
            let mut found = if e.is_full() {
                Vec::new()
            } else {
                vec![
                    at(e.start_point(), SnapKind::Endpoint),
                    at(e.end_point(), SnapKind::Endpoint),
                    at(e.mid_point(), SnapKind::Midpoint),
                ]
            };
            found.push(at(e.center, SnapKind::Center));
            // The ends of its axes, where the arc reaches them.
            let quarters = (0..4).map(|q| f64::from(q) * std::f64::consts::FRAC_PI_2);
            found.extend(
                quarters
                    .filter(|t| e.contains(*t))
                    .map(|t| at(e.point_at(t), SnapKind::Quadrant)),
            );
            found
        }
        // An open spline's ends, and on any spline the point halfway along
        // it (left out if its length cannot be found within bounds).
        Entity::Spline(s) => {
            let mut found = if s.closed {
                Vec::new()
            } else {
                vec![
                    at(s.start_point(), SnapKind::Endpoint),
                    at(s.end_point(), SnapKind::Endpoint),
                ]
            };
            found.extend(s.mid_point().ok().map(|m| at(m, SnapKind::Midpoint)));
            found
        }
        Entity::Rectangle { min, max } => {
            let corners = [
                Point::new(min.x, min.y),
                Point::new(max.x, min.y),
                Point::new(max.x, max.y),
                Point::new(min.x, max.y),
            ];
            let mut points: Vec<SnapPoint> =
                corners.iter().map(|c| at(*c, SnapKind::Endpoint)).collect();
            points.extend(
                (0..4).map(|i| at(mid(corners[i], corners[(i + 1) % 4]), SnapKind::Midpoint)),
            );
            points.push(at(mid(*min, *max), SnapKind::Center));
            points
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => {
            let mut found: Vec<SnapPoint> =
                points.iter().map(|p| at(*p, SnapKind::Endpoint)).collect();
            for (i, (a, b)) in segments(points, *closed).enumerate() {
                let curve = bulge::at(bulges, i);
                found.push(at(bulge::midpoint(a, b, curve), SnapKind::Midpoint));
                // An arc segment offers its centre and quadrants, as an arc does.
                if let Some(s) = bulge::arc_of(a, b, curve) {
                    found.push(at(s.center, SnapKind::Center));
                    found.extend(
                        quadrants(s.center, s.radius, s.start, s.end)
                            .map(|q| at(q, SnapKind::Quadrant)),
                    );
                }
            }
            found
        }
    }
}

/// The points east, north, west and south of the centre that the arc from
/// `start` to `end` reaches (equal angles: the whole circle).
fn quadrants(center: Point, radius: f64, start: f64, end: f64) -> impl Iterator<Item = Point> {
    (0..4)
        .map(|q| f64::from(q) * std::f64::consts::FRAC_PI_2)
        .filter(move |a| arc::contains(start, end, *a))
        .map(move |a| arc::point_at(center, radius.abs(), a))
}

/// The points that define a dimension (its nodes), and the tips of its
/// arrows (found among `picture`, what it draws).
fn dimension_candidates(d: &od_core::Dimension, picture: &[Entity]) -> Vec<SnapPoint> {
    let at = |point, kind| SnapPoint::new(point, kind);
    let mut found: Vec<SnapPoint> = d
        .points()
        .into_iter()
        .map(|p| at(p, SnapKind::Node))
        .collect();
    for piece in picture {
        if let Entity::Polyline { points, widths, .. } = piece {
            if points.len() == 2 && widths.first().is_some_and(|w| w.0 > 0.0) {
                found.push(at(points[1], SnapKind::Endpoint));
            }
        }
    }
    found
}

/// [`candidates`] for a stored object: a dimension uses the picture the
/// object remembers, and each point remembers which object it came from.
pub(crate) fn object_candidates(object: &Object) -> Vec<SnapPoint> {
    let found = match (&object.entity, object.dimension_picture()) {
        (Entity::Dimension(d), Some(picture)) => dimension_candidates(d, &picture),
        (entity, _) => candidates(entity),
    };
    found.into_iter().map(|c| c.from(object.id)).collect()
}

/// The centre of the area a closed polyline or closed spline encloses, when
/// there is one: not for an outline that crosses itself or has no area.
/// (Circles, ellipses and rectangles show theirs as their Center.)
pub fn geometric_center(entity: &Entity) -> Option<Point> {
    match entity {
        Entity::Polyline {
            points,
            closed: true,
            bulges,
            ..
        } => outline::path_centroid(points, bulges),
        Entity::Spline(s) if s.closed => {
            let b = s.bounds();
            let size = (b.max.x - b.min.x).hypot(b.max.y - b.min.y);
            let mut ring = s.flatten((size * 1e-3).max(1e-9)).ok()?;
            // A closed spline's flattening ends where it began.
            if ring.len() > 1 && ring.first() == ring.last() {
                ring.pop();
            }
            outline::path_centroid(&ring, &[])
        }
        _ => None,
    }
}
