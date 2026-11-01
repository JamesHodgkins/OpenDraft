//! What the attribute commands remember between uses: the modes new
//! definitions start with (`AFLAGS`) and whether INSERT asks for attribute
//! values (`ATTREQ`), saved with the user's settings.

use od_core::block::AttributeFlags;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    /// The modes ATTDEF starts with: invisible 1, constant 2, verify 4,
    /// preset 8 (the same bits as a definition's flags).
    pub flags: u16,
    /// Whether the position of a new definition is locked.
    pub lock_position: bool,
    /// Whether INSERT asks for the values (otherwise it uses the defaults).
    pub prompt_on_insert: bool,
}

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

const DEFAULT: Memory = Memory {
    flags: 0,
    // AutoCAD's Attribute Definition dialog opens with Lock position ticked.
    lock_position: true,
    prompt_on_insert: true,
};

/// The bits `AFLAGS` may hold: the four modes and lock position (16).
pub const AFLAGS_MASK: u16 = 31;

const FLAGS_KEY: &str = "attribute_flags";
const PROMPT_KEY: &str = "attribute_prompt";

impl Memory {
    /// `AFLAGS` as AutoCAD numbers it: the mode bits, and 16 for a locked
    /// position.
    pub fn aflags(&self) -> u16 {
        self.flags | if self.lock_position { 16 } else { 0 }
    }

    pub fn set_aflags(&mut self, value: u16) {
        self.flags = value
            & (AttributeFlags::INVISIBLE
                | AttributeFlags::CONSTANT
                | AttributeFlags::VERIFY
                | AttributeFlags::PRESET);
        self.lock_position = value & 16 != 0;
    }
}

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let mut m = DEFAULT;
    let Some(storage) = storage else { return m };
    if let Some(v) = storage
        .get_string(FLAGS_KEY)
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v <= AFLAGS_MASK)
    {
        m.set_aflags(v);
    }
    if let Some(v) = storage.get_string(PROMPT_KEY) {
        m.prompt_on_insert = v != "0";
    }
    m
}

pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    storage.set_string(FLAGS_KEY, m.aflags().to_string());
    storage.set_string(PROMPT_KEY, u8::from(m.prompt_on_insert).to_string());
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
    fn aflags_splits_into_modes_and_lock() {
        let mut m = Memory::default();
        m.set_aflags(2 | 16);
        assert_eq!((m.flags, m.lock_position, m.aflags()), (2, true, 18));
        m.set_aflags(0);
        assert_eq!((m.flags, m.lock_position), (0, false));
        assert!(Memory::default().lock_position, "new definitions lock");
    }

    #[test]
    fn what_was_saved_is_read_back_and_nonsense_is_ignored() {
        let mut saved = Saved::default();
        let m = Memory {
            flags: 5,
            lock_position: true,
            prompt_on_insert: false,
        };
        save(&m, &mut saved);
        assert_eq!(load(Some(&saved)), m);
        saved.set_string(FLAGS_KEY, "999".into());
        assert_eq!(load(Some(&saved)).flags, 0);
        assert_eq!(load(None), Memory::default());
    }
}
