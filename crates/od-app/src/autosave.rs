//! Autosave: a recovery copy of each drawing with unsaved changes, and
//! offering back the drawings a crash left behind. The files and the crash
//! detection are in `recovery.rs`; this connects them to the open drawings.

use std::path::Path;

use od_commands::Session;

use crate::io::Done;
use crate::recovery::Recovery;
use crate::state::AppState;

impl AppState {
    /// Begin this run's autosave session under `root`, and collect what an
    /// earlier crashed run left there (see `pending_recovery`).
    pub fn start_recovery(&mut self, root: &Path) {
        match Recovery::open(root) {
            Ok(recovery) => {
                self.pending_recovery = Recovery::find_orphans(root, recovery.dir());
                self.recovery = Some(std::sync::Arc::new(recovery));
            }
            Err(error) => self.cmdline.say(format!("Autosave is off: {error}")),
        }
    }

    /// Write a recovery copy of every drawing changed since its last one, and
    /// delete the copies of drawings that are clean again. The writing is done
    /// in the background from copies taken now; if the last pass has not
    /// finished this one is skipped, so a slow disk never queues them up.
    pub fn autosave(&mut self) {
        if self.autosaving {
            return;
        }
        let Some(recovery) = self.recovery.clone() else {
            return;
        };
        let mut writes = Vec::new();
        let mut removals = Vec::new();
        for doc in &mut self.docs {
            let position = doc.drawing().history().position();
            match (doc.is_modified(), doc.autosaved) {
                (true, last) if last != Some(position) => writes.push((
                    doc.uid,
                    position,
                    doc.title.clone(),
                    // A recovered drawing is still "from" where it was.
                    doc.path.clone().or_else(|| doc.recovery_origin.clone()),
                    doc.drawing().document().clone(),
                )),
                (false, Some(_)) => {
                    removals.push(doc.uid);
                    doc.autosaved = None;
                }
                _ => {}
            }
        }
        if writes.is_empty() && removals.is_empty() {
            return;
        }
        self.autosaving = true;
        self.submit(move || {
            for uid in removals {
                recovery.remove(uid);
            }
            Done::Autosaved(
                writes
                    .into_iter()
                    .map(|(uid, position, title, path, document)| {
                        let result = recovery.write(uid, &title, path.as_deref(), &document);
                        (uid, position, result)
                    })
                    .collect(),
            )
        });
    }

    /// A pass of recovery copies finished.
    pub(crate) fn finish_autosave(&mut self, results: Vec<(u64, u64, std::io::Result<()>)>) {
        self.autosaving = false;
        for (uid, position, result) in results {
            match result {
                Ok(()) => {
                    // Only if it still needs the copy: it may have been saved
                    // or closed while the copy was being written.
                    if let Some(doc) = self.docs.iter_mut().find(|d| d.uid == uid) {
                        if doc.is_modified() {
                            doc.autosaved = Some(position);
                        }
                    }
                }
                Err(error) => self.message = format!("Autosave failed: {error}"),
            }
        }
    }

    /// Drawing `uid` no longer needs a recovery copy (saved or closed). The
    /// file is deleted by the worker, after any copy still being written.
    pub(crate) fn forget_recovery(&mut self, uid: u64) {
        if let Some(recovery) = self.recovery.clone() {
            self.submit(move || {
                recovery.remove(uid);
                Done::Finished
            });
        }
        if let Some(doc) = self.docs.iter_mut().find(|d| d.uid == uid) {
            doc.autosaved = None;
        }
    }

    /// Open the drawings of crashed session `index` (of `pending_recovery`)
    /// as unsaved ones with no file of their own: the first Save asks where,
    /// starting at where each came from, so it cannot overwrite a newer
    /// original. The session's copies are deleted after the attempt; a
    /// drawing that could not be read is noted in `recovery_report` (and
    /// said on the command line), since its copy is gone too.
    pub fn recover_session(&mut self, index: usize) {
        if index >= self.pending_recovery.len() {
            return;
        }
        let orphan = self.pending_recovery.remove(index);
        let (mut recovered, mut failed) = (0, 0);
        for item in &orphan.items {
            match od_files::load(&item.file) {
                Ok(loaded) => {
                    let session = Session::recovered(loaded.document);
                    let doc = self.install_document(item.title.clone(), session);
                    doc.recovery_origin.clone_from(&item.original);
                    doc.view.request_zoom_extents();
                    recovered += 1;
                }
                Err(error) => {
                    self.cmdline
                        .say(format!("Could not recover {}: {error}", item.title));
                    self.recovery_report
                        .failed
                        .push((item.title.clone(), error.to_string()));
                    failed += 1;
                }
            }
        }
        orphan.discard();
        self.recovery_report.recovered += recovered;
        self.message = format!(
            "Recovered {}{}",
            crate::text::plural(recovered, "unsaved drawing"),
            if failed > 0 {
                format!(", {failed} could not be read")
            } else {
                String::new()
            }
        );
        self.cmdline.say(self.message.clone());
    }

