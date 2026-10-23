//! JOIN: turn lines, arcs and open polylines that meet end to end into one
//! object, and elliptical arcs of one ellipse into one arc or the whole
//! ellipse. The geometry is `od_core::join`.

use od_commands::{Cancelled, Editor};
use od_core::{join, Entity, EntityId};

use crate::text::plural;

/// Ends closer than this count as meeting.
const GAP: f64 = 1e-6;

/// JOIN: select the objects (or use those already selected), and whatever
/// meets end to end becomes one object. The first of each group keeps its
/// layer and place in the drawing order; the rest are removed. One undo step.
pub fn join_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects to join")?;
    let ids = chosen.clone();
    let found = ed.read(move |doc| -> Vec<(EntityId, Entity)> {
        ids.iter()
            .filter_map(|id| doc.object(*id).map(|o| (*id, o.entity.clone())))
            .collect()
    })?;
    let entities: Vec<Entity> = found.iter().map(|(_, e)| e.clone()).collect();
    // Nothing changes when any is refused, so the selection stays as it was.
    if entities.iter().any(|e| matches!(e, Entity::Spline(_))) {
        ed.say("JOIN does not support splines yet.");
        return Ok(());
    }
    let merged = join(&entities, GAP);
    if merged.is_empty() {
        ed.say("Nothing to join: the objects do not meet end to end.");
        return Ok(());
    }
    let gone: usize = merged.iter().map(|m| m.used.len()).sum();
    let groups = merged.len();
    ed.transaction("Join", move |tx| {
        for m in merged {
            let first = found[m.used[0]].0;
            let entity = m.entity;
            tx.modify(first, move |_| entity);
            let rest: Vec<EntityId> = m.used[1..].iter().map(|i| found[*i].0).collect();
            if !rest.is_empty() {
                tx.remove(rest);
            }
        }
        Ok(())
    })?;
    ed.say(format!(
        "Joined {} into {}.",
        plural(gone, "object"),
        plural(groups, "object")
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::state::AppState;
    use od_core::Point;

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

    fn joining(items: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in items {
            state.add_entity(e.clone());
        }
        state.select_all();
        state.start_command("Join", join_objects);
        state.settle_command();
        state
    }

    #[test]
    fn selected_lines_that_meet_become_one_polyline_in_one_undo_step() {
        let mut state = joining(&[
            line(p(0.0, 0.0), p(5.0, 0.0)),
            line(p(5.0, 0.0), p(5.0, 5.0)),
            line(p(20.0, 20.0), p(21.0, 20.0)),
        ]);
        let now = entities(&state);
        assert_eq!(now.len(), 2);
        assert!(matches!(&now[0], Entity::Polyline { points, .. } if points.len() == 3));
        assert_eq!(now[1], line(p(20.0, 20.0), p(21.0, 20.0)));
        state.undo();
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn elliptical_arcs_join_among_themselves_and_lines_among_theirs() {
        let half = |start: f64| {
            let e = od_core::Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, start, start + PI);
            Entity::Ellipse(e.unwrap())
        };
        let items = [
            line(p(0.0, 0.0), p(1.0, 0.0)),
            half(0.0),
            line(p(1.0, 0.0), p(1.0, 1.0)),
            half(PI),
        ];
        let state = joining(&items);
        let all = entities(&state);
        assert_eq!(all.len(), 2, "{all:?}");
        assert!(matches!(all[0], Entity::Polyline { .. }));
        assert!(
            matches!(all[1], Entity::Ellipse(e) if e.is_full()),
            "two halves close it"
        );
    }

    #[test]
    fn a_selection_with_a_spline_is_refused_whole() {
        let items = [
            line(p(-5.0, 0.0), p(-6.0, 0.0)),
            line(p(-6.0, 0.0), p(-6.0, 1.0)),
            Entity::Spline(Box::new(
                od_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)]).unwrap(),
            )),
        ];
        let state = joining(&items);
        assert_eq!(entities(&state), items, "the lines are not joined either");
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("JOIN does not support splines yet.")));
    }

    #[test]
    fn nothing_meeting_changes_nothing() {
        let state = joining(&[
            line(p(0.0, 0.0), p(1.0, 0.0)),
            line(p(5.0, 0.0), p(6.0, 0.0)),
        ]);
        assert_eq!(entities(&state).len(), 2);
    }
}
