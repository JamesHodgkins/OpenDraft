//! The dimension commands of the Annotate tab and the command line.

use od_commands::Command;

use crate::dim_override_cmd::{dim_override, dim_update};
use crate::dimension_angular_cmd::dim_angular;
use crate::dimension_cmd::{dim_aligned, dim_arc, dim_auto, dim_diameter, dim_linear, dim_radius};
use crate::dimension_edit_cmd::{dim_edit, dim_text_edit};
use crate::state::AppState;

/// As `editor_command!` in `commands.rs`.
macro_rules! editor_command {
    ($id:literal, $label:literal, $name:literal, $run:path) => {
        Command::new($id, $label, |s: &mut AppState| s.start_command($name, $run))
            .checked(|s: &AppState| s.is_running($name))
            .transparent()
    };
}

/// The commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        editor_command!("dim.dim", "Dimension", "DIM", dim_auto),
        editor_command!("dim.linear", "Linear", "DIMLINEAR", dim_linear),
        editor_command!("dim.aligned", "Aligned", "DIMALIGNED", dim_aligned),
        editor_command!("dim.angular", "Angular", "DIMANGULAR", dim_angular),
        editor_command!("dim.arc", "Arc Length", "DIMARC", dim_arc),
        editor_command!("dim.radius", "Radius", "DIMRADIUS", dim_radius),
        editor_command!("dim.diameter", "Diameter", "DIMDIAMETER", dim_diameter),
        editor_command!(
            "dim.override",
            "Dimension Override",
            "DIMOVERRIDE",
            dim_override
        ),
        editor_command!("dim.update", "Update Dimension", "-DIMSTYLE", dim_update),
        editor_command!("dim.edit", "Dimension Edit", "DIMEDIT", dim_edit),
        editor_command!(
            "dim.textedit",
            "Dimension Text Edit",
            "DIMTEDIT",
            dim_text_edit
        ),
    ]
}
