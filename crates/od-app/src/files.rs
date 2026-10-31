//! Opening, saving and closing drawings, and the questions about unsaved
//! changes. Pure logic over `AppState`; the dialog is drawn in
//! `app/file_dialogs.rs` and the file formats live in `od-files`.

use std::path::{Path, PathBuf};

use od_commands::Session;

use crate::picker::FilePicker;
use crate::state::AppState;

/// The picker a new `AppState` starts with: the real dialogs, or in tests one
/// that always says the user cancelled.
pub fn default_picker() -> Box<dyn FilePicker> {
    #[cfg(test)]
    {
        Box::<crate::picker::ScriptedPicker>::default()
    }
    #[cfg(not(test))]
    {
        Box::<crate::picker::NativePicker>::default()
    }
}

/// The file worker a new `AppState` starts with: a thread of its own, or in
/// tests one that runs each job on the spot.
pub fn default_io() -> crate::io::Io {
    #[cfg(test)]
    {
        crate::io::Io::inline()
    }
    #[cfg(not(test))]
    {
        crate::io::Io::background()
    }
}

/// What happens once every drawing in a [`Confirm`] has been dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    /// Close that tab.
    Close,
    /// Quit the app.
    Exit,
}

/// The user's answer to "save changes to X?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Save,
    /// Throw the changes away.
    Discard,
    Cancel,
}

/// Drawings with unsaved changes the user is being asked about, one at a
/// time, front first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirm {
    /// Drawing uids (not indexes: tabs can move while the question is up).
    pending: Vec<u64>,
    after: After,
    /// The drawing whose save the question is waiting for.
    saving: Option<u64>,
}

impl Confirm {
    pub fn after(&self) -> After {
        self.after
    }

    /// The drawing being saved at the user's request, if any.
    pub fn saving(&self) -> Option<u64> {
        self.saving
    }

    pub(crate) fn set_saving(&mut self, uid: Option<u64>) {
        self.saving = uid;
    }
}

