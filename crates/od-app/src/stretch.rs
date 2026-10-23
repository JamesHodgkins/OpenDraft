//! STRETCH: move the part of each object inside a crossing window or polygon.
//! The geometry is `Entity::stretched` in `od_core`.

use od_canvas::Region;
use od_commands::{Answer, Cancelled, Editor, GhostStyle, PointOptions};
use od_core::{Entity, EntityId, Point};

use crate::text::plural;

/// Ask for the region: two corners of a crossing window, or the CPolygon
/// option for a polygon of any number of points (Enter closes it).
fn ask_region(ed: &Editor) -> Result<Vec<Point>, Cancelled> {
    let first_options =
        PointOptions::new("Specify first corner of crossing window").keyword("CPolygon", "CP");
    let first = match ed.ask_point(first_options)? {
        Answer::Value(at) => at,
        Answer::Keyword(_) => return ask_polygon(ed),
        Answer::Enter => return Err(Cancelled),
    };
    let options = PointOptions::new("Specify opposite corner")
        .ghost_style(GhostStyle::Crossing)
        .ghost(move |at| {
            vec![Entity::Polyline {
                points: Region::rect_corners(first, at),
                closed: true,
                bulges: Vec::new(),
                widths: Vec::new(),
            }]
        });
    let opposite = ed.get_point(options)?;
    Ok(Region::rect_corners(first, opposite))
}

fn ask_polygon(ed: &Editor) -> Result<Vec<Point>, Cancelled> {
    let mut points: Vec<Point> = Vec::new();
    loop {
        let message = if points.is_empty() {
            "Specify first polygon point"
        } else {
            "Specify next point (Enter to finish)"
        };
        let so_far = points.clone();
        let options = PointOptions::new(message)
            .ghost_style(GhostStyle::Crossing)
            .ghost(move |at| {
                let mut path = so_far.clone();
                path.push(at);
                vec![Entity::Polyline {
                    points: path,
                    closed: true,
                    bulges: Vec::new(),
                    widths: Vec::new(),
                }]
            });
        match ed.ask_point(options)? {
            Answer::Value(at) => {
                if points.last() != Some(&at) {
                    points.push(at);
                }
            }
            Answer::Enter if points.len() >= 3 => return Ok(points),
            _ => ed.say("Specify more points first."),
        }
    }
}

/// STRETCH: choose a crossing window (or CPolygon), then a base point and a
/// second point. Whatever lies entirely inside moves; objects the window
/// crosses have only their points inside it moved. One undo step.
pub fn stretch_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let region = Region::Crossing(ask_region(ed)?);
    let found = {
        let region = region.clone();
        ed.read(move |doc| -> Vec<(EntityId, Entity)> {
            od_canvas::select_in(doc, &region)
                .into_iter()
                .filter_map(|id| doc.object(id).map(|o| (id, o.entity.clone())))
                .collect()
        })?
    };
    let moves = |dx: f64, dy: f64| -> Vec<(EntityId, Entity)> {
        found
            .iter()
            .filter_map(|(id, e)| {
                e.stretched(|p| region.contains(p), dx, dy)
                    .map(|stretched| (*id, stretched))
            })
            .collect()
    };
    if moves(0.0, 0.0).is_empty() {
        ed.say("Nothing to stretch: no object has a point inside the window.");
        return Ok(());
    }
    let base = ed.get_point("Specify base point")?;
    let (shown, inside) = (found.clone(), region.clone());
    let options = PointOptions::new("Specify second point")
        .base(base)
        .ghost(move |at| {
            shown
                .iter()
                .filter_map(|(_, e)| {
                    e.stretched(|p| inside.contains(p), at.x - base.x, at.y - base.y)
                })
                .collect()
        });
    // Enter alone takes the base point as the displacement from the origin.
    let (dx, dy) = match ed.ask_point(options)? {
        Answer::Value(to) => (to.x - base.x, to.y - base.y),
        _ => (base.x, base.y),
    };
    let changes = moves(dx, dy);
    let count = changes.len();
    ed.transaction("Stretch", move |tx| {
        for (id, entity) in changes {
            tx.modify(id, move |_| entity);
        }
        Ok(())
    })?;
    ed.say(format!("Stretched {}", plural(count, "object")));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn stretching() -> AppState {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        state.add_entity(line(p(0.0, 5.0), p(2.0, 5.0)));
        state.add_entity(line(p(20.0, 0.0), p(30.0, 0.0)));
        state.start_command("Stretch", stretch_objects);
        state.settle_command();
        // A window round the left end of the first line and all of the second.
        state.answer_point(p(-1.0, -1.0));
        state.answer_point(p(5.0, 6.0));
        state.answer_point(p(0.0, 0.0));
        state.answer_point(p(0.0, 3.0));
        state.settle_command();
        state
    }

    #[test]
    fn crossed_ends_move_whole_objects_inside_move_and_others_stay() {
        let mut state = stretching();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 3.0), p(10.0, 0.0)),
                line(p(0.0, 8.0), p(2.0, 8.0)),
                line(p(20.0, 0.0), p(30.0, 0.0)),
            ]
        );
        state.undo();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(10.0, 0.0)));
    }

    #[test]
    fn a_window_with_nothing_inside_changes_nothing() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        state.start_command("Stretch", stretch_objects);
        state.settle_command();
        // Crosses the middle of the line, no end inside.
        state.answer_point(p(4.0, -1.0));
        state.answer_point(p(6.0, 1.0));
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
    }
}
