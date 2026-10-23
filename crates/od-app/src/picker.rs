//! Asking the user which file to open or save to. A trait, so the logic in
//! `files.rs` can be tested without a dialog on screen.

use std::path::{Path, PathBuf};

pub trait FilePicker {
    /// Ask for a drawing to open; `None` if the user cancelled.
    fn open(&mut self) -> Option<PathBuf>;

    /// Ask where to save; `None` if the user cancelled. `suggested` is the
    /// file name to offer, `directory` where to start if known, and `filter`
    /// the kind of file (name and extensions) to save.
    fn save(
        &mut self,
        suggested: &str,
        directory: Option<&Path>,
        filter: Filter,
    ) -> Option<PathBuf>;

    /// Ask for a folder; `None` if the user cancelled.
    fn folder(&mut self, title: &str) -> Option<PathBuf>;
}

/// A kind of file in a dialog: its name and extensions.
pub type Filter = (&'static str, &'static [&'static str]);

/// The operating system's own file dialogs. Tests use a scripted picker
/// instead, so this is only built into the real app.
#[cfg_attr(test, allow(dead_code))]
pub struct NativePicker;

impl FilePicker for NativePicker {
    fn open(&mut self) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().set_title("Open drawing");
        for (name, extensions) in od_files::open_filters() {
            dialog = dialog.add_filter(name, &extensions);
        }
        dialog.pick_file()
    }

    fn save(
        &mut self,
        suggested: &str,
        directory: Option<&Path>,
        (name, extensions): Filter,
    ) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save drawing as")
            .set_file_name(suggested)
            .add_filter(name, extensions);
        if let Some(directory) = directory {
            dialog = dialog.set_directory(directory);
        }
        dialog.save_file()
    }

    fn folder(&mut self, title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_folder()
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
        /// The kind of file (by name) each Save dialog offered.
        pub filters: Vec<String>,
        pub open_dialogs: usize,
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

        pub fn filters(&self) -> Vec<String> {
            self.0.borrow().filters.clone()
        }

        pub fn open_dialogs(&self) -> usize {
            self.0.borrow().open_dialogs
        }
    }

    #[derive(Default)]
    pub struct ScriptedPicker(pub Script);

    impl FilePicker for ScriptedPicker {
        fn open(&mut self) -> Option<PathBuf> {
            let mut data = self.0 .0.borrow_mut();
            data.open_dialogs += 1;
            data.opens.pop_front().flatten()
        }

        fn save(
            &mut self,
            suggested: &str,
            _directory: Option<&Path>,
            filter: Filter,
        ) -> Option<PathBuf> {
            let mut data = self.0 .0.borrow_mut();
            data.suggested.push(suggested.to_owned());
            data.filters.push(filter.0.to_owned());
            data.saves.pop_front().flatten()
        }

        fn folder(&mut self, _title: &str) -> Option<PathBuf> {
            self.0 .0.borrow_mut().folders.pop_front().flatten()
        }
    }
}
