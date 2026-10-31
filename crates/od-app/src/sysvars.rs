//! The application's system variables (AutoCAD's `GRIDMODE`, `OSMODE`...),
//! bound to [`Settings`](crate::settings::Settings) and the open drawing.
//! The table type lives in `od-commands`; this is the list.
//!
//! Variables that mirror a setting are saved with the user's settings, so
//! they persist with no extra code. Variables saved in the drawing are not
//! built yet (see "System variables" in ARCHITECTURE.md).

use std::sync::OnceLock;

use od_commands::{Kind, SysVar, SysVarTable, Value};

use crate::corner_memory::{memory, remember};
use crate::settings::{
    AUTOSAVE_CHOICES, CROSSHAIR_SIZE_RANGE, GRID_SPACING_RANGE, GRIP_SIZE_RANGE, PICK_SIZE_RANGE,
    SNAP_APERTURE_RANGE,
};
use crate::state::AppState;
use crate::text_memory;

fn flag(on: bool) -> Value {
    Value::Int(i64::from(on))
}

fn int(value: Value) -> i64 {
    match value {
        Value::Int(n) => n,
        _ => 0,
    }
}

/// A pixel size the table has already range-checked.
fn size(value: Value) -> u32 {
    u32::try_from(int(value)).unwrap_or(1)
}

fn range(bounds: (u32, u32)) -> (i64, i64) {
    (i64::from(bounds.0), i64::from(bounds.1))
}

