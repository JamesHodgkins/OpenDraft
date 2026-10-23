//! Application state that commands operate on. No UI code here.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::cmdline::CommandLine;
use crate::dock::LayoutRequest;
use crate::files::Confirm;
use crate::io::Io;
use crate::picker::FilePicker;
use crate::properties::{self, Field};
use crate::recent::RecentFiles;
use crate::recovery::{Orphan, Recovery};
use crate::settings::Settings;
use crate::text::plural;
use od_canvas::{RegionSelect, View};
use od_commands::{Action, ModifyEntities, Runner, Session};
use od_core::{EntityId, EntityKind, Point};

/// One open drawing.
pub struct Doc {
    /// Identifies the drawing for as long as it is open, even as tabs close
    /// and shift (an index would not).
    pub uid: u64,
    /// The name shown on its tab: the file's name, or "Drawing1".
    pub title: String,
    /// The file it was opened from or last saved to.
    pub path: Option<PathBuf>,
    pub session: Session,
    pub view: View,
    pub dynamic_input: crate::dynamic_input::DynamicInputState,
    /// Selected objects. Ids that an edit, undo or redo removes from the
    /// drawing drop out of it; the rest stay selected.
    pub selection: BTreeSet<EntityId>,
    /// Where the history stood when its recovery copy was last written.
    pub autosaved: Option<u64>,
    /// The in-place text editor, while the running command waits on it.
    pub mtext_editor: Option<crate::mtext_editor::MTextEditor>,
    /// The block being edited in the Block Editor, with the drawing set
    /// aside meanwhile (see `block_editor.rs`).
    pub block_edit: Option<Box<crate::block_editor::BlockEdit>>,
    /// Stands in for "no drawing open" so there is always something for the
    /// rest of the app to point at. It has no tab and is never saved.
    pub placeholder: bool,
    /// Where a recovered drawing came from. It is not the drawing's file
    /// (`path` is none, so Save asks where): only where Save As starts, until
    /// the drawing is saved.
    pub recovery_origin: Option<PathBuf>,
}

impl Doc {
    pub fn new(uid: u64, title: String, session: Session) -> Self {
        Self {
            uid,
            title,
            path: None,
            session,
            view: View::default(),
            dynamic_input: Default::default(),
            selection: BTreeSet::new(),
            autosaved: None,
            mtext_editor: None,
            block_edit: None,
            placeholder: false,
            recovery_origin: None,
        }
    }

    /// Whether it has changes that are not in its file. Unsaved changes in
    /// the Block Editor are not in the drawing yet, so they do not count.
    pub fn is_modified(&self) -> bool {
        self.drawing().is_modified()
    }

    /// The tab text: the name, with a star while it has unsaved changes, and
    /// the block being edited.
    pub fn tab_title(&self) -> String {
        let star = if self.is_modified() { "*" } else { "" };
        match &self.block_edit {
            Some(edit) => format!("{}{star} - Block Editor: {}", self.title, edit.name),
            None => format!("{}{star}", self.title),
        }
    }

    /// Forget selected ids that are no longer in the drawing or can no longer
    /// be selected (their layer was hidden, frozen or locked).
    pub(crate) fn prune_selection(&mut self) {
        let document = &self.session.document();
        self.selection.retain(|&id| {
            document
                .object(id)
                .is_some_and(|o| document.is_selectable(o))
        });
    }
}

