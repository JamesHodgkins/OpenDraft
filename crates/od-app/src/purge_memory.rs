//! What the Purge dialog remembers between uses, saved with the user's
//! settings: its two switches and the two unnamed-object options.

use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    /// Ask about each item before deleting it.
    pub confirm: bool,
    /// Go on to what deleting the items frees.
    pub nested: bool,
    /// Also delete zero-length lines, arcs, circles and polylines.
    pub zero_length: bool,
    /// Also delete text with nothing in it.
    pub empty_text: bool,
}

const DEFAULT: Memory = Memory {
    confirm: false,
    nested: true,
    zero_length: false,
    empty_text: false,
};

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

static MEMORY: Mutex<Memory> = Mutex::new(DEFAULT);

/// The settings as they are now.
pub fn memory() -> Memory {
    *MEMORY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Change the settings; used by the dialog and loading the user's settings.
pub fn remember(change: impl FnOnce(&mut Memory)) {
    change(&mut MEMORY.lock().unwrap_or_else(|e| e.into_inner()));
}

const KEYS: [&str; 4] = [
    "purge_confirm",
    "purge_nested",
    "purge_zero_length",
    "purge_empty_text",
];

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) {
    let Some(storage) = storage else { return };
    let get = |key: &str| storage.get_string(key).and_then(|v| v.parse::<bool>().ok());
    remember(|m| {
        m.confirm = get(KEYS[0]).unwrap_or(m.confirm);
        m.nested = get(KEYS[1]).unwrap_or(m.nested);
        m.zero_length = get(KEYS[2]).unwrap_or(m.zero_length);
        m.empty_text = get(KEYS[3]).unwrap_or(m.empty_text);
    });
}

/// Write the settings for the next run.
pub fn save(storage: &mut dyn eframe::Storage) {
    let m = memory();
    storage.set_string(KEYS[0], m.confirm.to_string());
    storage.set_string(KEYS[1], m.nested.to_string());
    storage.set_string(KEYS[2], m.zero_length.to_string());
    storage.set_string(KEYS[3], m.empty_text.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage;

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
        let mut saved = Saved::default();
        remember(|m| {
            *m = Memory {
                confirm: true,
                nested: false,
                zero_length: true,
                empty_text: true,
            }
        });
        let wanted = memory();
        save(&mut saved);
        remember(|m| *m = Memory::default());
        load(Some(&saved));
        assert_eq!(memory(), wanted);
        saved.set_string(KEYS[0], "maybe".to_owned());
        remember(|m| *m = Memory::default());
        load(Some(&saved));
        assert!(!memory().confirm, "a bad value keeps the default");
        remember(|m| *m = Memory::default());
    }
}