fn build() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "GRIDMODE",
            "Grid display: 0 off, 1 on",
            (0, 1),
            |s| flag(s.settings.show_grid),
            |s, v| {
                s.settings.show_grid = int(v) == 1;
                Ok(())
            },
        ),
        SysVar::real(
            "GRIDUNIT",
            "Grid spacing in drawing units",
            GRID_SPACING_RANGE,
            |s| Value::Real(s.settings.grid_spacing),
            |s, v| {
                if let Value::Real(n) = v {
                    s.settings.grid_spacing = n;
                }
                Ok(())
            },
        ),
        SysVar::int(
            "ORTHOMODE",
            "Ortho: 0 off, 1 on",
            (0, 1),
            |s| flag(s.settings.ortho.enabled),
            |s, v| {
                s.settings.set_ortho(int(v) == 1);
                Ok(())
            },
        ),
        SysVar::int(
            "ORTHOLOCK",
            "How firmly ortho holds: 0 an object snap wins, 1 an object snap is held to the axis",
            (0, 1),
            |s| flag(s.settings.ortho.strict),
            |s, v| {
                s.settings.ortho.strict = int(v) == 1;
                Ok(())
            },
        ),
        SysVar::int(
            "SNAPMODE",
            "Snap to the grid: 0 off, 1 on",
            (0, 1),
            |s| flag(s.settings.snap.grid),
            |s, v| {
                s.settings.snap.grid = int(v) == 1;
                Ok(())
            },
        ),
        SysVar::int(
            "CURSORSIZE",
            "Length of the crosshair lines as a percent of the window",
            range(CROSSHAIR_SIZE_RANGE),
            |s| Value::Int(i64::from(s.settings.crosshair_size)),
            |s, v| {
                s.settings.crosshair_size = size(v);
                Ok(())
            },
        ),
        SysVar::int(
            "PICKBOX",
            "Radius in pixels within which a click picks an object",
            range(PICK_SIZE_RANGE),
            |s| Value::Int(i64::from(s.settings.pick_size)),
            |s, v| {
                s.settings.pick_size = size(v);
                Ok(())
            },
        ),
        SysVar::int(
            "APERTURE",
            "Radius in pixels within which the cursor snaps to geometry",
            range(SNAP_APERTURE_RANGE),
            |s| Value::Int(i64::from(s.settings.snap_aperture)),
            |s, v| {
                s.settings.snap_aperture = size(v);
                Ok(())
            },
        ),
        SysVar::int(
            "GRIPSIZE",
            "Size in pixels of a grip square",
            range(GRIP_SIZE_RANGE),
            |s| Value::Int(i64::from(s.settings.grip_size)),
            |s, v| {
                s.settings.grip_size = size(v);
                Ok(())
            },
        ),
        SysVar::int(
            "SAVETIME",
            "Minutes between automatic saves, 0 for off",
            (0, 10),
            |s| Value::Int(i64::from(s.settings.autosave_minutes)),
            |s, v| {
                let minutes = u32::try_from(int(v)).unwrap_or(u32::MAX);
                if !AUTOSAVE_CHOICES.contains(&minutes) {
                    let choices: Vec<_> = AUTOSAVE_CHOICES.iter().map(u32::to_string).collect();
                    return Err(format!("Requires one of {}.", choices.join(", ")));
                }
                s.settings.autosave_minutes = minutes;
                Ok(())
            },
        ),
        SysVar::real(
            "FILLETRAD",
            "Fillet radius",
            (0.0, 1.0e9),
            |_| Value::Real(memory().radius),
            |_, v| {
                if let Value::Real(n) = v {
                    remember(|m| m.radius = n);
                }
                Ok(())
            },
        ),
        SysVar::int(
            "PEDITACCEPT",
            "1 to turn lines and arcs into polylines in PEDIT without asking",
            (0, 1),
            |_| Value::Int(i64::from(crate::pedit_memory::memory().accept)),
            |_, v| {
                crate::pedit_memory::remember(|m| m.accept = int(v) == 1);
                Ok(())
            },
        ),
        SysVar::real(
            "TEXTSIZE",
            "Height of new text",
            (1.0e-6, 1.0e9),
            |_| Value::Real(text_memory::memory().height),
            |_, v| {
                if let Value::Real(n) = v {
                    text_memory::remember(|m| m.height = n);
                }
                Ok(())
            },
        ),
        SysVar::int(
            "POLYSIDES",
            "Number of sides POLYGON offers first",
            (3, 1024),
            |_| Value::Int(crate::polygon_cmd::sides() as i64),
            |_, v| {
                crate::polygon_cmd::set_sides(usize::try_from(int(v)).unwrap_or(4));
                Ok(())
            },
        ),
        SysVar::real(
            "PLINEWID",
            "Width of the next rectangles",
            (0.0, 1.0e9),
            |st| Value::Real(st.defaults().rect.get().width),
            |st, v| {
                if let Value::Real(n) = v {
                    st.defaults().rect.change(|m| m.width = n);
                }
                Ok(())
            },
        ),
        SysVar::real(
            "ELEVATION",
            "Elevation of the next rectangles (kept, not drawn: 2D only)",
            (-1.0e9, 1.0e9),
            |st| Value::Real(st.defaults().rect.get().elevation),
            |st, v| {
                if let Value::Real(n) = v {
                    st.defaults().rect.change(|m| m.elevation = n);
                }
                Ok(())
            },
        ),
        SysVar::real(
            "THICKNESS",
            "Thickness of the next rectangles (kept, not drawn: 2D only)",
            (-1.0e9, 1.0e9),
            |st| Value::Real(st.defaults().rect.get().thickness),
            |st, v| {
                if let Value::Real(n) = v {
                    st.defaults().rect.change(|m| m.thickness = n);
                }
                Ok(())
            },
        ),
        SysVar::real(
            "CHAMFERA",
            "First chamfer distance",
            (0.0, 1.0e9),
            |_| Value::Real(memory().first),
            |_, v| {
                if let Value::Real(n) = v {
                    remember(|m| m.first = n);
                }
                Ok(())
            },
        ),
        SysVar::real(
            "CHAMFERB",
            "Second chamfer distance",
            (0.0, 1.0e9),
            |_| Value::Real(memory().second),
            |_, v| {
                if let Value::Real(n) = v {
                    remember(|m| m.second = n);
                }
                Ok(())
            },
        ),
        SysVar::real(
            "CHAMFERC",
            "Chamfer length when measured by length and angle",
            (0.0, 1.0e9),
            |_| Value::Real(memory().length),
            |_, v| {
                if let Value::Real(n) = v {
                    remember(|m| m.length = n);
                }
                Ok(())
            },
        ),
        SysVar::real(
            "CHAMFERD",
            "Chamfer angle in degrees when measured by length and angle",
            (0.0, 180.0),
            |_| Value::Real(memory().angle),
            |_, v| {
                if let Value::Real(n) = v {
                    remember(|m| m.angle = n);
                }
                Ok(())
            },
        ),
        SysVar::int(
            "CHAMMODE",
            "Chamfer measured by: 0 two distances, 1 length and angle",
            (0, 1),
            |_| flag(memory().by_angle),
            |_, v| {
                remember(|m| m.by_angle = int(v) == 1);
                Ok(())
            },
        ),
        SysVar::int(
            "TRIMMODE",
            "Fillet and chamfer trim the lines: 0 no, 1 yes",
            (0, 1),
            |_| flag(memory().trim),
            |_, v| {
                remember(|m| m.trim = int(v) == 1);
                Ok(())
            },
        ),
        SysVar::read_only(
            "CMDACTIVE",
            "Whether a command is running: 0 no, 1 yes",
            Kind::Int { min: 0, max: 1 },
            |s| flag(s.command_running()),
        ),
        SysVar::read_only("DWGNAME", "Name of the current drawing", Kind::Text, |s| {
            Value::Text(s.docs[s.active].title.clone())
        }),
    ]
}

