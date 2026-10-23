//! The Home tab: what the app shows when no drawing is open, or when the
//! user picks the house at the left of the tabs. With no drawing open, `docs`
//! holds one hidden placeholder so the rest of the app always has a drawing to
//! point at; the registry blocks drawing commands while Home shows (see
//! `Command::global`).

use std::path::Path;

use od_commands::Session;

use crate::recent::RecentId;
use crate::state::{AppState, Doc};

impl AppState {
    /// Whether any real drawing is open.
    pub fn has_drawings(&self) -> bool {
        self.docs.iter().any(|d| !d.placeholder)
    }

    /// Show the Home tab, keeping the open drawings as they are.
    pub fn show_home(&mut self) {
        self.abort_command();
        self.home = true;
    }

    /// Show drawing `index` (a click on its tab).
    pub fn show_document(&mut self, index: usize) {
        if index < self.docs.len() {
            self.abort_command();
            self.active = index;
            self.home = false;
        }
    }

    /// Nothing is open: leave only the hidden placeholder, and show Home.
    pub(crate) fn show_home_without_drawings(&mut self) {
        self.abort_command();
        let uid = self.next_doc_uid;
        self.next_doc_uid += 1;
        let mut placeholder = Doc::new(uid, String::new(), Session::default());
        placeholder.placeholder = true;
        self.docs = vec![placeholder];
        self.active = 0;
        self.home = true;
    }

    /// Start with no drawing open, as if none had ever been made.
    pub(crate) fn start_on_home(&mut self) {
        self.show_home_without_drawings();
        self.next_doc_number = 1;
    }
}

impl AppState {
    /// Open the recent drawing `id`. A failure leaves it on the list: the
    /// drive may only be away for now.
    pub fn open_recent_id(&mut self, id: RecentId) {
        if let Some(path) = self.recent.get(id).map(|i| i.path.clone()) {
            self.open_path(&path);
        }
    }

    /// Ask for a file to stand in for the recent drawing `id`. Nothing
    /// changes unless it opens; then it takes the entry's place.
    pub fn locate_recent(&mut self, id: RecentId) {
        if let Some(path) = self.picker.open() {
            self.locate.retain(|(_, old)| *old != id);
            self.locate.push((path.clone(), id));
            self.open_path(&path);
        }
    }

    /// Note that `path` was opened or saved successfully.
    pub(crate) fn commit_recent(&mut self, path: &Path) {
        let old = self.locate.iter().position(|(p, _)| p == path);
        let old = old.map(|i| self.locate.remove(i).1);
        self.recent.add_replacing(path, old);
    }
}

/// "12 KB", "3.4 MB".
pub fn size_text(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{:.1} MB", b / (KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_everything_leaves_a_hidden_placeholder_on_home() {
        let mut state = AppState::new();
        state.new_document();
        state.close_document(1);
        state.close_document(0);
        assert!(!state.has_drawings());
        assert!(state.home);
        assert_eq!(state.docs.len(), 1);
        assert!(state.docs[0].placeholder);
    }

    #[test]
    fn a_new_drawing_replaces_the_placeholder_and_leaves_home() {
        let mut state = AppState::new();
        state.start_on_home();
        state.new_document();
        assert!(state.has_drawings());
        assert!(!state.home);
        assert_eq!(state.docs.len(), 1);
        assert_eq!(
            state.docs[0].title, "Drawing1",
            "starting on Home spent no number"
        );
    }

    #[test]
    fn home_and_drawing_tabs_swap_without_losing_anything() {
        let mut state = AppState::new();
        state.new_document();
        state.show_home();
        assert!(state.home);
        assert_eq!(state.docs.len(), 2);
        state.show_document(0);
        assert!(!state.home);
        assert_eq!(state.active, 0);
    }

    #[test]
    fn drawing_commands_wait_while_home_shows() {
        let registry = crate::commands::build_registry();
        let mut state = AppState::new();
        assert!(registry.is_enabled("draw.line", &state));
        state.show_home();
        assert!(!registry.is_enabled("draw.line", &state));
        assert!(registry.is_enabled("file.new", &state));
        assert!(registry.is_enabled("file.open", &state));
        assert!(registry.is_enabled("manage.settings", &state));
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(size_text(900), "900 B");
        assert_eq!(size_text(2048), "2 KB");
        assert_eq!(size_text(3 * 1024 * 1024 + 400 * 1024), "3.4 MB");
    }
}
