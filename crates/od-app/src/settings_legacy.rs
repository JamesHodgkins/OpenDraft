//! Reading settings saved before they became one JSON value: a separate
//! storage key for each. Used once, for a user whose storage has no
//! `settings` key yet; the next save writes the new form.

use od_canvas::SnapModes;
use od_ui::Palette;

use crate::dynamic_settings::DynamicInputSettings;
use crate::settings::Settings;

const SNAP_KEYS: [&str; 7] = [
    "snap_enabled",
    "snap_endpoint",
    "snap_midpoint",
    "snap_center",
    "snap_grid",
    "snap_quadrant",
    "snap_nearest",
];

fn parsed<T: std::str::FromStr>(storage: &dyn eframe::Storage, key: &str) -> Option<T> {
    storage.get_string(key).and_then(|v| v.parse().ok())
}

fn dynamic_input(storage: &dyn eframe::Storage) -> DynamicInputSettings {
    let mut value = DynamicInputSettings::default();
    if let Some(mode) = parsed(storage, "dynamic_input_dynmode") {
        value.dynmode = mode;
    }
    if let Some(v) = parsed(storage, "dynamic_input_prompts") {
        value.dynamic_prompts = v;
    }
    if let Some(v) = parsed(storage, "dynamic_input_absolute") {
        value.pointer_absolute = v;
    }
    if let Some(v) = parsed(storage, "dynamic_input_cartesian") {
        value.pointer_cartesian = v;
    }
    if let Some(v) = parsed(storage, "dynamic_input_visibility") {
        value.pointer_visibility = v;
    }
    value
}

/// Settings from the old per-key storage; anything missing or unreadable is
/// the default. Not yet range-checked (see `Settings::sanitized`).
pub fn load(storage: &dyn eframe::Storage) -> Settings {
    let default = Settings::default();
    let flag = |key: &str, fallback: bool| parsed(storage, key).unwrap_or(fallback);
    let d = default.snap;
    Settings {
        dynamic_input: dynamic_input(storage),
        palette: storage
            .get_string("palette")
            .and_then(|key| Palette::from_key(&key))
            .unwrap_or(default.palette),
        show_grid: flag("show_grid", default.show_grid),
        grid_spacing: parsed(storage, "grid_spacing").unwrap_or(default.grid_spacing),
        ui_scale_percent: parsed(storage, "ui_scale_percent").unwrap_or(default.ui_scale_percent),
        autosave_minutes: parsed(storage, "autosave_minutes").unwrap_or(default.autosave_minutes),
        automation_enabled: flag("automation_enabled", default.automation_enabled),
        automation_roots: storage
            .get_string("automation_roots")
            .map(|text| {
                text.lines()
                    .filter(|l| !l.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect()
            })
            .unwrap_or(default.automation_roots),
        snap: SnapModes {
            enabled: flag(SNAP_KEYS[0], d.enabled),
            endpoint: flag(SNAP_KEYS[1], d.endpoint),
            midpoint: flag(SNAP_KEYS[2], d.midpoint),
            center: flag(SNAP_KEYS[3], d.center),
            grid: flag(SNAP_KEYS[4], d.grid),
            quadrant: flag(SNAP_KEYS[5], d.quadrant),
            nearest: flag(SNAP_KEYS[6], d.nearest),
            ..d
        },
        // Added after the move to one value; never saved the old way.
        ..default
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn old_keys_become_settings() {
        let mut storage = MemoryStorage::default();
        for (key, value) in [
            ("palette", "slate"),
            ("show_grid", "false"),
            ("grid_spacing", "2.5"),
            ("ui_scale_percent", "125"),
            ("autosave_minutes", "10"),
            ("automation_enabled", "true"),
            ("automation_roots", "/tmp/a\nC:\\Plans"),
            ("snap_midpoint", "false"),
            ("snap_quadrant", "true"),
            ("dynamic_input_dynmode", "-2"),
            ("dynamic_input_visibility", "2"),
        ] {
            storage.0.insert(key.to_owned(), value.to_owned());
        }
        let settings = load(&storage);
        assert_eq!(settings.palette, Palette::Slate);
        assert!(!settings.show_grid);
        assert_eq!(settings.grid_spacing, 2.5);
        assert_eq!(settings.ui_scale_percent, 125);
        assert_eq!(settings.autosave_minutes, 10);
        assert!(settings.automation_enabled);
        assert_eq!(settings.automation_roots.len(), 2);
        assert!(!settings.snap.midpoint && settings.snap.quadrant);
        assert!(settings.snap.endpoint, "unsaved keys keep their default");
        assert_eq!(settings.dynamic_input.dynmode, -2);
        assert_eq!(settings.dynamic_input.pointer_visibility, 2);
    }

    #[test]
    fn unreadable_old_values_are_the_default() {
        let mut storage = MemoryStorage::default();
        for key in ["palette", "show_grid", "grid_spacing", "snap_endpoint"] {
            storage.0.insert(key.to_owned(), "nonsense".to_owned());
        }
        assert_eq!(load(&storage), Settings::default());
    }
}