pub struct AppState {
    pub docs: Vec<Doc>,
    /// User settings; commands such as the grid and snap toggles edit them.
    pub settings: Settings,
    pub active: usize,
    /// The Home tab is the one showing (see `home.rs`).
    pub home: bool,
    /// Feedback line for the status bar.
    pub message: String,
    /// World position under the pointer on the last frame, if over the canvas.
    pub cursor: Option<Point>,
    /// Whether the Settings window is open.
    pub show_settings: bool,
    /// Whether the Units dialog is open.
    pub show_units: bool,
    /// Whether the Point Style dialog is open.
    pub show_point_style: bool,
    /// The Point Style dialog's working copy while it is open.
    pub point_style_draft: Option<crate::point_vars::Draft>,
    /// The Power Select dialog.
    pub power_select: crate::power_select::Dialog,
    /// The Linetype Manager is open.
    pub show_linetypes: bool,
    /// The Text Style dialog is open.
    pub show_text_styles: bool,
    /// The Dimension Style dialog is open.
    pub show_dim_style: bool,
    /// The Dimension Style Manager is open.
    pub show_dim_manager: bool,
    /// What the Modify Dimension Style dialog edits.
    pub dim_edit: crate::dim_style_table::DimEdit,
    /// Which multileader-style dialogs are open.
    pub mleader_dialogs: crate::mleader_style_table::Dialogs,
    /// The Page Setup dialog's working copy while it is open.
    pub page_setup: Option<crate::layouts::PageDraft>,
    /// Panel layout change asked for by a command; the app applies it.
    pub layout_request: Option<LayoutRequest>,
    pub cmdline: CommandLine,
    /// The command in progress that asks through the `Editor`, if any.
    pub(crate) runner: Option<Runner>,
    /// Index of the drawing that command works on.
    pub(crate) run_doc: usize,
    pub(crate) next_doc_number: usize,
    pub(crate) next_doc_uid: u64,
    pub recent: RecentFiles,
    /// Files picked to stand in for a missing recent entry, until they open.
    pub(crate) locate: Vec<(std::path::PathBuf, crate::recent::RecentId)>,
    /// Where to ask for a Home-page picture after a drawing opens or saves;
    /// none in tests.
    pub previews: Option<crate::previews::PreviewHandle>,
    pub(crate) picker: Box<dyn FilePicker>,
    /// A question about unsaved changes waiting for the user's answer.
    pub confirm: Option<Confirm>,
    /// The user asked to quit (Exit command); the app acts on it.
    pub exit_requested: bool,
    /// Every unsaved change has been dealt with: the window may close.
    pub exit_ok: bool,
    /// This run's autosave session (none if the folder could not be made).
    pub(crate) recovery: Option<std::sync::Arc<Recovery>>,
    /// Drawings a crashed run left, waiting for the user to recover or
    /// discard them.
    pub pending_recovery: Vec<Orphan>,
    /// What this run's recoveries came to, for Home to say.
    pub recovery_report: crate::autosave::RecoveryReport,
    /// The worker that reads and writes files off the UI thread.
    pub(crate) io: Io,
    /// Drawings (by uid) being written right now.
    pub(crate) saving: Vec<u64>,
    /// Files being read right now.
    pub(crate) opening: Vec<PathBuf>,
    /// A pass of recovery copies is being written; the next one waits.
    pub(crate) autosaving: bool,
    /// Block Editor requests from commands (see `block_editor.rs`).
    pub(crate) block_requests: crate::block_editor::BlockEditRequests,
    /// File operations automation has asked for (see `automation/file_ops.rs`).
    pub(crate) file_ops: crate::automation::FileOps,
    /// Counts the commands started with `start_command`, so something can
    /// tell "this one" from the next.
    pub(crate) command_serial: u64,
    /// The snap override or `FRO`/`M2P` chain waiting for the next point.
    pub(crate) once: crate::snap_once::OneShot,
    /// Where the snap override menu is open (screen position), if it is.
    pub snap_menu: Option<(f32, f32)>,
    /// Where the right-click context menu is open (screen position), if it is.
    pub context_menu: Option<(f32, f32)>,
    /// What Copy and Cut last put aside.
    pub(crate) clipboard: crate::clipboard::ClipboardSlot,
    /// The snap point in force at the pointer on the last frame.
    pub snap_now: Option<od_canvas::SnapPoint>,
    /// How the last few commands ended, by serial.
    pub(crate) command_ends: Vec<(u64, od_commands::Outcome)>,
    /// What happened lately, for `events.poll`.
    pub(crate) events: crate::automation::EventLog,
    /// What keyed automation operations did, so a repeat does nothing.
    pub(crate) replays: crate::automation::Replays,
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self {
            docs: Vec::new(),
            settings: Settings::default(),
            active: 0,
            home: false,
            message: String::new(),
            cursor: None,
            show_settings: false,
            show_units: false,
            show_point_style: false,
            point_style_draft: None,
            power_select: Default::default(),
            show_linetypes: false,
            show_text_styles: false,
            show_dim_style: false,
            show_dim_manager: false,
            dim_edit: Default::default(),
            mleader_dialogs: Default::default(),
            page_setup: None,
            layout_request: None,
            cmdline: CommandLine::default(),
            runner: None,
            run_doc: 0,
            next_doc_number: 1,
            next_doc_uid: 1,
            recent: RecentFiles::default(),
            locate: Vec::new(),
            previews: None,
            picker: crate::files::default_picker(),
            confirm: None,
            exit_requested: false,
            exit_ok: false,
            recovery: None,
            pending_recovery: Vec::new(),
            recovery_report: Default::default(),
            io: crate::files::default_io(),
            saving: Vec::new(),
            opening: Vec::new(),
            autosaving: false,
            block_requests: Default::default(),
            file_ops: Default::default(),
            command_serial: 0,
            once: Default::default(),
            snap_menu: None,
            context_menu: None,
            clipboard: Default::default(),
            snap_now: None,
            command_ends: Vec::new(),
            events: Default::default(),
            replays: Default::default(),
        };
        state.new_document();
        state
    }

    /// Note what changed since the last look, for `events.poll`.
    pub(crate) fn observe_events(&mut self) {
        let running = self.runner.as_ref().map(|r| r.name().to_owned());
        self.events
            .observe(&self.docs, self.command_serial, running.as_deref());
    }

    pub fn session(&self) -> &Session {
        &self.docs[self.active].session
    }

    /// Run an undoable edit on the active drawing. An edit made from outside
    /// a running command (a panel, a shortcut) ends that command first, so it
    /// never works on a drawing that changed under it.
    pub fn perform(&mut self, action: impl Action + 'static) {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        doc.session.perform(action);
        doc.prune_selection();
    }

    /// Set one property on every selected entity (one undo step). Entities
    /// keep their positions, so the selection survives the edit. `kind`
    /// limits the edit to one kind of entity (the panel's type filter).
    pub fn set_property(&mut self, field: Field, label: &str, new: f64, kind: Option<EntityKind>) {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        let mut changes = Vec::new();
        for &id in &doc.selection {
            let Some(old) = doc.session.document().get(id) else {
                continue;
            };
            if kind.is_some_and(|k| k != old.kind()) {
                continue;
            }
            // Rows the object does not have, or cannot edit, leave it alone.
            if !properties::rows(old)
                .iter()
                .any(|r| r.field == field && r.editable)
            {
                continue;
            }
            match properties::with_value(old, field, new) {
                Some(next) if next != *old => changes.push((id, next)),
                Some(_) => {}
                None => {
                    self.message = format!("{label} must be a number above zero");
                    return;
                }
            }
        }
        if changes.is_empty() {
            return;
        }
        self.message = format!(
            "{label} set to {}",
            properties::format(doc.session.document().units(), field.unit(), new)
        );
        doc.session
            .perform(ModifyEntities::new(format!("Change {label}"), changes));
    }

    /// Select what a finished region (box, polygon or fence) covers.
    pub fn apply_region(&mut self, found: RegionSelect) {
        let hits = od_canvas::select_in(self.session().document(), &found.region);
        self.message = format!(
            "{} {}",
            plural(hits.len(), "object"),
            if found.remove {
                "deselected"
            } else {
                "selected"
            }
        );
        self.cmdline.say(self.message.clone());
        self.select_ids(hits, found.remove);
    }

    pub fn undo(&mut self) -> Option<String> {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        let step = doc.session.undo();
        doc.prune_selection();
        step.map(|s| s.name)
    }

    pub fn redo(&mut self) -> Option<String> {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        let step = doc.session.redo();
        doc.prune_selection();
        step.map(|s| s.name)
    }

    /// Whether Esc has something to back out of: a running command, file work
    /// that is taking long, or a selection.
    pub fn can_cancel(&self) -> bool {
        self.command_running()
            || self.cmdline.sysvar.is_some()
            || self.file_work_stalled()
            || self.docs[self.active].view.has_warm_grips()
            || !self.docs[self.active].selection.is_empty()
    }

    /// Esc: cancel the running command, else a half-typed variable prompt, else
    /// give up on slow file work, else
    /// clear the selection.
    pub fn cancel(&mut self) {
        if self.command_running() {
            self.cancel_command();
        } else if self.end_sysvar_prompt() {
            self.cmdline.say("*Cancel*");
        } else if self.file_work_stalled() {
            self.cancel_file_work();
        } else if self.clear_warm_grips() {
            // Grips picked with Shift: the first Esc only drops those.
        } else {
            self.docs[self.active].selection.clear();
        }
    }

    pub fn selection(&self) -> &BTreeSet<EntityId> {
        &self.docs[self.active].selection
    }

    /// Select every selectable object (not hidden, frozen or locked).
    pub fn select_all(&mut self) {
        let doc = &mut self.docs[self.active];
        doc.selection = doc
            .session
            .document
            .selectable_objects()
            .map(|o| o.id)
            .collect();
    }

    /// Add `ids` to the selection, or remove them when `remove` is set.
    pub fn select_ids(&mut self, ids: impl IntoIterator<Item = EntityId>, remove: bool) {
        let selection = &mut self.docs[self.active].selection;
        for i in ids {
            if remove {
                selection.remove(&i);
            } else {
                selection.insert(i);
            }
        }
    }

    pub fn new_document(&mut self) {
        let title = format!("Drawing{}", self.next_doc_number);
        self.next_doc_number += 1;
        self.abort_command();
        self.push_document(title, Session::default());
    }

    /// Open a tab for `session` and make it the active one.
    pub(crate) fn push_document(&mut self, title: String, session: Session) -> &mut Doc {
        let uid = self.next_doc_uid;
        self.next_doc_uid += 1;
        self.docs.retain(|d| !d.placeholder);
        self.home = false;
        self.docs.push(Doc::new(uid, title, session));
        self.active = self.docs.len() - 1;
        &mut self.docs[self.active]
    }
}

