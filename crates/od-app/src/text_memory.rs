//! What the text commands remember between uses: the height of new text
//! (`TEXTSIZE`) and where its insertion point sits on its box, saved with the
//! user's settings.

use od_core::mtext::MTextAttachment;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub height: f64,
    pub attachment: MTextAttachment,
}

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

const DEFAULT: Memory = Memory {
    height: 2.5,
    attachment: MTextAttachment::TopLeft,
};

const HEIGHT_KEY: &str = "text_height";
const ATTACHMENT_KEY: &str = "text_attachment";

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let mut m = DEFAULT;
    let Some(storage) = storage else { return m };
    let height = storage
        .get_string(HEIGHT_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|h| h.is_finite() && *h > 0.0);
    let attachment = storage
        .get_string(ATTACHMENT_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .and_then(MTextAttachment::from_code);
    m.height = height.unwrap_or(m.height);
    m.attachment = attachment.unwrap_or(m.attachment);
    m
}

pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    storage.set_string(HEIGHT_KEY, m.height.to_string());
    storage.set_string(ATTACHMENT_KEY, m.attachment.code().to_string());
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
        let kept = Memory {
            height: 7.5,
            attachment: MTextAttachment::BottomRight,
        };
        let mut saved = Saved::default();
        save(&kept, &mut saved);
        assert_eq!(load(Some(&saved)), kept);

        for (height, attachment) in [("-1", "0"), ("nan", "10"), ("x", "x"), ("0", "-3")] {
            let mut bad = Saved::default();
            bad.set_string(HEIGHT_KEY, height.to_owned());
            bad.set_string(ATTACHMENT_KEY, attachment.to_owned());
            assert_eq!(load(Some(&bad)), DEFAULT, "{height} {attachment}");
        }
    }
}
