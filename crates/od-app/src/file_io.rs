//! Opening, saving and exporting without freezing the window. Each is in two
//! halves: `begin_*` takes what the job needs (a path, a copy of the drawing)
//! and hands it to the worker in `io.rs`; `finish_*` applies the result on the
//! UI thread when `poll_io` collects it. Only the UI thread ever touches the
//! drawings, so there is no sharing to get wrong.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use od_commands::Session;

use crate::files::{display_name, same_file};
use crate::io::Done;
use crate::state::AppState;

/// How long file work runs before Cancel appears.
const STALLED_AFTER: Duration = Duration::from_secs(1);

impl AppState {
    /// Hand a job to the worker. In inline mode (tests) it has already run, so
    /// collect its result now.
    pub(crate) fn submit(&mut self, job: impl FnOnce() -> Done + Send + 'static) {
        self.io.submit(job);
        if self.io.is_inline() {
            self.poll_io();
        }
    }

    /// Like `submit`, for a job the user can give up on.
    fn submit_cancellable(
        &mut self,
        job: impl FnOnce(&AtomicBool) -> Done + Send + 'static,
    ) -> u64 {
        let id = self.io.submit_cancellable(job);
        if self.io.is_inline() {
            self.poll_io();
        }
        id
    }

    /// Give up on one worker job (by id) without touching the others.
    pub(crate) fn cancel_file_job(&mut self, id: u64) -> bool {
        self.io.cancel_job(id)
    }

    /// A file the user is waiting for has taken a noticeable time, so the
    /// status bar offers Cancel (and Esc does it).
    pub fn file_work_stalled(&self) -> bool {
        self.io.stalled(STALLED_AFTER)
    }

    /// Give up on the open, save or export in progress. The drawing stays as
    /// it was, still unsaved if it was; the old file is never replaced; and a
    /// quit that was waiting for the write is called off.
    pub fn cancel_file_work(&mut self) {
        if self.io.cancel_all() == 0 {
            return;
        }
        self.opening.clear();
        self.saving.clear();
        if let Some(confirm) = self.confirm.as_mut() {
            confirm.set_saving(None);
        }
        self.exit_ok = false;
        self.file_ops.cancel_running();
        self.tell("Cancelled. Nothing was changed on disk.".to_owned());
    }

    /// Apply everything the worker has finished. Called every frame.
    pub fn poll_io(&mut self) {
        while let Some(done) = self.io.next() {
            self.handle(done);
        }
    }

    fn handle(&mut self, done: Done) {
        match done {
            Done::Opened { path, result } => {
                let report = match &*result {
                    Ok(loaded) => Ok(loaded.warnings.clone()),
                    Err(error) => Err(error.to_string()),
                };
                self.finish_open(&path, *result);
                let uid = self.docs.get(self.active).map(|d| d.uid);
                self.file_ops
                    .opened(&path, report.map(|notes| (uid, notes)));
            }
            Done::Saved {
                uid,
                path,
                position,
                result,
            } => {
                let report = result
                    .as_ref()
                    .map(Clone::clone)
                    .map_err(ToString::to_string);
                self.finish_save(uid, &path, position, result);
                self.file_ops.saved(uid, report);
            }
            Done::Exported { path, result } => {
                let report = result
                    .as_ref()
                    .map(Clone::clone)
                    .map_err(ToString::to_string);
                self.finish_export(&path, result);
                self.file_ops.exported(&path, report);
            }
            Done::Autosaved(results) => self.finish_autosave(results),
            Done::Finished => {}
        }
    }

    /// Whether file work is still going on.
    pub fn io_busy(&self) -> bool {
        self.io.busy()
    }

    /// The window may close: every question is answered and no file is still
    /// being written (closing then would lose it).
    pub fn can_quit(&self) -> bool {
        self.exit_ok && self.confirm.is_none() && !self.io.busy()
    }

    /// Wait for all file work to finish and apply it (tests).
    #[cfg(test)]
    pub fn finish_io(&mut self) {
        self.poll_io();
        while self.io.busy() {
            match self.io.wait_next() {
                Some(done) => self.handle(done),
                None => break,
            }
        }
    }

    // ---- open ----