/// Helpers for tests that need a drawing with something in it.
#[cfg(test)]
mod test_support {
    use od_commands::AddEntity;
    use od_core::Entity;

    use super::AppState;

    impl AppState {
        /// Add a finished entity to the active drawing (undoable).
        pub fn add_entity(&mut self, entity: Entity) {
            self.message = format!("Added {}", entity.kind().display_name());
            self.perform(AddEntity::new(entity));
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::Entity;

    use super::*;

    #[test]
    fn starts_with_one_document() {
        let state = AppState::new();
        assert_eq!(state.docs.len(), 1);
        assert_eq!(state.docs[0].title, "Drawing1");
    }

    #[test]
    fn new_documents_get_increasing_numbers_and_become_active() {
        let mut state = AppState::new();
        state.new_document();
        assert_eq!(state.docs[1].title, "Drawing2");
        assert_eq!(state.active, 1);
    }

    #[test]
    fn closing_the_last_document_leaves_the_home_page() {
        let mut state = AppState::new();
        state.close_document(0);
        assert!(!state.has_drawings());
        assert!(state.home);
        assert_eq!(
            state.docs.len(),
            1,
            "a placeholder keeps the app pointing somewhere"
        );
    }

    #[test]
    fn closing_keeps_active_in_range() {
        let mut state = AppState::new();
        state.new_document();
        state.new_document();
        state.close_document(2);
        assert_eq!(state.active, 1);
        assert_eq!(state.docs.len(), 2);
    }

    #[test]
    fn click_select_is_sticky_and_shift_removes() {
        let mut state = AppState::new();
        let (a, b) = (EntityId(2), EntityId(5));
        state.click_select(Some(a), false);
        state.click_select(Some(b), false);
        assert_eq!(
            state.selection().iter().copied().collect::<Vec<_>>(),
            [a, b]
        );
        // Clicking a selected entity again keeps it; empty space is a no-op.
        state.click_select(Some(a), false);
        state.click_select(None, false);
        assert_eq!(state.selection().len(), 2);
        state.click_select(Some(a), true);
        assert_eq!(state.selection().iter().copied().collect::<Vec<_>>(), [b]);
        state.click_select(None, true);
        assert_eq!(state.selection().len(), 1);
    }

    fn state_with_circles(radii: &[f64]) -> AppState {
        let mut state = AppState::new();
        for &radius in radii {
            state.add_entity(Entity::Circle {
                center: Point::new(0.0, 0.0),
                radius,
            });
        }
        state
    }

    /// Ids of the objects at the given draw-order positions.
    fn ids(state: &AppState, positions: &[usize]) -> Vec<EntityId> {
        let objects = state.session().document().objects();
        positions.iter().map(|&i| objects[i].id).collect()
    }

    fn radii(state: &AppState) -> Vec<f64> {
        state
            .session()
            .document
            .objects()
            .iter()
            .map(|o| properties::value(&o.entity, Field::Radius))
            .collect()
    }

    #[test]
    fn set_property_edits_all_selected_and_undoes_in_one_step() {
        let mut state = state_with_circles(&[1.0, 2.0, 3.0]);
        let picked = ids(&state, &[0, 2]);
        state.select_ids(picked, false);
        state.set_property(Field::Radius, "Radius", 7.0, None);
        assert_eq!(radii(&state), [7.0, 2.0, 7.0]);
        // The selection survives an edit.
        assert_eq!(state.selection().len(), 2);
        state.undo();
        assert_eq!(radii(&state), [1.0, 2.0, 3.0]);
        state.redo();
        assert_eq!(radii(&state), [7.0, 2.0, 7.0]);
    }

    #[test]
    fn set_property_kind_filter_leaves_other_kinds_alone() {
        let mut state = state_with_circles(&[1.0, 2.0]);
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        });
        state.select_all();
        state.set_property(Field::Radius, "Radius", 9.0, Some(EntityKind::Circle));
        let objects = state.session().document().objects();
        assert_eq!(properties::value(&objects[0].entity, Field::Radius), 9.0);
        assert_eq!(properties::value(&objects[1].entity, Field::Radius), 9.0);
        assert_eq!(objects[2].entity.kind(), EntityKind::Line);
    }

