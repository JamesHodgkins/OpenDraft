//! BREAK: remove the part of an object between two points. The geometry is
//! `Entity::break_between` in `od_core`.

use od_commands::{Answer, Cancelled, Editor, Options, Picked, PointOptions};
use od_core::Point;

/// BREAK: click the object (the click is the first break point), then give
/// the second break point. First point re-asks for the first one. Giving the
/// same point twice (`@0,0`) divides the object without removing anything.
/// One undo step.
pub fn break_object(ed: &mut Editor) -> Result<(), Cancelled> {
    // Like AutoCAD, Break takes one object by clicking it, selected or not.
    let Answer::Value(Picked { id, at, .. }) =
        ed.ask_object(Options::<Picked>::new("Select object").allow_empty())?
    else {
        return Ok(());
    };
    let Some(object) = ed.read(move |doc| doc.object(id).cloned())? else {
        return Ok(());
    };
    let target = object.entity.clone();
    if matches!(target, od_core::Entity::Spline(_)) {
        ed.say("BREAK does not support splines yet.");
        return Ok(());
    }
    let mut first = at;
    let mut options =
        break_options(&target, first, "Specify second break point").keyword("First point", "F");
    let second = loop {
        match ed.ask_point(options)? {
            Answer::Value(p) => break p,
            Answer::Keyword(_) => {
                first = ed.get_point(PointOptions::new("Specify first break point"))?;
                options = break_options(&target, first, "Specify second break point");
            }
            Answer::Enter => return Err(Cancelled),
        }
    };
    let Some(mut left) = target.break_between(first, second) else {
        ed.say("Object cannot be broken.");
        return Ok(());
    };
    if left.len() == 1 && left[0] == target {
        ed.say("Nothing to break there.");
        return Ok(());
    }
    let attrs = object.attrs.clone();
    ed.transaction("Break", move |tx| {
        if left.is_empty() {
            tx.remove(vec![id]);
            return Ok(());
        }
        // The first piece keeps the object's identity and place in the draw
        // order; any others are new objects on the same layer.
        let kept = left.remove(0);
        tx.modify(id, move |_| kept);
        if !left.is_empty() {
            tx.add_with(left.into_iter().map(|e| (e, attrs.clone())).collect());
        }
        Ok(())
    })
}

/// A point prompt that previews what is left when the break ends at the
/// pointer.
fn break_options(target: &od_core::Entity, first: Point, message: &str) -> PointOptions {
    let shown = target.clone();
    PointOptions::new(message)
        .base(first)
        .ghost(move |at| shown.break_between(first, at).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::Entity;

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

    fn start(state: &mut AppState) -> od_core::EntityId {
        let id = state.session().document().objects()[0].id;
        state.start_command("Break", break_object);
        state.settle_command();
        id
    }

    #[test]
    fn a_spline_is_refused_by_name() {
        let mut state = AppState::new();
        let s = Entity::Spline(Box::new(
            od_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)]).unwrap(),
        ));
        state.add_entity(s.clone());
        let id = start(&mut state);
        state.answer_object(p(0.0, 0.0), Some(id), false);
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state), [s]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("BREAK does not support splines yet.")));
    }

    #[test]
    fn click_then_second_point_removes_the_part_between_in_one_undo_step() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let id = start(&mut state);
        state.answer_object(p(3.0, 0.0), Some(id), false);
        state.answer_point(p(7.0, 0.0));
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(3.0, 0.0)),
                line(p(7.0, 0.0), p(10.0, 0.0))
            ]
        );
        state.undo();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
    }

    #[test]
    fn the_first_point_option_replaces_the_click() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let id = start(&mut state);
        state.answer_object(p(9.0, 0.0), Some(id), false);
        state.answer_keyword("First point");
        state.answer_point(p(2.0, 0.0));
        state.answer_point(p(5.0, 0.0));
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(2.0, 0.0)),
                line(p(5.0, 0.0), p(10.0, 0.0))
            ]
        );
    }
}
