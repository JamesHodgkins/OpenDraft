//! What FILLET and CHAMFER remember between uses, shared by the commands, the
//! system variables of the same names and the saved settings.

use std::sync::Mutex;

use od_core::Chamfer;

/// What the two commands remember between uses, like AutoCAD's `FILLETRAD`,
/// `CHAMFERA`, `CHAMFERB`, `CHAMFERC`, `CHAMFERD`, `CHAMMODE` and `TRIMMODE`.
/// They are also the system variables of those names and are saved with the
/// user's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub radius: f64,
    /// The two chamfer distances (`CHAMFERA`, `CHAMFERB`).
    pub first: f64,
    pub second: f64,
    /// The chamfer length and angle in degrees (`CHAMFERC`, `CHAMFERD`).
    pub length: f64,
    pub angle: f64,
    /// Chamfer by length and angle rather than two distances (`CHAMMODE`).
    pub by_angle: bool,
    /// Cut the lines back to the joint (`TRIMMODE`).
    pub trim: bool,
}

const DEFAULT: Memory = Memory {
    radius: 10.0,
    first: 10.0,
    second: 10.0,
    length: 10.0,
    angle: 45.0,
    by_angle: false,
    trim: true,
};

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

impl Memory {
    pub fn chamfer(&self) -> Chamfer {
        if self.by_angle {
            Chamfer::Angle {
                length: self.length,
                angle: self.angle.to_radians(),
            }
        } else {
            Chamfer::Distances(self.first, self.second)
        }
    }
}

static MEMORY: Mutex<Memory> = Mutex::new(DEFAULT);

/// The settings as they are now.
pub fn memory() -> Memory {
    *MEMORY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Change the settings; used by the commands' options, the system variables
/// and loading the user's settings.
pub fn remember(change: impl FnOnce(&mut Memory)) {
    change(&mut MEMORY.lock().unwrap_or_else(|e| e.into_inner()));
}

const KEYS: [&str; 7] = [
    "fillet_radius",
    "chamfer_first",
    "chamfer_second",
    "chamfer_length",
    "chamfer_angle",
    "chamfer_by_angle",
    "corner_trim",
];

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) {
    let Some(storage) = storage else { return };
    let number = |key: &str| {
        storage
            .get_string(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|n| n.is_finite() && *n >= 0.0)
    };
    let flag = |key: &str| storage.get_string(key).and_then(|v| v.parse::<bool>().ok());
    remember(|m| {
        m.radius = number(KEYS[0]).unwrap_or(m.radius);
        m.first = number(KEYS[1]).unwrap_or(m.first);
        m.second = number(KEYS[2]).unwrap_or(m.second);
        m.length = number(KEYS[3]).unwrap_or(m.length);
        m.angle = number(KEYS[4]).filter(|a| *a <= 180.0).unwrap_or(m.angle);
        m.by_angle = flag(KEYS[5]).unwrap_or(m.by_angle);
        m.trim = flag(KEYS[6]).unwrap_or(m.trim);
    });
}

/// Write the settings for the next run.
pub fn save(storage: &mut dyn eframe::Storage) {
    let m = memory();
    for (key, value) in KEYS.iter().zip([
        m.radius.to_string(),
        m.first.to_string(),
        m.second.to_string(),
        m.length.to_string(),
        m.angle.to_string(),
        m.by_angle.to_string(),
        m.trim.to_string(),
    ]) {
        storage.set_string(key, value);
    }
}

/// The settings are shared by every test that uses them, so those tests take
/// turns, each starting from the defaults.
#[cfg(test)]
pub(crate) fn take_turn_for_tests() -> std::sync::MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    let guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
    remember(|m| *m = Memory::default());
    guard
}

#[cfg(test)]
mod tests {
    use eframe::Storage;

    use super::*;

    #[derive(Default)]
    struct Saved(std::collections::HashMap<String, String>);

    impl eframe::Storage for Saved {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn the_settings_survive_a_restart_and_bad_saved_values_are_ignored() {
        let _turn = take_turn_for_tests();
        remember(|m| {
            m.radius = 3.5;
            m.by_angle = true;
            m.trim = false;
        });
        let mut saved = Saved::default();
        save(&mut saved);
        remember(|m| *m = Memory::default());
        load(Some(&saved));
        let m = memory();
        assert_eq!((m.radius, m.by_angle, m.trim), (3.5, true, false));
        // Nonsense in the saved file leaves the current value alone.
        saved.set_string("fillet_radius", "not a number".into());
        saved.set_string("chamfer_angle", "500".into());
        saved.set_string("chamfer_first", "-4".into());
        remember(|m| *m = Memory::default());
        load(Some(&saved));
        let m = memory();
        assert_eq!((m.radius, m.angle, m.first), (10.0, 45.0, 10.0));
        load(None);
    }
}
