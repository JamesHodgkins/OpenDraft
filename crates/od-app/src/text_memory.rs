//! What the text commands remember between uses: the height of new text
//! (`TEXTSIZE`) and where its insertion point sits on its box, saved with the
//! user's settings.

use std::sync::Mutex;

use od_core::mtext::MTextAttachment;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub height: f64,
    pub attachment: MTextAttachment,
}

const DEFAULT: Memory = Memory {
    height: 2.5,
    attachment: MTextAttachment::TopLeft,
};

static MEMORY: Mutex<Memory> = Mutex::new(DEFAULT);

pub fn memory() -> Memory {
    *MEMORY.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn remember(change: impl FnOnce(&mut Memory)) {
    change(&mut MEMORY.lock().unwrap_or_else(|e| e.into_inner()));
}

const HEIGHT_KEY: &str = "text_height";
const ATTACHMENT_KEY: &str = "text_attachment";

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) {
    let Some(storage) = storage else { return };
    let height = storage
        .get_string(HEIGHT_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|h| h.is_finite() && *h > 0.0);
    let attachment = storage
        .get_string(ATTACHMENT_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .and_then(MTextAttachment::from_code);
    remember(|m| {
        m.height = height.unwrap_or(m.height);
        m.attachment = attachment.unwrap_or(m.attachment);
    });
}

pub fn save(storage: &mut dyn eframe::Storage) {
    let m = memory();
    storage.set_string(HEIGHT_KEY, m.height.to_string());
    storage.set_string(ATTACHMENT_KEY, m.attachment.code().to_string());
}

/// The settings are shared by every test that uses them, so those tests take
/// turns, each starting from the defaults.
#[cfg(test)]
pub(crate) fn take_turn_for_tests() -> std::sync::MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    let guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
    remember(|m| *m = DEFAULT);
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
            m.height = 7.5;
            m.attachment = MTextAttachment::BottomRight;
        });
        let mut saved = Saved::default();
        save(&mut saved);
        remember(|m| *m = DEFAULT);
        load(Some(&saved));
        assert_eq!(memory().height, 7.5);
        assert_eq!(memory().attachment, MTextAttachment::BottomRight);

        remember(|m| *m = DEFAULT);
        for (height, attachment) in [("-1", "0"), ("nan", "10"), ("x", "x"), ("0", "-3")] {
            let mut bad = Saved::default();
            bad.set_string(HEIGHT_KEY, height.to_owned());
            bad.set_string(ATTACHMENT_KEY, attachment.to_owned());
            load(Some(&bad));
            assert_eq!(memory(), DEFAULT, "{height} {attachment}");
        }
    }
}
