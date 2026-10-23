//! The list of recently opened or saved drawings, newest first. Kept apart
//! from `Settings` because it changes as the user works, not in the dialog.

use std::path::{Path, PathBuf};

const KEY: &str = "recent_files";
/// How many drawings are remembered.
pub const MAX_RECENT: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentFiles {
    paths: Vec<PathBuf>,
}

impl RecentFiles {
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Put `path` first, dropping an earlier entry for the same file and the
    /// oldest ones beyond the limit.
    pub fn add(&mut self, path: &Path) {
        self.remove(path);
        self.paths.insert(0, path.to_owned());
        self.paths.truncate(MAX_RECENT);
    }

    /// Forget every entry.
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Forget `path` (it could not be opened).
    pub fn remove(&mut self, path: &Path) {
        self.paths.retain(|p| p != path);
    }

    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let text = storage.and_then(|s| s.get_string(KEY)).unwrap_or_default();
        Self {
            paths: text
                .lines()
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .take(MAX_RECENT)
                .collect(),
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        let lines: Vec<_> = self.paths.iter().map(|p| p.to_string_lossy()).collect();
        storage.set_string(KEY, lines.join("\n"));
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

    fn p(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn newest_first_without_duplicates() {
        let mut recent = RecentFiles::default();
        recent.add(&p("a.odx"));
        recent.add(&p("b.odx"));
        recent.add(&p("a.odx"));
        assert_eq!(recent.paths(), [p("a.odx"), p("b.odx")]);
    }

    #[test]
    fn the_oldest_fall_off_past_the_limit() {
        let mut recent = RecentFiles::default();
        for i in 0..MAX_RECENT + 3 {
            recent.add(&p(&format!("{i}.odx")));
        }
        assert_eq!(recent.paths().len(), MAX_RECENT);
        assert_eq!(recent.paths()[0], p(&format!("{}.odx", MAX_RECENT + 2)));
        assert!(!recent.paths().contains(&p("0.odx")));
    }

    #[test]
    fn removing_forgets_a_file() {
        let mut recent = RecentFiles::default();
        recent.add(&p("a.odx"));
        recent.remove(&p("a.odx"));
        recent.remove(&p("never.odx"));
        assert!(recent.paths().is_empty());
    }

    #[test]
    fn saved_and_loaded_lists_match() {
        let mut recent = RecentFiles::default();
        recent.add(&p("dir with spaces/a.odx"));
        recent.add(&p("b.odx"));
        let mut storage = MemoryStorage::default();
        recent.save(&mut storage);
        assert_eq!(RecentFiles::load(Some(&storage)), recent);
        assert_eq!(RecentFiles::load(None), RecentFiles::default());
    }
}
