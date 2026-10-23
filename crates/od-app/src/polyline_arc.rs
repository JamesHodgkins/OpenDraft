//! The arc options of PLINE: each one finishes an arc segment from the
//! polyline's last point, as the end point and bulge the chain stores.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::arc::midpoint;
use od_core::arc_make::{from_start_end_angle, from_start_end_direction, from_start_end_radius};
use od_core::{bulge, Entity, Point};

use crate::arc_cmd::{arc_through, centre_after, shown, to_end_from_centre};
use crate::draw::{distance, line, MIN_SIZE};

/// A finished arc segment: where it ends and its bulge.
pub(crate) type Segment = (Point, f64);

/// The segment for `arc` (counter-clockwise, as entities are) travelled from
/// `from`: it ends at whichever of the arc's ends is not `from`.
pub(crate) fn segment_of(from: Point, arc: &Entity) -> Option<Segment> {
    let Entity::Arc {
        center,
        radius,
        start,
        end,
    } = arc
    else {
        return None;
    };
    let (s, e) = (
        od_core::arc::point_at(*center, *radius, *start),
        od_core::arc::point_at(*center, *radius, *end),
    );
    let to = if distance(from, s) <= distance(from, e) {
        e
    } else {
        s
    };
    let bulge = bulge::through(from, midpoint(*center, *radius, *start, *end), to)?;
    Some((to, bulge))
}

fn preview(from: Point, arc: Option<Entity>, at: Point) -> Vec<Entity> {
    shown(arc, from, at)
}

/// An arc about `from` that bends towards the side `end` is on, relative to
/// the way the polyline is heading.
fn radius_arc(from: Point, end: Point, heading: Point, radius: f64) -> Option<Entity> {
    let cross = heading.x * (end.y - from.y) - heading.y * (end.x - from.x);
    if cross < 0.0 {
        // Bending right: the same arc as the left-bending one from the end.
        from_start_end_radius(end, from, radius)
    } else {
        from_start_end_radius(from, end, radius)
    }
}

/// The chord of an arc of `radius` turning through `angle`, leaving `from`
/// along `direction`: its end point.
fn chord_end(from: Point, radius: f64, angle: f64, direction: f64) -> Point {
    let chord = 2.0 * radius * (angle.abs() / 2.0).sin();
    Point::new(
        from.x + chord * direction.cos(),
        from.y + chord * direction.sin(),
    )
}

/// Finish the arc segment for one of the arc-mode keywords, or `None` when
/// the answers make no arc (the caller says so and asks again).
pub(crate) fn arc_option(
    ed: &Editor,
    keyword: &str,
    from: Point,
    heading: Point,
) -> Result<Option<Segment>, Cancelled> {
    let arc = match keyword {
        "Angle" => by_angle(ed, from)?,
        "CEnter" => {
            let center = centre_after(ed, from)?;
            Some(to_end_from_centre(ed, center, from)?)
        }
        "Direction" => by_direction(ed, from)?,
        "Radius" => by_radius(ed, from, heading)?,
        _ => by_second_point(ed, from)?,
    };
    Ok(arc.and_then(|arc| segment_of(from, &arc)))
}

fn by_angle(ed: &Editor, from: Point) -> Result<Option<Entity>, Cancelled> {
    let angle = ed.get_angle(NumberOptions::new("Specify included angle"))?;
    let options = PointOptions::new("Specify endpoint of arc")
        .base(from)
        .keyword("CEnter", "CE")
        .keyword("Radius", "R")
        .ghost(move |at| preview(from, from_start_end_angle(from, at, angle), at));
    Ok(match ed.ask_point(options)? {
        Answer::Value(end) => from_start_end_angle(from, end, angle),
        Answer::Keyword(k) if k == "CEnter" => {
            let center = centre_after(ed, from)?;
            od_core::arc_make::from_center_start_angle(center, from, angle)
        }
        Answer::Keyword(_) => {
            let radius = positive(ed, NumberOptions::new("Specify radius of arc").base(from))?;
            let direction = ed.get_angle(
                NumberOptions::new("Specify direction of chord")
                    .default(0.0)
                    .base(from),
            )?;
            from_start_end_angle(from, chord_end(from, radius, angle, direction), angle)
        }
        Answer::Enter => return Err(Cancelled),
    })
}

fn by_direction(ed: &Editor, from: Point) -> Result<Option<Entity>, Cancelled> {
    let direction = ed.get_angle(
        NumberOptions::new("Specify the tangent direction for the start point of arc").base(from),
    )?;
    let options = PointOptions::new("Specify endpoint of arc")
        .base(from)
        .ghost(move |at| preview(from, from_start_end_direction(from, at, direction), at));
    let end = ed.get_point(options)?;
    Ok(from_start_end_direction(from, end, direction))
}

fn by_radius(ed: &Editor, from: Point, heading: Point) -> Result<Option<Entity>, Cancelled> {
    let radius = positive(ed, NumberOptions::new("Specify radius of arc").base(from))?;
    let options = PointOptions::new("Specify endpoint of arc")
        .base(from)
        .keyword("Angle", "A")
        .ghost(move |at| preview(from, radius_arc(from, at, heading, radius), at));
    Ok(match ed.ask_point(options)? {
        Answer::Value(end) => radius_arc(from, end, heading, radius),
        Answer::Keyword(_) => {
            let angle = ed.get_angle(NumberOptions::new("Specify included angle"))?;
            let direction = ed.get_angle(
                NumberOptions::new("Specify direction of chord")
                    .default(0.0)
                    .base(from),
            )?;
            from_start_end_angle(from, chord_end(from, radius, angle, direction), angle)
        }
        Answer::Enter => return Err(Cancelled),
    })
}

fn by_second_point(ed: &Editor, from: Point) -> Result<Option<Entity>, Cancelled> {
    let options = PointOptions::new("Specify second point on arc")
        .base(from)
        .ghost(move |at| vec![line(from, at)]);
    let second = ed.get_point(options)?;
    let options = PointOptions::new("Specify end point of arc")
        .base(second)
        .ghost(move |at| preview(from, arc_through(from, second, at), at));
    let end = ed.get_point(options)?;
    Ok(arc_through(from, second, end))
}

/// A distance above zero; asks again for anything else.
fn positive(ed: &Editor, options: NumberOptions) -> Result<f64, Cancelled> {
    loop {
        let value = ed.get_distance(options.clone())?;
        if value > MIN_SIZE {
            return Ok(value);
        }
        ed.say("Requires a non-zero value.");
    }
}