    /// Delete the recovery copies of session `index` for good.
    pub fn discard_session(&mut self, index: usize) {
        if index < self.pending_recovery.len() {
            self.pending_recovery.remove(index).discard();
        }
    }

    /// Decide later: stop offering this run, keep the copies for the next.
    pub fn defer_recovery(&mut self) {
        self.pending_recovery.clear();
    }
}

/// What this run's recoveries came to.
#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub recovered: usize,
    /// Drawings that could not be read: title and why.
    pub failed: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use od_core::{Entity, Point};

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("od-auto-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn draw(state: &mut AppState) {
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 5.0),
        });
    }

    fn autosaved_files(state: &AppState) -> usize {
        let Some(recovery) = &state.recovery else {
            return 0;
        };
        fs::read_dir(recovery.dir())
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "odx"))
            .count()
    }

    #[test]
    fn only_drawings_with_changes_are_autosaved() {
        let root = Scratch::new("changed");
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        state.new_document();
        state.autosave();
        assert_eq!(autosaved_files(&state), 0, "nothing is modified");

        draw(&mut state);
        state.autosave();
        assert_eq!(autosaved_files(&state), 1);
    }

    #[test]
    fn a_drawing_is_not_rewritten_until_it_changes_again() {
        let root = Scratch::new("rewrite");
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        draw(&mut state);
        state.autosave();
        let first = state.docs[0].autosaved;
        state.autosave();
        assert_eq!(state.docs[0].autosaved, first);
        draw(&mut state);
        state.autosave();
        assert_ne!(state.docs[0].autosaved, first);
    }

    #[test]
    fn saving_or_undoing_back_removes_the_recovery_copy() {
        let root = Scratch::new("clean");
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        draw(&mut state);
        state.autosave();
        assert_eq!(autosaved_files(&state), 1);
        state.undo();
        state.autosave();
        assert_eq!(
            autosaved_files(&state),
            0,
            "clean again, nothing to recover"
        );

        draw(&mut state);
        state.autosave();
        let script = crate::picker::Script::default();
        state.picker = Box::new(crate::picker::ScriptedPicker(script.clone()));
        script.will_save_to(Some(root.0.join("plan.odx")));
        state.save_active();
        assert_eq!(autosaved_files(&state), 0, "saved: the file is the copy");
    }

    #[test]
    fn closing_a_drawing_removes_its_recovery_copy() {
        let root = Scratch::new("close");
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        state.new_document();
        draw(&mut state);
        state.autosave();
        assert_eq!(autosaved_files(&state), 1);
        state.close_document(1);
        assert_eq!(autosaved_files(&state), 0);
    }

    /// A session that crashed with drawing "plan" (from `original`) and an
    /// untitled one, as a fresh app finds it.
    fn crashed_session(root: &Scratch, original: &Path) -> AppState {
        {
            let mut crashed = AppState::new();
            crashed.start_recovery(&root.0);
            draw(&mut crashed);
            draw(&mut crashed);
            crashed.docs[0].path = Some(original.to_owned());
            crashed.docs[0].title = "plan".into();
            crashed.new_document();
            draw(&mut crashed);
            crashed.autosave();
            // The process dies: the lock goes, the files stay.
            let recovery = crashed.recovery.take().unwrap();
            recovery_crash(recovery);
        }
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        state
    }

    #[test]
    fn a_crashed_run_is_offered_back_and_recovers_as_unsaved_with_no_file() {
        let root = Scratch::new("recover");
        let original = root.0.join("plan.odx");
        let mut state = crashed_session(&root, &original);
        assert_eq!(state.pending_recovery.len(), 1);
        assert_eq!(state.pending_recovery[0].items.len(), 2);
        state.recover_session(0);
        assert!(state.pending_recovery.is_empty());
        // The blank drawing makes way for the first; the second is a new tab.
        let plan = state.docs.iter().find(|d| d.title == "plan").unwrap();
        assert_eq!(plan.path, None, "Save must not go to the original");
        assert_eq!(plan.recovery_origin, Some(original));
        assert_eq!(plan.session.document().len(), 2);
        assert!(plan.is_modified(), "it is not in any file yet");
        assert_eq!(state.recovery_report.recovered, 2);
        assert!(state.recovery_report.failed.is_empty());
        assert!(state.message.contains("Recovered 2 unsaved drawings"));
        // And the old session's files are gone.
        let mut other = AppState::new();
        other.start_recovery(&root.0);
        assert!(other.pending_recovery.is_empty());
    }

    #[test]
    fn the_first_save_of_a_recovered_drawing_asks_where_and_starts_at_the_original() {
        let root = Scratch::new("saveas");
        let original = root.0.join("work").join("plan.odx");
        let mut state = crashed_session(&root, &original);
        state.recover_session(0);
        let script = crate::picker::Script::default();
        state.picker = Box::new(crate::picker::ScriptedPicker(script.clone()));
        let index = state.docs.iter().position(|d| d.title == "plan").unwrap();
        state.show_document(index);
        // Cancelling the dialog leaves it unsaved and the original alone.
        assert!(!state.save_active());
        assert_eq!(script.suggested(), ["plan.odx"]);
        assert_eq!(script.directories(), [Some(root.0.join("work"))]);
        assert_eq!(script.titles(), ["Save recovered drawing as"]);
        assert!(state.docs[index].is_modified());
        assert!(!original.exists());
        // Saving somewhere makes that its file and forgets the origin.
        let elsewhere = root.0.join("plan-recovered.odx");
        script.will_save_to(Some(elsewhere.clone()));
        assert!(state.save_active());
        assert_eq!(state.docs[index].path, Some(elsewhere));
        assert_eq!(state.docs[index].recovery_origin, None);
        assert!(!original.exists(), "the original was never written");
    }

    #[test]
    fn a_recovered_drawing_is_autosaved_still_from_its_original() {
        let root = Scratch::new("again");
        let original = root.0.join("plan.odx");
        let mut state = crashed_session(&root, &original);
        state.recover_session(0);
        state.autosave();
        let again = std::sync::Arc::into_inner(state.recovery.take().unwrap()).unwrap();
        again.abandon();
        let mut next = AppState::new();
        next.start_recovery(&root.0);
        let items = &next.pending_recovery[0].items;
        assert!(items
            .iter()
            .any(|i| i.original.as_deref() == Some(original.as_path())));
    }

    #[test]
    fn a_damaged_copy_is_reported_and_the_rest_still_recovers() {
        let root = Scratch::new("damaged");
        let mut state = crashed_session(&root, &root.0.join("plan.odx"));
        let bad = state.pending_recovery[0].items[0].file.clone();
        fs::write(&bad, "not a drawing").unwrap();
        let bad_title = state.pending_recovery[0].items[0].title.clone();
        state.recover_session(0);
        assert_eq!(state.recovery_report.recovered, 1);
        assert_eq!(state.recovery_report.failed.len(), 1);
        assert_eq!(state.recovery_report.failed[0].0, bad_title);
        assert!(!state.recovery_report.failed[0].1.is_empty());
        assert!(state.message.contains("1 could not be read"));
        assert!(state.pending_recovery.is_empty());
    }

    /// Leave the session folder behind, as a crash would.
    fn recovery_crash(recovery: std::sync::Arc<Recovery>) {
        std::sync::Arc::into_inner(recovery)
            .expect("no job still holds the session")
            .abandon();
    }

    #[test]
    fn discarding_deletes_what_a_crash_left() {
        let root = Scratch::new("discard");
        {
            let mut crashed = AppState::new();
            crashed.start_recovery(&root.0);
            draw(&mut crashed);
            crashed.autosave();
            recovery_crash(crashed.recovery.take().unwrap());
        }
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        assert_eq!(state.pending_recovery.len(), 1);
        state.discard_session(0);
        assert!(state.pending_recovery.is_empty());
        let mut next = AppState::new();
        next.start_recovery(&root.0);
        assert!(next.pending_recovery.is_empty());
    }

    #[test]
    fn deciding_later_keeps_the_copies_for_next_time() {
        let root = Scratch::new("later");
        {
            let mut crashed = AppState::new();
            crashed.start_recovery(&root.0);
            draw(&mut crashed);
            crashed.autosave();
            recovery_crash(crashed.recovery.take().unwrap());
        }
        let mut state = AppState::new();
        state.start_recovery(&root.0);
        state.defer_recovery();
        assert!(state.pending_recovery.is_empty());
        drop(state);
        let mut next = AppState::new();
        next.start_recovery(&root.0);
        assert_eq!(next.pending_recovery.len(), 1);
    }

    #[test]
    fn without_a_recovery_session_autosave_does_nothing() {
        let mut state = AppState::new();
        draw(&mut state);
        state.autosave();
        assert_eq!(state.docs[0].autosaved, None);
    }
}
