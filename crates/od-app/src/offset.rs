//! OFFSET: a parallel copy of an object at a distance, or through a point.
//! Written against the `Editor`; the geometry is `Entity::offset` in
//! `od_core`.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked, PointOptions};
use od_core::{Entity, Point};

/// Distances at or below this are refused.
const MIN_DISTANCE: f64 = 1e-9;

/// How far the copy is from the object.
#[derive(Clone, Copy)]
enum Gap {
    /// A fixed distance, on the side the user points to.
    Distance(f64),
    /// As far as the point the user gives.
    Through,
}

/// The copy of `target` for a click at `at`, if there is one.
fn copy_for(target: &Entity, gap: Gap, at: Point) -> Option<Entity> {
    let distance = match gap {
        Gap::Distance(d) => d,
        Gap::Through => target.distance_to(at),
    };
    target.offset(distance, at)
}

/// OFFSET: ask for a distance (or Through), then offset one object after
/// another: click the object, then the side. Enter or Esc ends. Each copy is
/// its own undo step and takes the object's layer and attributes.
pub fn offset_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut erase = false;
    let gap = loop {
        let options = NumberOptions::new("Specify offset distance")
            .keyword("Through", "T")
            .keyword("Erase", "E");
        match ed.ask_distance(options)? {
            Answer::Value(d) if d > MIN_DISTANCE => break Gap::Distance(d),
            Answer::Value(_) => ed.say("Requires a positive value."),
            Answer::Keyword(k) if k == "Through" => break Gap::Through,
            Answer::Keyword(_) => {
                let answer = ed.get_keyword(
                    Options::<String>::new("Erase source object after offsetting?")
                        .keyword("Yes", "Y")
                        .keyword("No", "N")
                        .default("No".to_owned()),
                )?;
                erase = answer == "Yes";
            }
            Answer::Enter => return Err(Cancelled),
        }
    };
    loop {
        let Ok(Answer::Value(Picked { id, .. })) =
            ed.ask_object(Options::<Picked>::new("Select object to offset or <exit>"))
        else {
            return Ok(());
        };
        let Some(object) = ed.read(move |doc| doc.object(id).cloned())? else {
            continue;
        };
        let target = object.entity.clone();
        if matches!(target, Entity::Ellipse(_)) {
            ed.say("OFFSET does not support ellipses yet.");
            continue;
        }
        if matches!(target, Entity::Spline(_)) {
            ed.say("OFFSET does not support splines yet.");
            continue;
        }
        let shown = target.clone();
        let message = match gap {
            Gap::Distance(_) => "Specify point on side to offset",
            Gap::Through => "Specify through point",
        };
        let options = PointOptions::new(message)
            .ghost(move |at| copy_for(&shown, gap, at).into_iter().collect());
        let at = match ed.ask_point(options) {
            Ok(Answer::Value(at)) => at,
            Ok(_) => continue,
            Err(Cancelled) => return Ok(()),
        };
        let Some(copy) = copy_for(&target, gap, at) else {
            ed.say("Cannot offset the object that way.");
            continue;
        };
        let attrs = object.attrs.clone();
        ed.transaction("Offset", move |tx| {
            tx.add_with(vec![(copy, attrs)]);
            if erase {
                tx.remove(vec![id]);
            }
            Ok(())
        })?;
    }
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

    fn offsetting(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("Offset", offset_objects);
        state.settle_command();
        state
    }

    fn click(state: &mut AppState, index: usize, at: Point) {
        let id = state.session().document().objects()[index].id;
        state.answer_object(at, Some(id), false);
        state.settle_command();
    }

    #[test]
    fn a_distance_then_an_object_then_a_side_makes_a_copy_each_time() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("2");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(10.0, 0.0)),
                line(p(0.0, 2.0), p(10.0, 2.0))
            ]
        );
        // Another object straight away, on the other side this time.
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,-9");
        state.settle_command();
        assert_eq!(entities(&state).len(), 3);
        assert_eq!(entities(&state)[2], line(p(0.0, -2.0), p(10.0, -2.0)));
        assert!(state.command_running());
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running(), "Enter ends it");
        // Each copy was one undo step.
        state.undo();
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn through_takes_the_distance_from_a_point() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("t");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        assert_eq!(state.cmd_prompt().text, "Specify through point");
        state.cmd_submit("3,7");
        state.settle_command();
        assert_eq!(entities(&state)[1], line(p(0.0, 7.0), p(10.0, 7.0)));
    }

    #[test]
    fn erase_removes_the_source() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("e");
        state.settle_command();
        state.cmd_submit("y");
        state.settle_command();
        assert!(
            state
                .cmd_prompt()
                .text
                .starts_with("Specify offset distance"),
            "back to asking for the distance"
        );
        state.cmd_submit("2");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 2.0), p(10.0, 2.0))]);
    }

    #[test]
    fn an_ellipse_is_refused_and_the_next_object_asked_for() {
        let e = od_core::Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let mut state = offsetting(&[Entity::Ellipse(e)]);
        state.cmd_submit("1");
        state.settle_command();
        click(&mut state, 0, p(4.0, 0.0));
        assert_eq!(entities(&state), [Entity::Ellipse(e)]);
        let said = state.cmdline.log.last().unwrap();
        assert!(
            said.contains("OFFSET does not support ellipses yet."),
            "{said}"
        );
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
    }

    #[test]
    fn a_spline_is_refused_and_the_next_object_asked_for() {
        let s = Entity::Spline(Box::new(
            od_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)]).unwrap(),
        ));
        let mut state = offsetting(std::slice::from_ref(&s));
        state.cmd_submit("1");
        state.settle_command();
        click(&mut state, 0, p(0.0, 0.0));
        assert_eq!(entities(&state), [s]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("OFFSET does not support splines yet.")));
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
    }

    #[test]
    fn a_circle_that_would_vanish_is_refused_and_asks_for_the_next_object() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let mut state = offsetting(&[circle]);
        state.cmd_submit("5");
        state.settle_command();
        click(&mut state, 0, p(2.0, 0.0));
        state.cmd_submit("0,0");
        state.settle_command();
        assert_eq!(entities(&state).len(), 1);
        assert!(state.cmdline.log.last().unwrap().contains("Cannot offset"));
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
    }

    #[test]
    fn a_zero_distance_is_refused() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("0");
        state.settle_command();
        assert!(state.cmdline.log.last().unwrap().contains("positive"));
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify offset distance"));
    }
}
