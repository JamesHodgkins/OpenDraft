//! POINT: place point objects. `draw_point` places one, as typing POINT does;
//! `draw_points` keeps asking until Enter or Esc, as the ribbon's Multiple
//! Point does. How they look is the drawing's `PDMODE` and `PDSIZE`
//! (`point_vars.rs`).

use od_commands::{Answer, Cancelled, Editor, PointOptions};
use od_core::{Entity, PointStyle};

use crate::draw::add;

fn ask(ed: &Editor) -> Result<Answer<od_core::Point>, Cancelled> {
    ed.ask_point(PointOptions::new("Specify a point").ghost(|at| vec![Entity::Point { at }]))
}

/// The reminder AutoCAD prints when POINT starts.
fn say_modes(ed: &Editor) -> Result<(), Cancelled> {
    let PointStyle { mode, size } = ed.read(|doc| doc.point_style())?;
    ed.say(format!("Current point modes: PDMODE={mode} PDSIZE={size}"));
    Ok(())
}

/// POINT.
pub fn draw_point(ed: &mut Editor) -> Result<(), Cancelled> {
    say_modes(ed)?;
    match ask(ed)? {
        Answer::Value(at) => add(ed, Entity::Point { at }, "Point"),
        _ => Ok(()),
    }
}

/// Multiple Point: one undo step for each point.
pub fn draw_points(ed: &mut Editor) -> Result<(), Cancelled> {
    say_modes(ed)?;
    loop {
        match ask(ed) {
            Ok(Answer::Value(at)) => add(ed, Entity::Point { at }, "Point")?,
            Ok(_) | Err(Cancelled) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::Point;

    use super::*;
    use crate::state::AppState;

    fn points(state: &AppState) -> Vec<Point> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match o.entity {
                Entity::Point { at } => Some(at),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn point_places_one_and_ends() {
        let mut state = AppState::new();
        state.start_command("POINT", draw_point);
        state.settle_command();
        state.cmd_submit("3,4");
        state.settle_command();
        assert_eq!(points(&state), [Point::new(3.0, 4.0)]);
        assert!(!state.command_running());
        state.undo();
        assert!(points(&state).is_empty());
    }

    #[test]
    fn multiple_point_keeps_asking_until_enter() {
        let mut state = AppState::new();
        state.start_command("POINT", draw_points);
        state.settle_command();
        for text in ["0,0", "5,5", "@1,1"] {
            state.cmd_submit(text);
            state.settle_command();
        }
        assert_eq!(points(&state).len(), 3);
        assert!(state.command_running());
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(points(&state).len(), 3);
    }
}
