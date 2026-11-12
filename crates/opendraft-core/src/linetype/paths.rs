//! The curves a linetype pattern is laid along, flattened to points.

use std::f64::consts::TAU;

use crate::{arc, bulge, Entity, Point};

/// A curve that would take more points than we are willing to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooDetailed;

/// Whether a linetype applies to this kind of entity. Text, hatches, dimensions
/// and block references are made of other things and are not dashed as a whole.
pub fn is_dashable(entity: &Entity) -> bool {
    matches!(
        entity,
        Entity::Line { .. }
            | Entity::Circle { .. }
            | Entity::Arc { .. }
            | Entity::Ellipse(_)
            | Entity::Spline(_)
            | Entity::Rectangle { .. }
            | Entity::Polyline { .. }
    )
}

/// A flattened path: its points, and whether it closes back to the first.
pub type Outline = (Vec<Point>, bool);

/// A dashable entity as flattened paths (points, closed), curves within
/// `tolerance`; `None` for any other kind. A closed path does not repeat its
/// first point. A circle starts at angle 0. With `per_segment` a polyline or
/// rectangle comes back as one path per segment, so a pattern laid on each
/// starts and ends with a dash on every one (AutoCAD's default); without it
/// they are one path.
pub fn curve_outlines(
    entity: &Entity,
    tolerance: f64,
    per_segment: bool,
) -> Result<Option<Vec<Outline>>, TooDetailed> {
    Ok(Some(match entity {
        Entity::Line { start, end } => vec![(vec![*start, *end], false)],
        Entity::Circle { center, radius } => {
            let mut ring = arc::flatten(*center, *radius, 0.0, TAU, tolerance);
            ring.pop(); // the last point repeats the first
            vec![(ring, true)]
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => vec![(
            arc::flatten(*center, *radius, *start, *end, tolerance),
            false,
        )],
        Entity::Ellipse(e) => {
            let mut points = e.flatten(tolerance).map_err(|_| TooDetailed)?;
            if e.is_full() {
                points.pop(); // the last point repeats the first
            }
            vec![(points, e.is_full())]
        }
        Entity::Spline(s) => {
            let mut points = s
                .flatten_within(tolerance, crate::spline::MAX_PLOT_POINTS)
                .map_err(|_| TooDetailed)?;
            if s.closed && points.len() > 2 {
                points.pop(); // the last point repeats the first
            }
            vec![(points, s.closed)]
        }
        Entity::Rectangle { min, max } => {
            let corners = vec![
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
            ];
            if per_segment {
                sides(&corners, &[], true, tolerance)
            } else {
                vec![(corners, true)]
            }
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => {
            if per_segment {
                sides(points, bulges, *closed, tolerance)
            } else {
                vec![(
                    bulge::flatten_path(points, bulges, *closed, tolerance),
                    *closed,
                )]
            }
        }
        _ => return Ok(None),
    }))
}

/// Each segment of a polyline (a line or an arc) as an open path of its own.
fn sides(points: &[Point], bulges: &[f64], closed: bool, tolerance: f64) -> Vec<Outline> {
    (0..bulge::segment_count(points.len(), closed))
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            (bulge::flatten(a, b, bulge::at(bulges, i), tolerance), false)
        })
        .collect()
}
