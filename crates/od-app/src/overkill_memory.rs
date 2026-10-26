//! What OVERKILL last ran with, offered again next time and saved with the
//! user's settings (never with a drawing).

use std::sync::Mutex;

use od_core::overkill::Settings;

static MEMORY: Mutex<Option<Settings>> = Mutex::new(None);

/// The settings as they are now.
pub fn memory() -> Settings {
    MEMORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or_default()
}

/// Remember `settings` for the next run.
pub fn remember(settings: Settings) {
    *MEMORY.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

const KEYS: [&str; 9] = [
    "overkill_tolerance",
    "overkill_ignore_layer",
    "overkill_ignore_color",
    "overkill_ignore_linetype",
    "overkill_ignore_ltscale",
    "overkill_ignore_lineweight",
    "overkill_optimize",
    "overkill_overlapping",
    "overkill_end_to_end",
];
const ASSOCIATIVE: &str = "overkill_maintain_associative";

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) {
    let Some(storage) = storage else { return };
    let flag = |key: &str| storage.get_string(key).and_then(|v| v.parse::<bool>().ok());
    let mut s = memory();
    let tolerance = storage
        .get_string(KEYS[0])
        .and_then(|v| v.parse::<f64>().ok());
    if let Some(t) = tolerance.filter(|t| t.is_finite() && *t >= 0.0) {
        s.tolerance = t;
    }
    s.ignore.layer = flag(KEYS[1]).unwrap_or(s.ignore.layer);
    s.ignore.color = flag(KEYS[2]).unwrap_or(s.ignore.color);
    s.ignore.linetype = flag(KEYS[3]).unwrap_or(s.ignore.linetype);
    s.ignore.linetype_scale = flag(KEYS[4]).unwrap_or(s.ignore.linetype_scale);
    s.ignore.lineweight = flag(KEYS[5]).unwrap_or(s.ignore.lineweight);
    s.optimize_polylines = flag(KEYS[6]).unwrap_or(s.optimize_polylines);
    s.overlapping = flag(KEYS[7]).unwrap_or(s.overlapping);
    s.end_to_end = flag(KEYS[8]).unwrap_or(s.end_to_end);
    s.maintain_associative = flag(ASSOCIATIVE).unwrap_or(s.maintain_associative);
    remember(s);
}

/// Write the settings for the next run.
pub fn save(storage: &mut dyn eframe::Storage) {
    let s = memory();
    storage.set_string(KEYS[0], s.tolerance.to_string());
    let flags = [
        s.ignore.layer,
        s.ignore.color,
        s.ignore.linetype,
        s.ignore.linetype_scale,
        s.ignore.lineweight,
        s.optimize_polylines,
        s.overlapping,
        s.end_to_end,
    ];
    for (key, on) in KEYS[1..].iter().zip(flags) {
        storage.set_string(key, on.to_string());
    }
    storage.set_string(ASSOCIATIVE, s.maintain_associative.to_string());
}

/// Tests that run OVERKILL or check the saved settings share them, so they
/// take turns, each starting from the defaults.
#[cfg(test)]
pub(crate) fn take_turn_for_tests() -> std::sync::MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    let guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
    remember(Settings::default());
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
    fn the_settings_round_trip_and_bad_values_keep_the_default() {
        let _turn = take_turn_for_tests();
        let mut wanted = Settings {
            tolerance: 0.25,
            optimize_polylines: false,
            maintain_associative: false,
            ..Settings::default()
        };
        wanted.ignore.color = true;
        wanted.ignore.linetype_scale = true;
        remember(wanted);
        let mut saved = Saved::default();
        save(&mut saved);
        remember(Settings::default());
        load(Some(&saved));
        assert_eq!(memory(), wanted);
        saved.set_string(KEYS[0], "-3".to_owned());
        saved.set_string(KEYS[6], "maybe".to_owned());
        remember(Settings::default());
        load(Some(&saved));
        let now = memory();
        assert_eq!(now.tolerance, Settings::default().tolerance);
        assert!(now.optimize_polylines);
        remember(Settings::default());
    }
}