    #[test]
    fn set_property_rejects_bad_values_and_skips_no_ops() {
        let mut state = state_with_circles(&[1.0]);
        let picked = ids(&state, &[0]);
        state.select_ids(picked, false);
        state.set_property(Field::Radius, "Radius", -2.0, None);
        assert_eq!(radii(&state), [1.0]);
        assert!(state.message.contains("above zero"));
        // Setting the current value adds no undo step.
        state.set_property(Field::Radius, "Radius", 1.0, None);
        state.undo(); // undoes the circle itself
        assert!(state.session().document().is_empty());
    }

    #[test]
    fn select_ids_adds_and_shift_removes() {
        let mut state = AppState::new();
        let id = EntityId;
        state.select_ids([id(1), id(2), id(3)], false);
        state.select_ids([id(2), id(9)], true);
        assert_eq!(
            state.selection().iter().copied().collect::<Vec<_>>(),
            [id(1), id(3)]
        );
    }

    #[test]
    fn selection_survives_undo_and_redo_of_edits_and_drops_erased_objects() {
        let mut state = state_with_circles(&[1.0, 2.0]);
        state.select_all();
        state.set_property(Field::Radius, "Radius", 5.0, None);
        state.undo();
        assert_eq!(
            state.selection().len(),
            2,
            "undoing an edit keeps the selection"
        );
        state.redo();
        assert_eq!(state.selection().len(), 2);

        // Undoing the second circle's creation drops only that one.
        state.undo(); // radius edit
        state.undo(); // second circle
        assert_eq!(state.selection().len(), 1);
        state.redo();
        assert_eq!(state.selection().len(), 1, "redo does not re-select");
    }
}