/// The table of system variables.
pub fn table() -> &'static SysVarTable<AppState> {
    static TABLE: OnceLock<SysVarTable<AppState>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut variables = build();
        variables.extend(crate::dynamic_settings::variables());
        variables.extend(crate::snap_vars::variables());
        variables.extend(crate::units_vars::variables());
        variables.extend(crate::linetype_vars::variables());
        variables.extend(crate::point_vars::variables());
        variables.extend(crate::dim_vars::variables());
        variables.extend(crate::dim_vars_extra::variables());
        variables.extend(crate::spline_memory::variables());
        variables.extend(crate::hatch_memory::variables());
        SysVarTable::new(variables).unwrap_or_else(|e| panic!("invalid system variable table: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_read_and_write_the_settings() {
        let (table, mut state) = (table(), AppState::new());
        assert_eq!(table.get("GRIDMODE", &state), Ok(Value::Int(1)));
        table.set("gridmode", "0", &mut state).unwrap();
        assert!(!state.settings.show_grid);
        table.set("GRIDUNIT", "25", &mut state).unwrap();
        assert_eq!(state.settings.grid_spacing, 25.0);
        table.set("OSMODE", "16385", &mut state).unwrap();
        assert!(!state.settings.snap.enabled && state.settings.snap.endpoint);
        table.set("SNAPMODE", "0", &mut state).unwrap();
        assert!(!state.settings.snap.grid);
    }

    #[test]
    fn the_pointer_sizes_are_variables_with_ranges() {
        let (table, mut state) = (table(), AppState::new());
        assert_eq!(table.get("PICKBOX", &state), Ok(Value::Int(6)));
        table.set("PICKBOX", "10", &mut state).unwrap();
        table.set("APERTURE", "20", &mut state).unwrap();
        table.set("GRIPSIZE", "9", &mut state).unwrap();
        table.set("CURSORSIZE", "100", &mut state).unwrap();
        assert_eq!(state.settings.crosshair().size, 100);
        assert_eq!(
            state.settings.pointer_sizes(),
            od_canvas::PointerSizes {
                pick: 10.0,
                snap: 20.0,
                grip: 9.0
            }
        );
        assert!(table.set("PICKBOX", "0", &mut state).is_err());
        assert!(table.set("GRIPSIZE", "21", &mut state).is_err());
        assert_eq!(
            state.settings.grip_size, 9,
            "a refused value changes nothing"
        );
    }

    #[test]
    fn savetime_accepts_only_the_intervals_the_settings_offer() {
        let (table, mut state) = (table(), AppState::new());
        assert!(table.set("SAVETIME", "3", &mut state).is_err());
        assert_eq!(state.settings.autosave_minutes, 2);
        table.set("SAVETIME", "5", &mut state).unwrap();
        assert_eq!(state.settings.autosave_minutes, 5);
    }

    #[test]
    fn state_variables_are_read_only() {
        let (table, mut state) = (table(), AppState::new());
        assert_eq!(table.get("CMDACTIVE", &state), Ok(Value::Int(0)));
        assert!(table.set("CMDACTIVE", "1", &mut state).is_err());
        assert!(matches!(
            table.get("DWGNAME", &state),
            Ok(Value::Text(name)) if !name.is_empty()
        ));
    }

    #[test]
    fn the_fillet_and_chamfer_settings_are_variables() {
        let (table, mut state) = (table(), AppState::new());
        let _turn = crate::corner_memory::take_turn_for_tests();
        table.set("FILLETRAD", "4.5", &mut state).unwrap();
        assert_eq!(table.get("FILLETRAD", &state), Ok(Value::Real(4.5)));
        assert_eq!(crate::corner_memory::memory().radius, 4.5);
        table.set("CHAMMODE", "1", &mut state).unwrap();
        assert!(crate::corner_memory::memory().by_angle);
        table.set("CHAMFERD", "30", &mut state).unwrap();
        assert_eq!(crate::corner_memory::memory().angle, 30.0);
        table.set("TRIMMODE", "0", &mut state).unwrap();
        assert!(!crate::corner_memory::memory().trim);
        assert!(
            table.set("CHAMFERD", "200", &mut state).is_err(),
            "out of range"
        );
        assert!(table.set("FILLETRAD", "-1", &mut state).is_err());
        assert!(table.set("TRIMMODE", "2", &mut state).is_err());
    }

    #[test]
    fn peditaccept_is_a_variable() {
        let (table, mut state) = (table(), AppState::new());
        let _turn = crate::pedit_memory::take_turn_for_tests();
        assert_eq!(table.get("PEDITACCEPT", &state), Ok(Value::Int(0)));
        table.set("PEDITACCEPT", "1", &mut state).unwrap();
        assert!(crate::pedit_memory::memory().accept);
        assert!(table.set("PEDITACCEPT", "2", &mut state).is_err());
    }
}
