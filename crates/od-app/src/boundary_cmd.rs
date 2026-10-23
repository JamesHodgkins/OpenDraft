//! BOUNDARY: closed polylines from the area around a picked point, as
//! `-BOUNDARY` makes them. The tracing is the hatch engine's
//! (`od_core::hatch::trace`) and the conversion `od_core::loop_polyline`; this
//! only asks.
//!
//! Each point picked adds the polyline round the area it is in and, with
//! island detection on (the default), one for each island inside it, as one
//! undo step. The Region object type is not built.

use od_commands::{Answer, Cancelled, Editor, PointOptions};
use od_core::hatch::trace;
use od_core::loop_polyline::loop_polyline;
use od_core::{Bounds, Entity, Point};

use crate::text::plural;

/// The polylines round the area `at` is in, outer boundary first.
fn around(ed: &Editor, at: Point, islands: bool) -> Result<Vec<Entity>, Cancelled> {
    let drawn: Vec<Entity> = ed.read(|doc| {
        doc.visible_objects()
            .map(|o| o.entity.clone())
            .filter(|e| e.infinite_line().is_none())
            .collect()
    })?;
    let extent = drawn
        .iter()
        .map(Entity::bounds)
        .filter(Bounds::is_finite)
        .reduce(|a, b| a.union(b))
        .map_or(1.0, |b| (b.max.x - b.min.x).hypot(b.max.y - b.min.y));
    // Curves are followed to a hundred-thousandth of the drawing's size.
    let tolerance = (extent * 1e-5).max(1e-9);
    match trace(&drawn, at, tolerance) {
        Ok(found) => {
            let wanted = if islands { found.loops.len() } else { 1 };
            Ok(found
                .loops
                .iter()
                .take(wanted)
                .filter_map(|l| loop_polyline(l, tolerance))
                .collect())
        }
        Err(why) => {
            ed.say(format!("{why}."));
            Ok(Vec::new())
        }
    }
}

/// BOUNDARY. See the module notes.
pub fn draw_boundary(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut islands = true;
    loop {
        let options = PointOptions::new("Pick internal point").keyword("Island detection", "I");
        match ed.ask_point(options) {
            Ok(Answer::Value(at)) => {
                let made = around(ed, at, islands)?;
                if made.is_empty() {
                    continue;
                }
                let count = made.len();
                ed.transaction("Boundary", move |tx| {
                    for polyline in made {
                        tx.add(polyline);
                    }
                    Ok(())
                })?;
                ed.say(format!("{} created.", plural(count, "polyline")));
            }
            Ok(Answer::Keyword(_)) => {
                islands = !islands;
                ed.say(format!(
                    "Island detection is {}.",
                    if islands { "on" } else { "off" }
                ));
            }
            Ok(Answer::Enter) | Err(Cancelled) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn square(state: &mut AppState, min: f64, max: f64) {
        state.perform(od_commands::AddEntity::new(Entity::Rectangle {
            min: Point::new(min, min),
            max: Point::new(max, max),
        }));
    }

    fn polylines(state: &AppState) -> usize {
        let objects = state.session().document().objects();
        objects
            .iter()
            .filter(|o| matches!(o.entity, Entity::Polyline { .. }))
            .count()
    }

    #[test]
    fn a_point_inside_a_shape_gets_its_outline_and_its_islands() {
        let mut state = AppState::new();
        square(&mut state, 0.0, 10.0);
        square(&mut state, 3.0, 5.0);
        state.start_command("BOUNDARY", draw_boundary);
        state.settle_command();
        state.cmd_submit("8,8");
        state.settle_command();
        assert_eq!(polylines(&state), 2, "the outline and the island");
        state.undo();
        assert_eq!(polylines(&state), 0, "one undo step");
    }

    #[test]
    fn island_detection_off_makes_only_the_outline() {
        let mut state = AppState::new();
        square(&mut state, 0.0, 10.0);
        square(&mut state, 3.0, 5.0);
        state.start_command("BOUNDARY", draw_boundary);
        state.settle_command();
        state.cmd_submit("i");
        state.settle_command();
        state.cmd_submit("8,8");
        state.settle_command();
        assert_eq!(polylines(&state), 1);
    }

    #[test]
    fn a_point_outside_everything_makes_nothing() {
        let mut state = AppState::new();
        square(&mut state, 0.0, 10.0);
        state.start_command("BOUNDARY", draw_boundary);
        state.settle_command();
        state.cmd_submit("50,50");
        state.settle_command();
        assert_eq!(polylines(&state), 0);
        assert!(state.command_running(), "still asking");
    }
}
