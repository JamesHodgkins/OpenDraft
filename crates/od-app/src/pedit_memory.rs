//! What PEDIT remembers between uses, shared by the command, the system
//! variable and the saved settings: whether it turns lines and arcs into
//! polylines without asking (`PEDITACCEPT`), and the Join type.

use od_core::pedit::JoinType;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    /// Turn lines, arcs and rectangles into polylines without asking
    /// (`PEDITACCEPT` 1).
    pub accept: bool,
    /// What Multiple's Join does with ends that nearly meet.
    pub join_type: JoinType,
}

const DEFAULT: Memory = Memory {
    accept: false,
    join_type: JoinType::Extend,
};

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

const KEYS: [&str; 2] = ["pedit_accept", "pedit_join_type"];

fn join_name(kind: JoinType) -> &'static str {
    match kind {
        JoinType::Extend => "Extend",
        JoinType::Add => "Add",
        JoinType::Both => "Both",
    }
}

/// Take the saved settings, keeping the default for anything missing or not
/// a sensible value.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let mut m = DEFAULT;
    let Some(storage) = storage else { return m };
    let accept = storage
        .get_string(KEYS[0])
        .and_then(|v| v.parse::<bool>().ok());
    let kind = storage.get_string(KEYS[1]).and_then(|v| match v.as_str() {
        "Extend" => Some(JoinType::Extend),
        "Add" => Some(JoinType::Add),
        "Both" => Some(JoinType::Both),
        _ => None,
    });
    m.accept = accept.unwrap_or(m.accept);
    m.join_type = kind.unwrap_or(m.join_type);
    m
}

/// Write the settings for the next run.
pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    storage.set_string(KEYS[0], m.accept.to_string());
    storage.set_string(KEYS[1], join_name(m.join_type).to_owned());
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
        let kept = Memory {
            accept: true,
            join_type: JoinType::Both,
        };
        let mut saved = Saved::default();
        save(&kept, &mut saved);
        assert_eq!(load(Some(&saved)), kept);
        saved.set_string(KEYS[1], "Sideways".to_owned());
        saved.set_string(KEYS[0], "maybe".to_owned());
        assert_eq!(load(Some(&saved)), Memory::default());
    }
}