    /// Start reading `path`. The drawing appears in its tab when the read is
    /// done (`finish_open`).
    pub(crate) fn begin_open(&mut self, path: &Path) -> Option<u64> {
        if self.opening.iter().any(|p| same_file(p, path)) {
            return None;
        }
        self.opening.push(path.to_owned());
        self.message = format!("Opening {}...", display_name(path));
        let path = path.to_owned();
        Some(self.submit_cancellable(move |cancelled| Done::Opened {
            result: Box::new(od_files::load_cancellable(&path, cancelled)),
            path,
        }))
    }

    fn finish_open(&mut self, path: &Path, result: Result<od_files::Loaded, od_files::FileError>) {
        self.opening.retain(|p| p != path);
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(error) => {
                self.locate.retain(|(p, _)| p != path);
                self.tell(format!("Could not open {}: {error}", path.display()));
                return;
            }
        };
        // Opened by some other route while it was being read: use that tab.
        if let Some(i) = self
            .docs
            .iter()
            .position(|d| d.path.as_deref().is_some_and(|p| same_file(p, path)))
        {
            self.active = i;
            self.commit_recent(path);
            return;
        }
        self.abort_command();
        let title = display_name(path);
        for warning in &loaded.warnings {
            self.cmdline.say(warning.clone());
        }
        if loaded.imported {
            // An exchange format (DXF, DWG): it opens as a new drawing that
            // has no file of its own yet, so it counts as unsaved.
            let objects = loaded.document.len();
            let session = Session::recovered(loaded.document);
            let doc = self.install_document(title, session);
            doc.view.request_zoom_extents();
            self.commit_recent(path);
            let left_out = if loaded.warnings.is_empty() {
                ""
            } else {
                " (see the command line for what was left out)"
            };
            let file = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            self.tell(format!(
                "Imported {file}: {}{left_out}",
                crate::text::plural(objects, "object")
            ));
            return;
        }
        let session = Session::with_document(loaded.document);
        let doc = self.install_document(title.clone(), session);
        doc.path = Some(path.to_owned());
        doc.view.request_zoom_extents();
        self.commit_recent(path);
        self.message = format!("Opened {title}");
    }

    // ---- save ----

    /// Start writing drawing `index` to `path`, from a copy taken now. False
    /// if that drawing is already being written.
    pub(crate) fn begin_save(&mut self, index: usize, path: &Path) -> bool {
        self.begin_save_job(index, path).is_some()
    }

    /// As `begin_save`, returning the worker's job id.
    pub(crate) fn begin_save_job(&mut self, index: usize, path: &Path) -> Option<u64> {
        let doc = &self.docs[index];
        if self.saving.contains(&doc.uid) {
            self.message = format!("{} is already being saved", doc.title);
            return None;
        }
        let (uid, position) = (doc.uid, doc.drawing().history.position());
        let copy = doc.drawing().document.clone();
        self.saving.push(uid);
        self.message = format!("Saving {}...", display_name(path));
        let path = path.to_owned();
        Some(self.submit_cancellable(move |cancelled| Done::Saved {
            result: od_files::save_cancellable(&path, &copy, cancelled),
            uid,
            path,
            position,
        }))
    }

    fn finish_save(
        &mut self,
        uid: u64,
        path: &Path,
        position: u64,
        result: Result<Vec<String>, od_files::FileError>,
    ) {
        self.saving.retain(|&u| u != uid);
        let asked = self.confirm.as_ref().and_then(|c| c.saving()) == Some(uid);
        if let Err(error) = result {
            self.tell(format!("Could not save {}: {error}", path.display()));
            if asked {
                // Stay on the question so the user can try again or give up.
                if let Some(confirm) = self.confirm.as_mut() {
                    confirm.set_saving(None);
                }
            }
            return;
        }
        let title = display_name(path);
        if let Some(doc) = self.docs.iter_mut().find(|d| d.uid == uid) {
            doc.path = Some(path.to_owned());
            doc.title.clone_from(&title);
            // What was written is the drawing as it was when the copy was
            // taken; anything done since still counts as unsaved.
            doc.drawing_mut().mark_saved_at(position);
            if !doc.is_modified() {
                self.forget_recovery(uid);
            }
        }
        self.recent.add(path);
        self.tell(format!("Saved {title}"));
        if asked {
            self.next_pending();
        }
    }

    // ---- export ----

    pub(crate) fn begin_export(&mut self, path: &Path) {
        self.begin_export_of(self.active, path);
    }

    /// Export drawing `index`; returns the worker's job id.
    pub(crate) fn begin_export_of(&mut self, index: usize, path: &Path) -> u64 {
        let copy = self.docs[index].drawing().document().clone();
        self.message = format!("Exporting {}...", display_name(path));
        let path = path.to_owned();
        self.submit_cancellable(move |cancelled| Done::Exported {
            result: od_files::save_cancellable(&path, &copy, cancelled),
            path,
        })
    }

    /// Write a layout of the active drawing as PDF or SVG, from a copy.
    pub(crate) fn begin_plot(&mut self, path: &Path, layout: od_core::LayoutId) {
        self.begin_plot_of(self.active, path, layout);
    }

    /// Plot a layout of drawing `index`; returns the worker's job id.
    pub(crate) fn begin_plot_of(
        &mut self,
        index: usize,
        path: &Path,
        layout: od_core::LayoutId,
    ) -> u64 {
        let copy = self.docs[index].drawing().document().clone();
        self.message = format!("Plotting {}...", display_name(path));
        let path = path.to_owned();
        self.submit_cancellable(move |cancelled| Done::Exported {
            result: od_files::plot_cancellable(&path, &copy, layout, cancelled),
            path,
        })
    }

    fn finish_export(&mut self, path: &Path, result: Result<Vec<String>, od_files::FileError>) {
        match result {
            Ok(notes) => {
                self.tell(format!("Exported {}", path.display()));
                for note in notes {
                    self.cmdline.say(note);
                }
            }
            Err(error) => self.tell(format!("Could not export {}: {error}", path.display())),
        }
    }
}

