//! Persisted Dynamic Input controls, with AutoCAD's system-variable values.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DynamicInputSettings {
    pub dynmode: i32,
    pub dynamic_prompts: bool,
    pub pointer_absolute: bool,
    pub pointer_cartesian: bool,
    /// DYNPIVIS: after typing, at point prompts, or always.
    pub pointer_visibility: u8,
}

impl Default for DynamicInputSettings {
    fn default() -> Self {
        Self {
            dynmode: 3,
            dynamic_prompts: true,
            pointer_absolute: false,
            pointer_cartesian: false,
            pointer_visibility: 1,
        }
    }
}

impl DynamicInputSettings {
    pub fn enabled(&self) -> bool {
        self.dynmode > 0
    }
    pub fn pointer(&self) -> bool {
        self.enabled() && self.dynmode & 1 != 0
    }
    pub fn dimensions(&self) -> bool {
        self.enabled() && self.dynmode & 2 != 0
    }
    pub fn toggle(&mut self) {
        self.dynmode = if self.dynmode == 0 { 3 } else { -self.dynmode };
    }

    pub fn sanitized(mut self) -> Self {
        if !(-3..=3).contains(&self.dynmode) {
            self.dynmode = Self::default().dynmode;
        }
        if self.pointer_visibility > 2 {
            self.pointer_visibility = Self::default().pointer_visibility;
        }
        self
    }
}

pub fn variables() -> Vec<od_commands::SysVar<crate::state::AppState>> {
    use od_commands::{SysVar, Value};
    vec![
        SysVar::int(
            "DYNMODE",
            "Dynamic input: 1 pointer, 2 dimensions, 3 both; negative suppresses",
            (-3, 3),
            |s: &crate::state::AppState| Value::Int(s.settings.dynamic_input.dynmode.into()),
            |s, v| {
                if let Value::Int(n) = v {
                    s.settings.dynamic_input.dynmode = n as i32;
                }
                s.docs[s.active].dynamic_input.clear();
                Ok(())
            },
        ),
        SysVar::int(
            "DYNPROMPT",
            "Command prompts at the cursor",
            (0, 1),
            |s| Value::Int(i64::from(s.settings.dynamic_input.dynamic_prompts)),
            |s, v| {
                s.settings.dynamic_input.dynamic_prompts = v == Value::Int(1);
                Ok(())
            },
        ),
        SysVar::int(
            "DYNPICOORDS",
            "Pointer coordinates: 0 relative, 1 absolute",
            (0, 1),
            |s| Value::Int(i64::from(s.settings.dynamic_input.pointer_absolute)),
            |s, v| {
                s.settings.dynamic_input.pointer_absolute = v == Value::Int(1);
                Ok(())
            },
        ),
        SysVar::int(
            "DYNPIFORMAT",
            "Pointer format: 0 polar, 1 Cartesian",
            (0, 1),
            |s| Value::Int(i64::from(s.settings.dynamic_input.pointer_cartesian)),
            |s, v| {
                s.settings.dynamic_input.pointer_cartesian = v == Value::Int(1);
                Ok(())
            },
        ),
        SysVar::int(
            "DYNPIVIS",
            "Pointer visibility: 0 after typing, 1 at prompts, 2 always",
            (0, 2),
            |s| Value::Int(s.settings.dynamic_input.pointer_visibility.into()),
            |s, v| {
                if let Value::Int(n) = v {
                    s.settings.dynamic_input.pointer_visibility = n as u8;
                }
                Ok(())
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_commands::Value;

    #[test]
    fn variables_validate_values_and_toggle_remembers_subfeatures() {
        let mut state = AppState::new();
        let table = crate::sysvars::table();
        for (name, valid, invalid) in [
            ("DYNMODE", "-2", "4"),
            ("DYNPROMPT", "0", "2"),
            ("DYNPICOORDS", "1", "-1"),
            ("DYNPIFORMAT", "1", "2"),
            ("DYNPIVIS", "2", "3"),
        ] {
            table.set(name, valid, &mut state).unwrap();
            assert!(table.set(name, invalid, &mut state).is_err());
            assert_eq!(
                table.get(name, &state),
                Ok(Value::Int(valid.parse().unwrap()))
            );
        }
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        crate::commands::build_registry()
            .execute("view.dynamic_input", &mut state)
            .unwrap();
        assert_eq!(state.settings.dynamic_input.dynmode, 2);
        assert!(state.command_running());
        assert!(
            !state.settings.dynamic_input.pointer() && state.settings.dynamic_input.dimensions()
        );
        assert_eq!(
            DynamicInputSettings {
                dynmode: i32::MIN,
                pointer_visibility: 255,
                ..Default::default()
            }
            .sanitized(),
            DynamicInputSettings::default()
        );
    }
}
