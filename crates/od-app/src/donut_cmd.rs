//! DONUT: a filled ring, from an inside and an outside diameter, placed as
//! often as wanted. The geometry is `od_core::donut`.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::donut::donut;
use od_core::{Entity, Point};

use crate::defaults::EditorDefaults;
use crate::draw::add;

/// The diameters DONUT offers first; they last the session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub inside: f64,
    pub outside: f64,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            inside: 0.5,
            outside: 1.0,
        }
    }
}

/// DONUT: the diameters, then centres until Enter or Esc, one undo step each.
pub fn draw_donut(ed: &mut Editor) -> Result<(), Cancelled> {
    let (inside, outside) = loop {
        let was = ed.defaults().donut.get();
        let (was_inside, was_outside) = (was.inside, was.outside);
        let inside = ed.get_distance(
            NumberOptions::new("Specify inside diameter of donut").default(was_inside),
        )?;
        let outside = ed.get_distance(
            NumberOptions::new("Specify outside diameter of donut").default(was_outside),
        )?;
        if donut(Point::default(), inside, outside).is_some() {
            ed.defaults()
                .donut
                .change(|m| *m = Memory { inside, outside });
            break (inside, outside);
        }
        ed.say("Requires a non-negative inside diameter and an outside diameter above zero.");
    };
    loop {
        let options = PointOptions::new("Specify center of donut or <exit>").ghost(move |at| {
            donut(at, inside, outside)
                .into_iter()
                .collect::<Vec<Entity>>()
        });
        match ed.ask_point(options) {
            Ok(Answer::Value(at)) => {
                if let Some(ring) = donut(at, inside, outside) {
                    add(ed, ring, "Donut")?;
                }
            }
            Ok(_) | Err(Cancelled) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn count(state: &AppState) -> usize {
        state.session().document().objects().len()
    }

    #[test]
    fn donut_asks_the_diameters_then_places_rings_until_enter() {
        let mut state = AppState::new();
        state.start_command("DONUT", draw_donut);
        state.settle_command();
        for text in ["2", "6", "0,0", "10,0"] {
            state.cmd_submit(text);
            state.settle_command();
        }
        assert_eq!(count(&state), 2);
        assert!(state.command_running());
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running());
        state.undo();
        assert_eq!(count(&state), 1);
    }

    #[test]
    fn a_bad_pair_of_diameters_is_asked_again() {
        let mut state = AppState::new();
        state.start_command("DONUT", draw_donut);
        state.settle_command();
        for text in ["0", "0", "1", "3", "5,5"] {
            state.cmd_submit(text);
            state.settle_command();
        }
        assert_eq!(count(&state), 1);
    }
}
