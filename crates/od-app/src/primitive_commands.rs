//! The commands for the remaining primitives of the Draw panel: Point (and
//! its multiple form) and Donut.

use od_commands::Command;

use crate::donut_cmd::draw_donut;
use crate::point_cmd::{draw_point, draw_points};
use crate::state::AppState;

/// As `editor_command!` in `commands.rs`.
macro_rules! editor {
    ($id:literal, $label:literal, $name:literal, $run:path) => {
        Command::new($id, $label, |s: &mut AppState| s.start_command($name, $run))
            .checked(|s: &AppState| s.is_running($name))
            .transparent()
    };
}

/// The commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        editor!("draw.donut", "Donut", "DONUT", draw_donut),
        editor!("draw.point", "Point", "POINT", draw_point),
        editor!(
            "draw.point_multiple",
            "Multiple Point",
            "POINT",
            draw_points
        ),
    ]
}
