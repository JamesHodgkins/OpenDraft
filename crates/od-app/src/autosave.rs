//! Autosave: a recovery copy of each drawing with unsaved changes, and
//! offering back the drawings a crash left behind. The files and the crash
//! detection are in `recovery.rs`; this connects them to the open drawings.

use std::path::Path;

use od_commands::Session;

use crate::io::Done;
use crate::recovery::{Orphan, Recovery};
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
            let position = doc.drawing().history.position();
            match (doc.is_modified(), doc.autosaved) {
                (true, last) if last != Some(position) => writes.push((
                    doc.uid,
                    position,
                    doc.title.clone(),
                    doc.path.clone(),
                    doc.drawing().document.clone(),
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

    /// Open the drawings a crash left behind, as unsaved ones (saving them
    /// writes to where they came from, if they had been saved before).
    pub fn recover_pending(&mut self) {
        let orphans: Vec<Orphan> = std::mem::take(&mut self.pending_recovery);
        let mut recovered = 0;
        for orphan in orphans {
            for item in &orphan.items {
                match od_files::load(&item.file) {
                    Ok(loaded) => {
                        let session = Session::recovered(loaded.document);
                        let doc = self.install_document(item.title.clone(), session);
                        doc.path.clone_from(&item.original);
                        recovered += 1;
                    }
                    Err(error) => self
                        .cmdline
                        .say(format!("Could not recover {}: {error}", item.title)),
                }
            }
            orphan.discard();
        }
        if recovered > 0 {
            self.message = format!(
                "Recovered {recovered} unsaved drawing{}",
                if recovered == 1 { "" } else { "s" }
            );
            self.cmdline.say(self.message.clone());
        }
    }

    /// Delete the recovery copies for good.
    pub fn discard_pending(&mut self) {
        for orphan in std::mem::take(&mut self.pending_recovery) {
            orphan.discard();
        }
    }

    /// Decide later: stop asking this run, keep the copies for the next.
    pub fn defer_pending(&mut self) {
        self.pending_recovery.clear();
    }
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

    #[test]
    fn a_crashed_run_is_offered_back_and_recovers_as_unsaved() {
        let root = Scratch::new("recover");
        let original = root.0.join("plan.odx");
        {
            let mut crashed = AppState::new();
            crashed.start_recovery(&root.0);
            draw(&mut crashed);
            draw(&mut crashed);
            crashed.docs[0].path = Some(original.clone());
            crashed.docs[0].title = "plan".into();
            crashed.autosave();
            // The process dies: the lock goes, the files stay.
            let recovery = crashed.recovery.take().unwrap();
            recovery_crash(recovery);
        }

        let mut state = AppState::new();
        state.start_recovery(&root.0);
        assert_eq!(state.pending_recovery.len(), 1);
        state.recover_pending();
        assert!(state.pending_recovery.is_empty());
        // The blank drawing is still there; the recovered one is a new tab.
        let doc = state.docs.last().unwrap();
        assert_eq!(doc.title, "plan");
        assert_eq!(doc.path, Some(original));
        assert_eq!(doc.session.document().len(), 2);
        assert!(doc.is_modified(), "it is not in any file yet");
        assert!(state.message.contains("Recovered 1 unsaved drawing"));
        // And the old session's files are gone.
        let mut other = AppState::new();
        other.start_recovery(&root.0);
        assert!(other.pending_recovery.is_empty());
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
        state.discard_pending();
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
        state.defer_pending();
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
