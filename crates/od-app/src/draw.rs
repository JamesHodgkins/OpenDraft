//! Drawing commands written against the `Editor`. Each segment or shape is
//! added the moment it is complete, so it can be snapped to straight away.

use od_commands::{Answer, Cancelled, Editor, PointOptions};
use od_core::{Entity, Point};

/// Shapes smaller than this (drawing units) are treated as accidental clicks.
pub(crate) const MIN_SIZE: f64 = 1e-6;

pub(crate) fn line(start: Point, end: Point) -> Entity {
    Entity::Line { start, end }
}

pub(crate) fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

pub(crate) fn add(ed: &Editor, entity: Entity, name: &'static str) -> Result<(), Cancelled> {
    ed.transaction(name, move |tx| {
        tx.add(entity);
        Ok(())
    })
}

/// LINE: a chain of segments, each starting where the last ended. Enter or
/// Esc ends the chain and keeps what was drawn; Undo takes back the last
/// segment; Close joins the end to the start.
pub fn draw_line(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut first = ed.get_point("Specify first point")?;
    // The chain's vertices; the segments are between neighbours.
    let mut vertices = vec![first];
    loop {
        let from = vertices.last().copied().unwrap_or(first);
        let mut options = PointOptions::new("Specify next point")
            .dimensions(od_commands::DynamicDimensions::Polar)
            .base(from)
            .keyword("Undo", "U")
            .ghost(move |at| vec![line(from, at)]);
        if vertices.len() > 2 {
            options = options.keyword("Close", "C");
        }
        match ed.ask_point(options) {
            // Esc keeps the segments already drawn, as in AutoCAD.
            Err(Cancelled) | Ok(Answer::Enter) => return Ok(()),
            Ok(Answer::Value(to)) => {
                if distance(from, to) > MIN_SIZE {
                    add(ed, line(from, to), "Line")?;
                    vertices.push(to);
                }
            }
            Ok(Answer::Keyword(k)) if k == "Undo" => {
                if vertices.len() == 1 {
                    first = ed.get_point("Specify first point")?;
                    vertices[0] = first;
                } else {
                    ed.undo()?;
                    vertices.pop();
                }
            }
            Ok(Answer::Keyword(_)) => return add(ed, line(from, first), "Line"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    /// Start `command`, then answer its prompts with typed `inputs`.
    fn run(
        state: &mut AppState,
        command: fn(&mut Editor) -> Result<(), Cancelled>,
        inputs: &[&str],
    ) {
        state.start_command("Test", command);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn line_chains_segments_with_relative_points_until_enter() {
        let mut state = AppState::new();
        run(&mut state, draw_line, &["10,10", "@30,40", "@0,10"]);
        assert_eq!(
            entities(&state),
            [
                line(p(10.0, 10.0), p(40.0, 50.0)),
                line(p(40.0, 50.0), p(40.0, 60.0))
            ]
        );
        assert!(state.command_running(), "still asking for the next point");
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn line_undo_takes_back_the_last_segment() {
        let mut state = AppState::new();
        run(&mut state, draw_line, &["0,0", "5,0", "5,5"]);
        assert!(state.cmd_prompt().options.iter().any(|o| o.label == "Undo"));
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(5.0, 0.0))]);
        state.cmd_submit("u");
        state.settle_command();
        assert!(entities(&state).is_empty());
        assert_eq!(state.waiting().and_then(|ask| ask.base), Some(p(0.0, 0.0)));
        assert!(state.cmd_prompt().options.iter().any(|o| o.label == "Undo"));
        state.cmd_submit("0,5");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(0.0, 5.0))]);
    }

    #[test]
    fn line_close_joins_the_end_to_the_start() {
        let mut state = AppState::new();
        run(&mut state, draw_line, &["0,0", "4,0", "4,3"]);
        state.cmd_submit("c");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state).len(), 3);
        assert_eq!(entities(&state)[2], line(p(4.0, 3.0), p(0.0, 0.0)));
    }

    #[test]
    fn escape_keeps_the_segments_already_drawn() {
        let mut state = AppState::new();
        run(&mut state, draw_line, &["0,0", "5,0"]);
        state.cancel();
        assert!(!state.command_running());
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn each_segment_is_its_own_undo_step_and_clicks_place_points() {
        let mut state = AppState::new();
        state.start_command("Line", draw_line);
        state.settle_command();
        for at in [p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)] {
            state.answer_point(at);
            state.settle_command();
        }
        assert_eq!(entities(&state).len(), 2);
        state.cancel();
        state.undo();
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn a_bare_distance_follows_the_cursor_direction() {
        let mut state = AppState::new();
        run(&mut state, draw_line, &["0,0", "5"]);
        assert!(state.cmdline.log.last().unwrap().contains("direction"));
        state.cursor = Some(p(0.0, 100.0));
        state.cmd_submit("5");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(0.0, 5.0))]);
    }

    #[test]
    fn a_point_on_the_last_one_draws_nothing() {
        let mut state = AppState::new();
        run(&mut state, draw_line, &["1,1", "1,1"]);
        assert!(entities(&state).is_empty());
    }
}
