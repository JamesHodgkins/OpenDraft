//! Asking the user which file to open or save to. A trait, so the logic in
//! `files.rs` can be tested without a dialog on screen.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub trait FilePicker {
    /// Ask for a drawing to open, starting in `directory` if known; `None`
    /// if the user cancelled.
    fn open(&mut self, directory: Option<&Path>) -> Option<PathBuf>;

    /// Ask where to save; `None` if the user cancelled. `title` is the
    /// dialog's, `suggested` the file name to offer, `directory` where to start if known, and `filter`
    /// the kind of file (name and extensions) to save.
    fn save(
        &mut self,
        title: &str,
        suggested: &str,
        directory: Option<&Path>,
        filter: Filter,
    ) -> Option<PathBuf>;

    /// Ask for a folder; `None` if the user cancelled.
    fn folder(&mut self, title: &str) -> Option<PathBuf>;

    /// Something the user should be told about the dialog just asked for
    /// (that none could be shown), once. Most pickers have nothing to say.
    fn take_problem(&mut self) -> Option<String> {
        None
    }
}

/// A kind of file in a dialog: its name and extensions.
pub type Filter = (&'static str, &'static [&'static str]);

/// A dialog that answers "cancelled" sooner than this was never shown: nobody
/// can look at a dialog and dismiss it in under a third of a second.
const SHOWN_AT_LEAST: Duration = Duration::from_millis(300);

/// What to tell a Linux user whose desktop has nothing to show a file dialog
/// with. The dialogs come from the desktop portal, or `zenity` when there is
/// none; with neither, asking for a file does nothing, and this says why.
const NO_DIALOG: &str = "No file dialog could be shown. On Linux a dialog needs xdg-desktop-portal with a backend (xdg-desktop-portal-gtk or -kde) or zenity. Until one is installed, start OpenDraft with the file's name: opendraft drawing.dxf";

/// Whether a dialog that was answered with `None` after `elapsed` was in fact
/// never shown.
fn never_appeared(elapsed: Duration) -> bool {
    elapsed < SHOWN_AT_LEAST
}

/// The operating system's own file dialogs. Tests use a scripted picker
/// instead, so this is only built into the real app.
#[cfg_attr(test, allow(dead_code))]
#[derive(Default)]
pub struct NativePicker {
    problem: Option<String>,
}

#[cfg_attr(test, allow(dead_code))]
impl NativePicker {
    /// Show a dialog with `ask`. On Linux, one that comes back at once with
    /// no answer could not be shown, which is worth saying (see [`NO_DIALOG`]).
    fn shown<T>(&mut self, ask: impl FnOnce() -> Option<T>) -> Option<T> {
        let started = Instant::now();
        let answer = ask();
        if answer.is_none() && cfg!(target_os = "linux") && never_appeared(started.elapsed()) {
            self.problem = Some(NO_DIALOG.to_owned());
        }
        answer
    }
}

impl FilePicker for NativePicker {
    fn open(&mut self, directory: Option<&Path>) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().set_title("Open drawing");
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        for (name, extensions) in od_files::open_filters() {
            dialog = dialog.add_filter(name, &extensions);
        }
        self.shown(|| dialog.pick_file())
    }

    fn take_problem(&mut self) -> Option<String> {
        self.problem.take()
    }

    fn save(
        &mut self,
        title: &str,
        suggested: &str,
        directory: Option<&Path>,
        (name, extensions): Filter,
    ) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(suggested)
            .add_filter(name, extensions);
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        self.shown(|| dialog.save_file())
    }

    fn folder(&mut self, title: &str) -> Option<PathBuf> {
        self.shown(|| rfd::FileDialog::new().set_title(title).pick_folder())
    }
}

impl crate::state::AppState {
    /// Tell the user what the picker said about its last dialog, if it said
    /// anything (once a frame is enough).
    pub fn report_picker_problem(&mut self) {
        if let Some(problem) = self.picker.take_problem() {
            self.message = problem;
        }
    }
}

#[cfg(test)]
pub use scripted::{Script, ScriptedPicker};

#[cfg(test)]
mod scripted {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use super::{FilePicker, Filter};

    /// What a test tells the picker to answer, and what it was asked.
    #[derive(Default)]
    pub struct ScriptData {
        pub opens: VecDeque<Option<PathBuf>>,
        pub saves: VecDeque<Option<PathBuf>>,
        /// The suggested name of each Save dialog shown.
        pub suggested: Vec<String>,
        /// The title and starting folder each Save dialog was given.
        pub titles: Vec<String>,
        pub directories: Vec<Option<PathBuf>>,
        /// The kind of file (by name) each Save dialog offered.
        pub filters: Vec<String>,
        pub open_dialogs: usize,
        /// The starting folder each Open dialog was given.
        pub open_directories: Vec<Option<PathBuf>>,
        pub folders: VecDeque<Option<PathBuf>>,
    }

