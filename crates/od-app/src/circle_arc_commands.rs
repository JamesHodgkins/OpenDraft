//! The Circle and Arc commands of the Draw panel's lists, one per type
//! (`draw.circle_2p`, `draw.arc_sce`...), each starting at its own prompt.

use od_commands::Command;

use crate::arc_cmd::{draw_arc, draw_arc_sce};
use crate::circle::{
    draw_circle, draw_circle_2p, draw_circle_3p, draw_circle_diameter, draw_circle_ttr,
};
use crate::state::AppState;

/// As `editor_command!` in `commands.rs`; every type of a shape runs under
/// the shape's name, so the button lights while any of them runs.
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
        editor!(
            "draw.circle",
            "Circle, Center, Radius",
            "Circle",
            draw_circle
        ),
        editor!(
            "draw.circle_diameter",
            "Circle, Center, Diameter",
            "Circle",
            draw_circle_diameter
        ),
        editor!(
            "draw.circle_2p",
            "Circle, 2-Point",
            "Circle",
            draw_circle_2p
        ),
        editor!(
            "draw.circle_3p",
            "Circle, 3-Point",
            "Circle",
            draw_circle_3p
        ),
        editor!(
            "draw.circle_ttr",
            "Circle, Tan, Tan, Radius",
            "Circle",
            draw_circle_ttr
        ),
        editor!("draw.arc", "Arc, 3-Point", "Arc", draw_arc),
        editor!(
            "draw.arc_sce",
            "Arc, Start, Center, End",
            "Arc",
            draw_arc_sce
        ),
    ]
}