/// The name shown for a file: its own name without the folder or the
/// extension.
pub(crate) fn display_name(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.display().to_string(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// Whether two paths name the same file (compared as the system sees them,
/// so `./a.odx` and `a.odx` match).
pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

impl AppState {
    pub(crate) fn tell(&mut self, text: String) {
        self.cmdline.say(text.clone());
        self.message = text;
    }

    /// Where an Open, Save or Export dialog starts: the drawing's own
    /// folder, else where the last drawing was opened or saved.
    pub(crate) fn folder_for_dialogs(&self, path: Option<&Path>) -> Option<PathBuf> {
        path.and_then(Path::parent)
            .map(Path::to_owned)
            .or_else(|| self.recent.last_folder())
    }

    // ---- open ----

    /// Ask which drawing to open, and open it.
    pub fn open_dialog(&mut self) {
        let directory = self.recent.last_folder();
        if let Some(path) = self.picker.open(directory.as_deref()) {
            self.open_path(&path);
        }
    }

    /// Open the `index`th recent file.
    pub fn open_recent(&mut self, index: usize) {
        if let Some(path) = self.recent.items().get(index).map(|i| i.path.clone()) {
            self.open_path(&path);
        }
    }

    /// Open a drawing in a tab (or switch to it if it is already open). The
    /// file is read in the background and appears when it is ready; a failure
    /// is reported in the status bar then. Returns whether the open started.
    pub fn open_path(&mut self, path: &Path) -> bool {
        if let Some(i) = self
            .docs
            .iter()
            .position(|d| d.path.as_deref().is_some_and(|p| same_file(p, path)))
        {
            self.active = i;
            self.home = false;
            self.commit_recent(path);
            self.message = format!("{} is already open", self.docs[i].title);
            return true;
        }
        self.begin_open(path);
        true
    }

    /// Show `session` in a tab and make it active. A blank, untouched
    /// drawing makes way for it; otherwise it gets a new tab.
    pub(crate) fn install_document(
        &mut self,
        title: String,
        session: Session,
    ) -> &mut crate::state::Doc {
        let blank = {
            let doc = &self.docs[self.active];
            doc.path.is_none()
                && !doc.is_modified()
                && doc.block_edit.is_none()
                && doc.session.document().is_empty()
        };
        if blank {
            let uid = self.docs[self.active].uid;
            let active = self.active;
            self.docs[active] = crate::state::Doc::new(uid, title, session);
            self.home = false;
            &mut self.docs[active]
        } else {
            self.push_document(title, session)
        }
    }

    // ---- save ----

    /// Save the active drawing (asking for a name if it has none).
    pub fn save_active(&mut self) -> bool {
        self.save_doc(self.active)
    }

    pub fn save_active_as(&mut self) -> bool {
        self.save_doc_as(self.active)
    }

    /// Save drawing `index` to its file, or ask where if it has none.
    pub fn save_doc(&mut self, index: usize) -> bool {
        match self.docs.get(index).map(|d| d.path.clone()) {
            Some(Some(path)) => self.write_doc(index, &path),
            Some(None) => self.save_doc_as(index),
            None => false,
        }
    }

    /// Ask where to save drawing `index`, and save it there. Returns false
    /// if the user cancelled or the save failed.
    pub fn save_doc_as(&mut self, index: usize) -> bool {
        let Some(doc) = self.docs.get(index) else {
            return false;
        };
        // A recovered drawing has no file of its own: its original is only
        // where to start, so saving can never silently replace it.
        let (title, suggested, directory) = match &doc.recovery_origin {
            Some(origin) => (
                "Save recovered drawing as",
                origin.file_name().map_or_else(
                    || format!("{}.{}", doc.title, od_files::NATIVE_EXTENSION),
                    |n| n.to_string_lossy().into_owned(),
                ),
                origin.parent().map(Path::to_owned),
            ),
            None => (
                "Save drawing as",
                format!("{}.{}", doc.title, od_files::NATIVE_EXTENSION),
                self.folder_for_dialogs(doc.path.as_deref()),
            ),
        };
        let Some(chosen) = self.picker.save(
            title,
            &suggested,
            directory.as_deref(),
            od_files::native_filter(),
        ) else {
            return false;
        };
        let path = od_files::native_path(&chosen);
        self.write_doc(index, &path)
    }

    fn write_doc(&mut self, index: usize, path: &Path) -> bool {
        self.begin_save(index, path)
    }

    /// Write a copy of the active drawing as DXF, for other CAD programs.
    /// The drawing itself stays as it is: still unsaved if it was, and still
    /// tied to its own file.
    pub fn export_dxf(&mut self) -> bool {
        let doc = &self.docs[self.active];
        let suggested = format!("{}.dxf", doc.title);
        let directory = self.folder_for_dialogs(doc.path.as_deref());
        let Some(chosen) = self.picker.save(
            "Export DXF",
            &suggested,
            directory.as_deref(),
            od_files::dxf_filter(),
        ) else {
            return false;
        };
        let path = if od_files::format_for(&chosen).is_some_and(|f| f.extensions() == ["dxf"]) {
            chosen
        } else {
            chosen.with_extension("dxf")
        };
        self.begin_export(&path);
        true
    }

    // ---- close and exit ----

    /// Close a drawing, first asking what to do with unsaved changes.
    pub fn request_close(&mut self, index: usize) {
        if self.docs.get(index).is_none_or(|d| d.placeholder) {
            return;
        }
        if !self.leave_block_editor(index) {
            return;
        }
        let doc = &self.docs[index];
        if doc.is_modified() {
            self.confirm = Some(Confirm {
                pending: vec![doc.uid],
                after: After::Close,
                saving: None,
            });
        } else {
            self.close_document(index);
        }
    }

    /// The window is closing or Exit was chosen. Returns true when nothing
    /// is unsaved, so the app may quit now; otherwise asks about each
    /// modified drawing and returns false (`exit_ok` is set when done).
    pub fn try_exit(&mut self) -> bool {
        if !(0..self.docs.len()).all(|i| self.leave_block_editor(i)) {
            return false;
        }
        let pending: Vec<u64> = self
            .docs
            .iter()
            .filter(|d| d.is_modified())
            .map(|d| d.uid)
            .collect();
        if pending.is_empty() {
            self.exit_ok = true;
            return true;
        }
        self.confirm = Some(Confirm {
            pending,
            after: After::Exit,
            saving: None,
        });
        false
    }

    /// The drawing the open question is about.
    pub fn confirm_subject(&self) -> Option<&crate::state::Doc> {
        let uid = *self.confirm.as_ref()?.pending.first()?;
        self.docs.iter().find(|d| d.uid == uid)
    }

    /// Apply the user's answer to the open question.
    pub fn answer_confirm(&mut self, choice: Choice) {
        let Some(confirm) = self.confirm.as_ref() else {
            return;
        };
        let Some(&uid) = confirm.pending.first() else {
            self.confirm = None;
            return;
        };
        let index = self.docs.iter().position(|d| d.uid == uid);
        match (choice, index) {
            (Choice::Cancel, _) => self.confirm = None,
            // A save that is cancelled or fails keeps the question open.
            // The question stays up, showing "Saving...", until the file is
            // written (`finish_save` moves on).
            (Choice::Save, Some(i)) => {
                if confirm.saving().is_some() {
                    return; // already waiting for a save
                }
                if let Some(confirm) = self.confirm.as_mut() {
                    confirm.set_saving(Some(uid));
                }
                if !self.save_doc(i) {
                    if let Some(confirm) = self.confirm.as_mut() {
                        confirm.set_saving(None);
                    }
                }
            }
            // The drawing is gone already (or the user discards it).
            _ => self.next_pending(),
        }
    }

    /// The front drawing is settled: ask about the next, or finish.
    pub(crate) fn next_pending(&mut self) {
        let Some(confirm) = self.confirm.as_mut() else {
            return;
        };
        let settled = confirm.pending.remove(0);
        if !confirm.pending.is_empty() {
            return;
        }
        let after = confirm.after;
        self.confirm = None;
        match after {
            After::Close => {
                if let Some(i) = self.docs.iter().position(|d| d.uid == settled) {
                    self.close_document(i);
                }
            }
            After::Exit => self.exit_ok = true,
        }
    }

    /// Paths from the command line or a drop onto the window. Each is opened;
    /// the first failure message stays in the status bar.
    pub fn open_paths(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.open_path(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;
    use crate::picker::{Script, ScriptedPicker};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("od-app-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn state() -> (AppState, Script) {
        let mut state = AppState::new();
        let script = Script::default();
        state.picker = Box::new(ScriptedPicker(script.clone()));
        (state, script)
    }

    fn draw(state: &mut AppState) {
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 5.0),
        });
    }

    #[test]
    fn dialogs_start_where_the_last_drawing_was_saved_until_a_drawing_has_its_own() {
        let dir = Scratch::new("last-folder");
        let (mut state, script) = state();
        assert!(!state.save_active());
        assert!(!state.export_dxf());
        state.open_dialog();
        assert_eq!(script.directories(), [None, None]);
        assert_eq!(script.open_directories(), [None]);

        draw(&mut state);
        script.will_save_to(Some(dir.path("plan")));
        state.save_active();
        state.open_dialog();
        assert_eq!(script.open_directories()[1], Some(dir.0.clone()));

        state.new_document();
        state.export_dxf();
        assert_eq!(
            script.directories()[3],
            Some(dir.0.clone()),
            "a new drawing starts where the last one went"
        );
    }

    #[test]
    fn saving_a_new_drawing_asks_where_and_adds_the_extension() {
        let dir = Scratch::new("saveas");
        let (mut state, script) = state();
        draw(&mut state);
        assert!(state.docs[0].is_modified());
        script.will_save_to(Some(dir.path("plan")));

        assert!(state.save_active());
        assert_eq!(script.suggested(), ["Drawing1.odx"]);
        let doc = &state.docs[0];
        assert_eq!(doc.path, Some(dir.path("plan.odx")));
        assert_eq!(doc.title, "plan");
        assert!(!doc.is_modified());
        assert_eq!(
            od_files::load(&dir.path("plan.odx"))
                .unwrap()
                .document
                .len(),
            1
        );
        assert_eq!(state.recent.paths(), [dir.path("plan.odx")]);
        assert_eq!(state.message, "Saved plan");
    }

    #[test]
    fn saving_again_goes_to_the_same_file_without_asking() {
        let dir = Scratch::new("resave");
        let (mut state, script) = state();
        draw(&mut state);
        script.will_save_to(Some(dir.path("plan.odx")));
        state.save_active();
        draw(&mut state);
        assert!(state.docs[0].is_modified());

        assert!(state.save_active());
        assert_eq!(script.suggested().len(), 1, "no second dialog");
        assert_eq!(
            od_files::load(&dir.path("plan.odx"))
                .unwrap()
                .document
                .len(),
            2
        );
        assert!(!state.docs[0].is_modified());
    }

    #[test]
    fn save_as_asks_even_when_there_is_a_file() {
        let dir = Scratch::new("saveas2");
        let (mut state, script) = state();
        draw(&mut state);
        script.will_save_to(Some(dir.path("a.odx")));
        state.save_active();
        script.will_save_to(Some(dir.path("b.odx")));
        assert!(state.save_active_as());
        assert_eq!(state.docs[0].path, Some(dir.path("b.odx")));
        assert_eq!(state.docs[0].title, "b");
        assert!(dir.path("a.odx").exists() && dir.path("b.odx").exists());
    }

    #[test]
    fn cancelling_the_save_dialog_saves_nothing() {
        let (mut state, _script) = state();
        draw(&mut state);
        assert!(!state.save_active());
        assert!(state.docs[0].is_modified());
        assert!(state.docs[0].path.is_none());
    }

    #[test]
    fn a_failed_save_says_why_and_stays_modified() {
        let dir = Scratch::new("failsave");
        let (mut state, script) = state();
        draw(&mut state);
        script.will_save_to(Some(dir.path("missing folder/plan.odx")));
        state.save_active();
        assert!(state.message.starts_with("Could not save"));
        assert!(state.docs[0].is_modified());
        assert!(state.docs[0].path.is_none());
    }

    #[test]
    fn opening_a_file_that_is_already_open_switches_to_it() {
        let dir = Scratch::new("open");
        let (mut state, script) = state();
        draw(&mut state);
        script.will_save_to(Some(dir.path("plan.odx")));
        state.save_active();
        state.new_document();
        state.new_document();
        state.close_document(1);
        assert_eq!(state.docs.len(), 2);
        state.active = 1;

        script.will_open(Some(dir.path("plan.odx")));
        // The saved drawing is already open: switch to it.
        state.open_dialog();
        assert_eq!(state.docs.len(), 2);
        assert_eq!(state.active, 0);
        assert!(state.message.contains("already open"));
    }

    #[test]
    fn opening_into_a_blank_drawing_replaces_it() {
        let dir = Scratch::new("openblank");
        let mut document = od_core::Document::default();
        document.add(Entity::Circle {
            center: Point::new(1.0, 1.0),
            radius: 2.0,
        });
        od_files::save(&dir.path("circle.odx"), &document).unwrap();

        let (mut state, script) = state();
        script.will_open(Some(dir.path("circle.odx")));
        state.open_dialog();
        assert_eq!(state.docs.len(), 1, "the blank Drawing1 made way");
        let doc = &state.docs[0];
        assert_eq!(doc.title, "circle");
        assert_eq!(doc.path, Some(dir.path("circle.odx")));
        assert_eq!(*doc.session.document(), document);
        assert!(!doc.is_modified());
        assert!(!doc.session.history().can_undo());
        assert_eq!(state.message, "Opened circle");
        assert_eq!(state.recent.paths(), [dir.path("circle.odx")]);
    }

    #[test]
    fn a_dxf_opens_as_a_new_unsaved_drawing() {
        let dir = Scratch::new("dxfopen");
        let mut document = od_core::Document::default();
        document.add(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(3.0, 4.0),
        });
        // An arc cannot be kept as one, so opening warns.
        let mut text = String::from_utf8(
            od_files::format_for(Path::new("x.dxf"))
                .unwrap()
                .write(&document)
                .unwrap()
                .bytes,
        )
        .unwrap();
        text = text.replace(
            "  0\r\nEOF\r\n",
            "  0\r\nSECTION\r\n  2\r\nENTITIES\r\n  0\r\nTEXT\r\n  8\r\n0\r\n  0\r\nENDSEC\r\n  0\r\nEOF\r\n",
        );
        std::fs::write(dir.path("plan.dxf"), text).unwrap();

        let (mut state, _) = state();
        assert!(state.open_path(&dir.path("plan.dxf")));
        assert_eq!(state.docs.len(), 1, "the blank drawing made way");
        let doc = &state.docs[0];
        assert_eq!(doc.title, "plan");
        assert!(doc.path.is_none(), "it has no file of its own");
        assert!(doc.is_modified(), "it must be saved before it is safe");
        assert_eq!(doc.session.document().len(), 1);
        assert!(state.message.contains("Imported"));
        assert!(
            state
                .cmdline
                .log
                .iter()
                .any(|l| l.contains("Skipped 1 unsupported object: 1 TEXT")),
            "{:?}",
            state.cmdline.log
        );
        assert_eq!(state.recent.paths(), [dir.path("plan.dxf")]);
    }

    #[test]
    fn a_damaged_dwg_installs_nothing_and_stays_listed() {
        let dir = Scratch::new("dwgbad");
        // A DWG header that stops short: the version tag, then nothing usable.
        let mut bytes = b"AC1032".to_vec();
        bytes.resize(2048, 0);
        std::fs::write(dir.path("half.dwg"), bytes).unwrap();
        let (mut state, _) = state();
        state.recent.add(&dir.path("half.dwg"));
        state.open_path(&dir.path("half.dwg"));
        assert!(
            state.message.starts_with("Could not open"),
            "{}",
            state.message
        );
        assert_eq!(
            state.recent.paths(),
            [dir.path("half.dwg")],
            "a failed open keeps the entry"
        );
        assert_eq!(state.docs.len(), 1);
        assert!(!state.docs[0].is_modified(), "still the blank drawing");
    }

    #[test]
    fn saving_an_imported_drawing_asks_for_a_native_name() {
        let dir = Scratch::new("dxfsave");
        let document = {
            let mut d = od_core::Document::default();
            d.add(Entity::Circle {
                center: Point::new(0.0, 0.0),
                radius: 1.0,
            });
            d
        };
        od_files::save(&dir.path("plan.dxf"), &document).unwrap();
        let (mut state, script) = state();
        state.open_path(&dir.path("plan.dxf"));
        // Even if the user types a DXF name, the drawing is saved natively.
        script.will_save_to(Some(dir.path("plan.dxf")));
        assert!(state.save_active());
        assert_eq!(script.suggested(), ["plan.odx"]);
        assert_eq!(script.filters(), ["OpenDraft drawing"]);
        assert_eq!(state.docs[0].path, Some(dir.path("plan.odx")));
        assert!(!state.docs[0].is_modified());
        assert!(
            od_files::load(&dir.path("plan.odx"))
                .unwrap()
                .document
                .len()
                == 1
        );
    }

    #[test]
    fn exporting_dxf_leaves_the_drawing_untouched() {
        let dir = Scratch::new("export");
        let (mut state, script) = state();
        draw(&mut state);
        script.will_save_to(Some(dir.path("out")));
        assert!(state.export_dxf());
        assert_eq!(script.suggested(), ["Drawing1.dxf"]);
        assert_eq!(script.filters(), ["AutoCAD DXF"]);
        assert!(dir.path("out.dxf").exists(), "the extension was added");
        let doc = &state.docs[0];
        assert!(doc.is_modified(), "an export is not a save");
        assert!(doc.path.is_none());
        assert_eq!(doc.title, "Drawing1");
        assert!(state.message.starts_with("Exported"));
        assert_eq!(
            od_files::load(&dir.path("out.dxf")).unwrap().document.len(),
            1
        );
    }

    #[test]
    fn export_says_what_the_format_could_not_keep() {
        let dir = Scratch::new("exportnotes");
        let (mut state, script) = state();
        state.add_entity(Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(2.0, 1.0),
        });
        script.will_save_to(Some(dir.path("r.dxf")));
        state.export_dxf();
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("Rectangles were written")));
    }

    #[test]
    fn a_cancelled_or_failed_export_changes_nothing() {
        let dir = Scratch::new("exportfail");
        let (mut state, script) = state();
        draw(&mut state);
        assert!(!state.export_dxf(), "cancelled");
        script.will_save_to(Some(dir.path("no such folder/x.dxf")));
        state.export_dxf();
        assert!(state.message.starts_with("Could not export"));
        assert!(state.docs[0].is_modified());
    }

    #[test]
    fn opening_next_to_real_work_adds_a_tab() {
        let dir = Scratch::new("opentab");
        od_files::save(&dir.path("empty.odx"), &od_core::Document::default()).unwrap();
        let (mut state, _) = state();
        draw(&mut state);
        assert!(state.open_path(&dir.path("empty.odx")));
        assert_eq!(state.docs.len(), 2);
        assert_eq!(state.active, 1);
        assert!(
            state.docs[0].is_modified(),
            "the first drawing is untouched"
        );
    }

    #[test]
    fn a_bad_file_is_reported_and_stays_listed() {
        let dir = Scratch::new("badfile");
        std::fs::write(dir.path("junk.odx"), "not a drawing").unwrap();
        let (mut state, _) = state();
        state.recent.add(&dir.path("junk.odx"));
        state.open_path(&dir.path("junk.odx"));
        assert!(state.message.starts_with("Could not open"));
        assert_eq!(state.recent.paths(), [dir.path("junk.odx")]);
        assert_eq!(state.docs.len(), 1);
        state.open_path(&dir.path("gone.odx"));
        assert!(state.message.starts_with("Could not open"));
        state.open_path(&dir.path("plan.txt"));
        assert!(state.message.starts_with("Could not open"));
    }

    #[test]
    fn locating_a_missing_recent_drawing_replaces_it_once_the_file_opens() {
        let dir = Scratch::new("locate");
        od_files::save(&dir.path("moved.odx"), &od_core::Document::default()).unwrap();
        let (mut state, script) = state();
        state.recent.add(&dir.path("gone.odx"));
        let gone = state.recent.items()[0].id;
        state.recent.pin(gone).unwrap();
        script.will_open(Some(dir.path("moved.odx")));
        state.locate_recent(gone);
        assert_eq!(state.recent.paths(), [dir.path("moved.odx")]);
        assert!(state.recent.items()[0].pinned, "the pin moved with it");
        assert!(state.locate.is_empty());
    }

    #[test]
    fn locating_a_file_that_will_not_open_changes_nothing() {
        let dir = Scratch::new("locatebad");
        std::fs::write(dir.path("junk.odx"), "not a drawing").unwrap();
        let (mut state, script) = state();
        state.recent.add(&dir.path("gone.odx"));
        let gone = state.recent.items()[0].id;
        script.will_open(Some(dir.path("junk.odx")));
        state.locate_recent(gone);
        assert_eq!(state.recent.paths(), [dir.path("gone.odx")]);
        assert!(state.locate.is_empty());
    }

    #[test]
    fn a_cancelled_open_dialog_does_nothing() {
        let (mut state, script) = state();
        state.open_dialog();
        assert_eq!(script.open_dialogs(), 1);
        assert_eq!(state.docs.len(), 1);
    }

    #[test]
    fn closing_a_clean_drawing_just_closes() {
        let (mut state, _) = state();
        state.new_document();
        state.request_close(1);
        assert_eq!(state.docs.len(), 1);
        assert!(state.confirm.is_none());
    }

    #[test]
    fn closing_a_modified_drawing_asks_first() {
        let (mut state, _) = state();
        state.new_document();
        draw(&mut state);
        state.request_close(1);
        assert_eq!(state.docs.len(), 2, "not closed yet");
        assert_eq!(state.confirm_subject().unwrap().title, "Drawing2");

        state.answer_confirm(Choice::Cancel);
        assert!(state.confirm.is_none());
        assert_eq!(state.docs.len(), 2);

        state.request_close(1);
        state.answer_confirm(Choice::Discard);
        assert_eq!(state.docs.len(), 1);
        assert!(state.confirm.is_none());
    }

    #[test]
    fn saving_from_the_question_saves_then_closes() {
        let dir = Scratch::new("confirmsave");
        let (mut state, script) = state();
        state.new_document();
        draw(&mut state);
        state.request_close(1);
        script.will_save_to(Some(dir.path("kept.odx")));
        state.answer_confirm(Choice::Save);
        assert_eq!(state.docs.len(), 1);
        assert!(dir.path("kept.odx").exists());
    }

    #[test]
    fn cancelling_the_save_dialog_keeps_the_question_open() {
        let (mut state, _script) = state();
        state.new_document();
        draw(&mut state);
        state.request_close(1);
        state.answer_confirm(Choice::Save); // the dialog says cancel
        assert_eq!(state.docs.len(), 2);
        assert!(state.confirm.is_some());
    }

    #[test]
    fn closing_the_last_drawing_asks_then_leaves_the_home_page() {
        let (mut state, _) = state();
        draw(&mut state);
        state.request_close(0);
        assert!(state.confirm.is_some());
        state.answer_confirm(Choice::Discard);
        assert!(!state.has_drawings());
        assert!(state.home);
        assert!(state.confirm.is_none());
    }

    #[test]
    fn the_home_page_has_nothing_to_close() {
        let (mut state, _) = state();
        state.start_on_home();
        state.request_close(0);
        assert!(state.confirm.is_none());
        assert!(state.try_exit());
    }

    #[test]
    fn exit_with_nothing_unsaved_goes_ahead() {
        let (mut state, _) = state();
        assert!(state.try_exit());
        assert!(state.exit_ok);
    }

    #[test]
    fn exit_asks_about_each_modified_drawing_in_turn() {
        let dir = Scratch::new("exit");
        let (mut state, script) = state();
        draw(&mut state);
        state.new_document();
        state.new_document();
        draw(&mut state);
        assert!(!state.try_exit());
        assert!(!state.exit_ok);
        assert_eq!(state.confirm_subject().unwrap().title, "Drawing1");

        script.will_save_to(Some(dir.path("one.odx")));
        state.answer_confirm(Choice::Save);
        assert_eq!(state.confirm_subject().unwrap().title, "Drawing3");
        assert!(!state.exit_ok);
        state.answer_confirm(Choice::Discard);
        assert!(state.exit_ok);
        assert!(state.confirm.is_none());
    }

    #[test]
    fn cancelling_one_question_cancels_the_exit() {
        let (mut state, _) = state();
        draw(&mut state);
        state.new_document();
        draw(&mut state);
        state.try_exit();
        state.answer_confirm(Choice::Discard);
        state.answer_confirm(Choice::Cancel);
        assert!(!state.exit_ok);
        assert!(state.confirm.is_none());
        assert!(
            state.docs.iter().all(|d| d.is_modified()),
            "nothing was discarded"
        );
    }

    #[test]
    fn a_drawing_closed_while_asked_about_is_skipped() {
        let (mut state, _) = state();
        draw(&mut state);
        state.new_document();
        draw(&mut state);
        state.try_exit();
        state.docs.remove(0); // the tab went away behind the question
        state.answer_confirm(Choice::Save);
        assert!(state.confirm_subject().is_some(), "on to the other drawing");
    }

    #[test]
    fn closing_a_tab_to_the_left_keeps_the_same_drawing_active() {
        let (mut state, _) = state();
        state.new_document();
        state.new_document();
        let uid = state.docs[2].uid;
        state.active = 2;
        state.close_document(0);
        assert_eq!(state.docs[state.active].uid, uid);
    }

    #[test]
    fn undoing_back_to_the_saved_state_clears_the_star() {
        let dir = Scratch::new("star");
        let (mut state, script) = state();
        draw(&mut state);
        script.will_save_to(Some(dir.path("plan.odx")));
        state.save_active();
        assert_eq!(state.docs[0].tab_title(), "plan");
        draw(&mut state);
        assert_eq!(state.docs[0].tab_title(), "plan*");
        state.undo();
        assert_eq!(state.docs[0].tab_title(), "plan");
    }

    #[test]
    fn a_saved_drawing_keeps_working_after_reopening() {
        let dir = Scratch::new("reopen");
        let (mut state, script) = state();
        draw(&mut state);
        draw(&mut state);
        let first = state.docs[0].session.document().objects()[0].id;
        state.docs[0]
            .session
            .perform(od_commands::RemoveEntities::new(vec![first]));
        script.will_save_to(Some(dir.path("plan.odx")));
        state.save_active();

        let (mut other, _) = self::state();
        other.open_path(&dir.path("plan.odx"));
        draw(&mut other);
        let ids: Vec<_> = other.docs[0]
            .session
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(
            !ids.contains(&first),
            "a retired id is not handed out again"
        );
    }
}