/// These run the worker on its own thread, as the app does; the tests in
/// `files.rs` run it inline.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use od_core::{Entity, Point};

    use crate::files::Choice;
    use crate::io::Io;
    use crate::picker::{Script, ScriptedPicker};
    use crate::state::AppState;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("od-bg-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn threaded() -> (AppState, Script) {
        let mut state = AppState::new();
        state.io = Io::background();
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
    fn saving_returns_at_once_and_applies_when_collected() {
        let dir = Scratch::new("save");
        let (mut state, script) = threaded();
        draw(&mut state);
        script.will_save_to(Some(dir.0.join("plan.odx")));
        assert!(state.save_active());
        assert!(state.docs[0].is_modified(), "not applied until collected");
        assert!(state.io_busy());
        state.finish_io();
        assert!(!state.docs[0].is_modified());
        assert_eq!(state.docs[0].path, Some(dir.0.join("plan.odx")));
        assert_eq!(state.message, "Saved plan");
    }

    #[test]
    fn edits_made_while_saving_stay_unsaved() {
        let dir = Scratch::new("during");
        let (mut state, script) = threaded();
        draw(&mut state);
        script.will_save_to(Some(dir.0.join("plan.odx")));
        state.save_active();
        draw(&mut state);
        state.finish_io();
        assert!(
            state.docs[0].is_modified(),
            "the second line is not on disk"
        );
        let saved = od_files::load(&dir.0.join("plan.odx")).unwrap();
        assert_eq!(saved.document.len(), 1);
        // The recovery copy is kept for the edit that is not saved.
        assert_eq!(state.docs[0].path, Some(dir.0.join("plan.odx")));
    }

    #[test]
    fn a_drawing_already_being_saved_is_not_saved_twice() {
        let dir = Scratch::new("twice");
        let (mut state, script) = threaded();
        draw(&mut state);
        script.will_save_to(Some(dir.0.join("plan.odx")));
        assert!(state.save_active());
        assert!(
            !state.begin_save(0, &dir.0.join("plan.odx")),
            "second request while one is running"
        );
        assert!(state.message.contains("already being saved"));
        state.finish_io();
        assert!(!state.docs[0].is_modified());
    }

    #[test]
    fn opening_shows_the_drawing_when_the_read_is_done() {
        let dir = Scratch::new("open");
        let mut document = od_core::Document::default();
        document.add(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 1.0),
        });
        let path = dir.0.join("plan.odx");
        od_files::save(&path, &document).unwrap();
        let (mut state, _) = threaded();
        assert!(state.open_path(&path));
        assert!(
            state.open_path(&path),
            "asking again does not read it twice"
        );
        assert_eq!(state.docs.len(), 1);
        assert!(state.message.starts_with("Opening"));
        state.finish_io();
        assert_eq!(state.docs.len(), 1, "the blank drawing made way");
        assert_eq!(state.docs[0].session.document().len(), 1);
        assert_eq!(state.docs[0].path, Some(path));
    }

    #[test]
    fn the_window_waits_for_files_being_written() {
        let dir = Scratch::new("quit");
        let (mut state, script) = threaded();
        draw(&mut state);
        script.will_save_to(Some(dir.0.join("plan.odx")));
        state.save_active();
        state.exit_ok = true;
        assert!(!state.can_quit(), "still writing");
        state.finish_io();
        assert!(state.can_quit());
    }

    #[test]
    fn the_unsaved_question_waits_for_the_save_then_closes() {
        let dir = Scratch::new("confirm");
        let (mut state, script) = threaded();
        state.new_document();
        draw(&mut state);
        script.will_save_to(Some(dir.0.join("plan.odx")));
        state.request_close(1);
        assert!(state.confirm.is_some());
        state.answer_confirm(Choice::Save);
        assert!(
            state.confirm.as_ref().is_some_and(|c| c.saving().is_some()),
            "asks no more, but stays up while writing"
        );
        assert_eq!(state.docs.len(), 2);
        state.finish_io();
        assert!(state.confirm.is_none());
        assert_eq!(state.docs.len(), 1, "closed once written");
    }

    #[test]
    fn autosave_skips_a_pass_while_the_last_is_still_writing() {
        let root = Scratch::new("auto");
        let (mut state, _) = threaded();
        state.start_recovery(&root.0);
        draw(&mut state);
        state.autosave();
        assert!(state.autosaving);
        draw(&mut state);
        state.autosave(); // skipped, not queued
        state.finish_io();
        assert!(!state.autosaving);
        assert_ne!(
            state.docs[0].autosaved,
            Some(state.session().history().position())
        );
        state.autosave();
        state.finish_io();
        assert_eq!(
            state.docs[0].autosaved,
            Some(state.session().history().position())
        );
    }

    #[test]
    fn closing_a_drawing_after_its_autosave_leaves_no_copy_behind() {
        let root = Scratch::new("order");
        let (mut state, _) = threaded();
        state.start_recovery(&root.0);
        state.new_document();
        draw(&mut state);
        state.autosave();
        state.close_document(1); // removal is queued behind the write
        state.finish_io();
        let copies = std::fs::read_dir(state.recovery.as_ref().unwrap().dir())
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "odx"))
            .count();
        assert_eq!(copies, 0);
    }

    /// A job that stays stuck until the returned sender is dropped, as a
    /// write to a dead network path would.
    fn stuck(state: &mut AppState) -> std::sync::mpsc::Sender<()> {
        let (release, held) = std::sync::mpsc::channel::<()>();
        state.io.submit_cancellable(move |_| {
            let _ = held.recv_timeout(std::time::Duration::from_secs(10));
            crate::io::Done::Finished
        });
        release
    }

    #[test]
    fn cancelling_gives_up_cleanly_and_calls_off_a_quit() {
        let (mut state, _) = threaded();
        draw(&mut state);
        let uid = state.docs[0].uid;
        let _release = stuck(&mut state);
        state.saving.push(uid);
        state.opening.push(PathBuf::from("x.odx"));
        state.exit_ok = true;
        assert!(!state.can_quit());

        state.cancel_file_work();
        assert!(!state.io_busy(), "no longer waited for");
        assert!(state.saving.is_empty() && state.opening.is_empty());
        assert!(!state.exit_ok, "the quit that waited for it is off");
        assert!(state.message.starts_with("Cancelled"));
        assert!(state.docs[0].is_modified(), "still unsaved");

        // With nothing to cancel it does nothing, and says nothing.
        state.message.clear();
        state.cancel_file_work();
        assert!(state.message.is_empty());
    }

    #[test]
    fn a_cancelled_save_can_be_tried_again() {
        let dir = Scratch::new("retry");
        let (mut state, script) = threaded();
        draw(&mut state);
        let _release = stuck(&mut state);
        state.saving.push(state.docs[0].uid);
        state.cancel_file_work();
        script.will_save_to(Some(dir.0.join("plan.odx")));
        assert!(state.save_active(), "not blocked by the abandoned one");
        state.finish_io();
        assert!(!state.docs[0].is_modified());
    }

    #[test]
    fn esc_cancels_file_work_only_once_it_is_slow() {
        let (mut state, _) = threaded();
        let _release = stuck(&mut state);
        assert!(!state.can_cancel(), "a quick save is not cancelled by Esc");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(state.file_work_stalled());
        assert!(state.can_cancel());
        state.cancel();
        assert!(!state.io_busy());
    }
}