    /// A handle a test keeps to steer a [`ScriptedPicker`]. An empty queue
    /// means the user cancelled.
    #[derive(Clone, Default)]
    pub struct Script(pub Rc<RefCell<ScriptData>>);

    impl Script {
        pub fn will_open(&self, path: Option<PathBuf>) {
            self.0.borrow_mut().opens.push_back(path);
        }

        pub fn will_save_to(&self, path: Option<PathBuf>) {
            self.0.borrow_mut().saves.push_back(path);
        }

        pub fn will_pick_folder(&self, path: Option<PathBuf>) {
            self.0.borrow_mut().folders.push_back(path);
        }

        pub fn suggested(&self) -> Vec<String> {
            self.0.borrow().suggested.clone()
        }

        pub fn titles(&self) -> Vec<String> {
            self.0.borrow().titles.clone()
        }

        pub fn directories(&self) -> Vec<Option<PathBuf>> {
            self.0.borrow().directories.clone()
        }

        pub fn filters(&self) -> Vec<String> {
            self.0.borrow().filters.clone()
        }

        pub fn open_directories(&self) -> Vec<Option<PathBuf>> {
            self.0.borrow().open_directories.clone()
        }

        pub fn open_dialogs(&self) -> usize {
            self.0.borrow().open_dialogs
        }
    }

    #[derive(Default)]
    pub struct ScriptedPicker(pub Script);

    impl FilePicker for ScriptedPicker {
        fn open(&mut self, directory: Option<&Path>) -> Option<PathBuf> {
            let mut data = self.0 .0.borrow_mut();
            data.open_dialogs += 1;
            data.open_directories.push(directory.map(Path::to_owned));
            data.opens.pop_front().flatten()
        }

        fn save(
            &mut self,
            title: &str,
            suggested: &str,
            directory: Option<&Path>,
            filter: Filter,
        ) -> Option<PathBuf> {
            let mut data = self.0 .0.borrow_mut();
            data.titles.push(title.to_owned());
            data.directories.push(directory.map(Path::to_owned));
            data.suggested.push(suggested.to_owned());
            data.filters.push(filter.0.to_owned());
            data.saves.pop_front().flatten()
        }

        fn folder(&mut self, _title: &str) -> Option<PathBuf> {
            self.0 .0.borrow_mut().folders.pop_front().flatten()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    /// A picker that answers nothing and has a problem to report once.
    struct Troubled(Option<String>);

    impl FilePicker for Troubled {
        fn open(&mut self, _: Option<&Path>) -> Option<PathBuf> {
            None
        }
        fn save(&mut self, _: &str, _: &str, _: Option<&Path>, _: Filter) -> Option<PathBuf> {
            None
        }
        fn folder(&mut self, _: &str) -> Option<PathBuf> {
            None
        }
        fn take_problem(&mut self) -> Option<String> {
            self.0.take()
        }
    }

    #[test]
    fn an_answer_in_a_blink_means_no_dialog_was_shown() {
        assert!(never_appeared(Duration::ZERO));
        assert!(never_appeared(Duration::from_millis(299)));
        assert!(!never_appeared(Duration::from_millis(300)));
        assert!(!never_appeared(Duration::from_secs(4)));
    }

    #[test]
    fn the_help_names_the_things_to_install_and_a_way_round() {
        for word in ["xdg-desktop-portal", "zenity", "opendraft drawing.dxf"] {
            assert!(NO_DIALOG.contains(word), "{word}");
        }
    }

    #[test]
    fn a_problem_the_picker_reports_reaches_the_status_bar_once() {
        let mut state = AppState::new();
        state.picker = Box::new(Troubled(Some("no dialog".to_owned())));
        state.message.clear();
        state.report_picker_problem();
        assert_eq!(state.message, "no dialog");
        state.message.clear();
        state.report_picker_problem();
        assert!(state.message.is_empty(), "said once");
    }

    #[test]
    fn a_picker_with_nothing_to_say_leaves_the_message_alone() {
        let mut state = AppState::new();
        state.message = "Saved".to_owned();
        state.report_picker_problem();
        assert_eq!(state.message, "Saved");
    }
}
