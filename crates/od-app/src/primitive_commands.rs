//! The commands for the remaining primitives of the Draw panel: Point (and
//! its multiple form), Donut, Construction Line, Ray and Boundary.

use od_commands::Command;

use crate::boundary_cmd::draw_boundary;
use crate::donut_cmd::draw_donut;
use crate::point_cmd::{draw_point, draw_points};
use crate::state::AppState;
use crate::xline_cmd::{draw_ray, draw_xline};

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
        editor!("draw.boundary", "Boundary", "BOUNDARY", draw_boundary),
        editor!("draw.donut", "Donut", "DONUT", draw_donut),
        editor!("draw.ray", "Ray", "RAY", draw_ray),
        editor!("draw.xline", "Construction Line", "XLINE", draw_xline),
        editor!("draw.point", "Point", "POINT", draw_point),
        editor!(
            "draw.point_multiple",
            "Multiple Point",
            "POINT",
            draw_points
        ),
    ]
}
