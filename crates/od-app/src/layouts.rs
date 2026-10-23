//! Layouts in the app: which space a drawing is being edited in, and the
//! switches between the model tab, a sheet, and a viewport on it.
//!
//! Switching spaces never moves an object. In a viewport the commands edit
//! model space as usual (the canvas maps the pointer through the viewport);
//! on the bare sheet they edit paper space, whose objects are the ones tagged
//! with that layout. The document's active space (`set_active_layout`) is set
//! here, whenever the view's space changes.

use od_canvas::Space;
use od_commands::{AddCopies, RemoveEntities, SetLayouts};
use od_core::{LayerId, Layout, LayoutId, Viewport, ViewportId, MIN_SHEET, PAPER_SIZES};

use crate::state::{AppState, Doc};

/// The layouts with one viewport changed, or `None` when it is not there.
pub fn with_viewport(
    layouts: &[Layout],
    layout: LayoutId,
    viewport: ViewportId,
    change: impl FnOnce(&mut Viewport),
) -> Option<Vec<Layout>> {
    let mut layouts = layouts.to_vec();
    let vp = layouts
        .iter_mut()
        .find(|l| l.id == layout)?
        .viewports
        .iter_mut()
        .find(|v| v.id == viewport)?;
    change(vp);
    Some(layouts)
}

/// The Page Setup dialog's working copy of a layout's name and sheet size.
#[derive(Clone, Debug, PartialEq)]
pub struct PageDraft {
    pub layout: LayoutId,
    pub name: String,
    pub width: f64,
    pub height: f64,
}

impl Doc {
    /// Change the space being edited: the view, the document's active space
    /// (where new objects go), and nothing stays selected.
    pub(crate) fn set_space(&mut self, space: Space) {
        self.selection.clear();
        self.dynamic_input.clear();
        self.view.set_space(space);
        self.sync_active_space();
    }

    /// Tell the document which space the view is in, and which layers the
    /// viewport being worked through has frozen. Run every frame, so an undo or
    /// a VPLAYER is seen at once.
    fn sync_active_space(&mut self) {
        let document = self.session.document();
        let (layout, frozen) = match self.view.space() {
            Space::Model => (None, Vec::new()),
            Space::Paper(l) => (Some(l), Vec::new()),
            Space::Viewport(l, v) => {
                let frozen = document
                    .layout(l)
                    .and_then(|layout| layout.viewport(v))
                    .map(|vp| vp.frozen.clone())
                    .unwrap_or_default();
                (None, frozen)
            }
        };
        self.session.set_active_space(layout, frozen);
    }

    /// Leave a space whose layout or viewport no longer exists (an undo
    /// removed it, or it was deleted), and keep the document's active space
    /// in step with the view.
    pub(crate) fn keep_space_valid(&mut self) {
        let document = &self.session.document();
        let current = self.view.space();
        let valid = match current {
            Space::Model => Space::Model,
            Space::Paper(l) if document.layout(l).is_none() => Space::Model,
            Space::Paper(l) => Space::Paper(l),
            Space::Viewport(l, v) => match document.layout(l) {
                None => Space::Model,
                Some(layout) if layout.viewport(v).is_none() => Space::Paper(l),
                Some(_) => current,
            },
        };
        if valid == current {
            self.sync_active_space();
        } else {
            self.set_space(valid);
        }
    }
}

impl AppState {
    pub fn space(&self) -> Space {
        self.docs[self.active].view.space()
    }

    /// The viewport VPSCALE and VPLOCK act on: the active one, or the one
    /// picked on the sheet.
    pub fn target_viewport(&self) -> Option<(LayoutId, ViewportId)> {
        match self.space() {
            Space::Viewport(l, v) => Some((l, v)),
            Space::Paper(l) => Some((l, self.docs[self.active].view.selected_viewport()?)),
            Space::Model => None,
        }
    }

    /// A viewport is picked on the sheet (Erase would remove it).
    pub fn viewport_picked(&self) -> bool {
        matches!(self.space(), Space::Paper(_))
            && self.docs[self.active].view.selected_viewport().is_some()
    }

    fn switch_space(&mut self, space: Space) {
        self.interrupt_command();
        self.docs[self.active].set_space(space);
    }

    pub fn enter_model(&mut self) {
        self.switch_space(Space::Model);
    }

    pub fn enter_layout(&mut self, id: LayoutId) {
        if self.session().document().layout(id).is_some() {
            self.switch_space(Space::Paper(id));
        }
    }

    /// MSPACE: work in model space through a viewport (the one picked, else the top one).
    pub fn mspace(&mut self) {
        let Space::Paper(id) = self.space() else {
            self.message = "MSPACE works from a layout's paper space".to_owned();
            return;
        };
        let picked = self.docs[self.active].view.selected_viewport();
        let layout = self.session().document().layout(id);
        let usable = |v: &ViewportId| {
            layout
                .and_then(|l| l.viewport(*v))
                .is_some_and(|v| v.on && v.is_valid())
        };
        let target = picked
            .filter(usable)
            .or_else(|| layout.and_then(Layout::default_viewport));
        match target {
            Some(v) => self.switch_space(Space::Viewport(id, v)),
            None => self.message = "This layout has no viewport to enter; use MVIEW".to_owned(),
        }
    }

    pub fn pspace(&mut self) {
        match self.space() {
            Space::Viewport(l, _) => self.switch_space(Space::Paper(l)),
            _ => self.message = "PSPACE works from inside a viewport".to_owned(),
        }
    }

    /// Double-click on a viewport of the sheet.
    pub fn enter_viewport(&mut self, viewport: ViewportId) {
        if let Space::Paper(l) = self.space() {
            self.switch_space(Space::Viewport(l, viewport));
        }
    }

    /// Double-click outside the active viewport.
    pub fn leave_viewport(&mut self) {
        self.pspace();
    }

    /// Open the Page Setup dialog on the layout being worked in.
    pub fn open_page_setup(&mut self) {
        let Some(layout) = self
            .space()
            .layout()
            .and_then(|l| self.session().document().layout(l))
        else {
            self.message = "PAGESETUP works in a layout".to_owned();
            return;
        };
        self.page_setup = Some(PageDraft {
            layout: layout.id,
            name: layout.name.clone(),
            width: layout.width,
            height: layout.height,
        });
    }

    /// Rename a layout and resize its sheet as one undo step. Viewports keep
    /// their places on the sheet. Says what is wrong instead of changing anything.
    pub fn set_page_setup(&mut self, draft: &PageDraft) -> Result<(), String> {
        let document = &self.session().document();
        let name = draft.name.trim();
        if name.is_empty() {
            return Err("A layout needs a name.".to_owned());
        }
        if document.layout_name_taken(name, Some(draft.layout)) {
            return Err(format!("A layout called \"{name}\" already exists."));
        }
        let size_ok = |v: f64| v.is_finite() && v >= MIN_SHEET;
        if !size_ok(draft.width) || !size_ok(draft.height) {
            return Err(format!(
                "The sheet must be at least {MIN_SHEET} mm each way."
            ));
        }
        let mut layouts = document.layouts().to_vec();
        let layout = layouts
            .iter_mut()
            .find(|l| l.id == draft.layout)
            .ok_or("The layout is gone.")?;
        if (layout.name.as_str(), layout.width, layout.height) == (name, draft.width, draft.height)
        {
            return Ok(());
        }
        layout.name = name.to_owned();
        layout.width = draft.width;
        layout.height = draft.height;
        self.perform(SetLayouts::new("Page setup", layouts));
        self.docs[self.active].view.request_zoom_extents();
        self.message = "Page setup changed".to_owned();
        Ok(())
    }

    /// Add a layout the size of the first paper size, and switch to it.
    pub fn new_layout(&mut self) {
        if self.in_block_editor() {
            self.message =
                "Layouts are not part of a block. Close the Block Editor first.".to_owned();
            return;
        }
        let document = &self.session().document();
        let (_, width, height) = PAPER_SIZES[0];
        let layout = document.new_layout(document.unused_layout_name(), width, height);
        let id = layout.id;
        self.add_layout(layout, "New layout");
        self.enter_layout(id);
    }

    pub fn duplicate_layout(&mut self, id: LayoutId) {
        let document = &self.session().document();
        let name = document.unused_layout_name();
        if let Some(copy) = document.copied_layout(id, name) {
            let copy_id = copy.id;
            let drawn = document.copied_sheet_objects(id);
            self.interrupt_command();
            let session = &mut self.docs[self.active].session;
            let _ = session.transaction("Copy layout", |tx| {
                let mut layouts = tx.document().layouts().to_vec();
                layouts.push(copy);
                tx.perform(SetLayouts::new("Copy layout", layouts));
                tx.perform(AddCopies::new("Copy layout", drawn).in_layout(Some(copy_id)));
                Ok::<(), ()>(())
            });
            self.enter_layout(copy_id);
        }
    }

    fn add_layout(&mut self, layout: Layout, name: &str) {
        let mut layouts = self.session().document().layouts().to_vec();
        layouts.push(layout);
        self.perform(SetLayouts::new(name, layouts));
    }

    pub fn delete_layout(&mut self, id: LayoutId) {
        self.interrupt_command();
        let session = &mut self.docs[self.active].session;
        let _ = session.transaction("Delete layout", |tx| {
            let drawn = tx.document().objects_of_layout(id);
            let mut layouts = tx.document().layouts().to_vec();
            layouts.retain(|l| l.id != id);
            tx.perform(RemoveEntities::new(drawn));
            tx.perform(SetLayouts::new("Delete layout", layouts));
            Ok::<(), ()>(())
        });
        self.docs[self.active].keep_space_valid();
    }

    /// Store a viewport's view after panning or zooming it. Like AutoCAD's
    /// zoom this is not an undo step.
    pub fn apply_viewport_edit(&mut self, viewport: Viewport) {
        let Space::Viewport(layout, _) = self.space() else {
            return;
        };
        let session = &mut self.docs[self.active].session;
        if let Some(layouts) =
            with_viewport(session.document().layouts(), layout, viewport.id, |v| {
                *v = viewport.clone();
            })
        {
            session.store_view_layouts(layouts);
        }
    }

    /// The viewport the Layers panel's VP Freeze column acts on, if any.
    pub fn target_viewport_ref(&self) -> Option<&Viewport> {
        let (layout, viewport) = self.target_viewport()?;
        self.session().document().layout(layout)?.viewport(viewport)
    }

    /// Freeze a layer in the target viewport, or thaw it if it is frozen there.
    pub fn toggle_layer_in_viewport(&mut self, layer: LayerId) {
        let Some((layout, viewport)) = self.target_viewport() else {
            self.message = "Pick a viewport, or enter one with MSPACE".to_owned();
            return;
        };
        let layouts = self.session().document().layouts();
        let frozen = layouts
            .iter()
            .find(|l| l.id == layout)
            .and_then(|l| l.viewport(viewport))
            .is_some_and(|v| v.is_frozen(layer));
        let changed = with_viewport(layouts, layout, viewport, |v| {
            v.set_frozen(&[layer], !frozen)
        });
        if let Some(layouts) = changed {
            let name = if frozen {
                "Thaw layer in viewport"
            } else {
                "Freeze layer in viewport"
            };
            self.perform(SetLayouts::new(name, layouts));
        }
    }

    /// Lock or unlock the target viewport (VPLOCK).
    pub fn toggle_viewport_lock(&mut self) {
        let Some((layout, viewport)) = self.target_viewport() else {
            self.message = "Pick a viewport, or enter one with MSPACE".to_owned();
            return;
        };
        let layouts = self.session().document().layouts();
        let locked = layouts
            .iter()
            .find(|l| l.id == layout)
            .and_then(|l| l.viewport(viewport))
            .is_some_and(|v| v.locked);
        if let Some(layouts) = with_viewport(layouts, layout, viewport, |v| v.locked = !locked) {
            self.perform(SetLayouts::new(
                if locked {
                    "Unlock viewport"
                } else {
                    "Lock viewport"
                },
                layouts,
            ));
            self.message = format!("Viewport {}", if locked { "unlocked" } else { "locked" });
        }
    }

    /// Erase in paper space removes the picked viewport.
    pub fn erase_viewport(&mut self) -> bool {
        let Space::Paper(layout) = self.space() else {
            return false;
        };
        let Some(viewport) = self.docs[self.active].view.selected_viewport() else {
            return false;
        };
        let mut layouts = self.session().document().layouts().to_vec();
        if let Some(l) = layouts.iter_mut().find(|l| l.id == layout) {
            l.viewports.retain(|v| v.id != viewport);
        }
        self.perform(SetLayouts::new("Erase viewport", layouts));
        self.docs[self.active].view.clear_selected_viewport();
        self.message = "Erased 1 viewport".to_owned();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::{Entity, Point};

    fn state_with_layout() -> (AppState, LayoutId) {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(10000.0, 5000.0),
        });
        state.new_layout();
        let Space::Paper(id) = state.space() else {
            panic!("a new layout is entered");
        };
        (state, id)
    }

    #[test]
    fn a_new_layout_is_entered_and_frames_the_drawing() {
        let (state, id) = state_with_layout();
        let layout = state.session().document().layout(id).unwrap();
        assert_eq!(layout.name, "Layout1");
        let vp = &layout.viewports[0];
        let shown = vp.model_bounds();
        assert!(shown.min.x <= 0.0 && shown.max.x >= 10000.0, "{shown:?}");
    }

    #[test]
    fn mspace_and_pspace_switch_without_touching_the_drawing() {
        let (mut state, id) = state_with_layout();
        let before = state.session().document().objects().to_vec();
        state.mspace();
        assert!(matches!(state.space(), Space::Viewport(l, _) if l == id));
        state.pspace();
        assert_eq!(state.space(), Space::Paper(id));
        state.enter_model();
        assert_eq!(state.space(), Space::Model);
        assert_eq!(state.session().document().objects(), before.as_slice());
    }

    #[test]
    fn objects_drawn_on_the_sheet_stay_on_the_sheet() {
        let (mut state, id) = state_with_layout();
        let line = Entity::Line {
            start: Point::new(5.0, 5.0),
            end: Point::new(50.0, 5.0),
        };
        state.add_entity(line.clone());
        let document = &state.session().document();
        assert_eq!(document.objects_of_layout(id).len(), 1);
        assert_eq!(document.visible_objects().count(), 1, "the sheet's own");
        assert_eq!(document.visible_in(None).count(), 1, "only the model line");
        state.mspace();
        assert_eq!(state.session().document().visible_objects().count(), 1);
        assert_eq!(
            state.session().document().model_bounds().unwrap().max.x,
            10000.0
        );
        state.enter_model();
        state.undo();
        assert!(state.session().document().objects_of_layout(id).is_empty());
    }

    #[test]
    fn deleting_a_layout_takes_its_objects_and_undo_brings_them_back() {
        let (mut state, id) = state_with_layout();
        state.add_entity(Entity::Line {
            start: Point::new(1.0, 1.0),
            end: Point::new(2.0, 2.0),
        });
        state.delete_layout(id);
        assert_eq!(state.session().document().len(), 1, "only the model line");
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert_eq!(state.session().document().objects_of_layout(id).len(), 1);
    }

    #[test]
    fn copying_a_layout_copies_what_is_drawn_on_it() {
        let (mut state, id) = state_with_layout();
        state.add_entity(Entity::Line {
            start: Point::new(1.0, 1.0),
            end: Point::new(2.0, 2.0),
        });
        state.duplicate_layout(id);
        let Space::Paper(copy) = state.space() else {
            panic!("the copy is entered");
        };
        assert_ne!(copy, id);
        let document = &state.session().document();
        assert_eq!(document.objects_of_layout(copy).len(), 1);
        assert_eq!(document.objects_of_layout(id).len(), 1);
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn deleting_the_open_layout_returns_to_the_model_and_undo_restores_it() {
        let (mut state, id) = state_with_layout();
        state.delete_layout(id);
        assert_eq!(state.space(), Space::Model);
        assert!(state.session().document().layouts().is_empty());
        state.undo();
        assert_eq!(state.session().document().layouts().len(), 1);
    }

    #[test]
    fn undoing_a_layout_while_inside_it_leaves_it() {
        let (mut state, _) = state_with_layout();
        state.mspace();
        state.undo();
        state.docs[state.active].keep_space_valid();
        assert_eq!(state.space(), Space::Model);
    }

    #[test]
    fn page_setup_renames_and_resizes_in_one_undo_step_and_refuses_nonsense() {
        let (mut state, id) = state_with_layout();
        state.open_page_setup();
        let mut draft = state.page_setup.clone().unwrap();
        assert_eq!((draft.width, draft.height), (297.0, 210.0));
        draft.name = "Plan".into();
        draft.width = 420.0;
        draft.height = 297.0;
        assert_eq!(state.set_page_setup(&draft), Ok(()));
        let layout = state.session().document().layout(id).unwrap();
        assert_eq!((layout.name.as_str(), layout.width), ("Plan", 420.0));
        assert_eq!(layout.viewports.len(), 1, "viewports stay where they were");
        for bad in [
            PageDraft {
                name: "  ".into(),
                ..draft.clone()
            },
            PageDraft {
                width: 2.0,
                ..draft.clone()
            },
            PageDraft {
                height: f64::NAN,
                ..draft.clone()
            },
        ] {
            assert!(state.set_page_setup(&bad).is_err());
        }
        state.new_layout();
        let Space::Paper(second) = state.space() else {
            panic!("the new layout is entered");
        };
        let other = PageDraft {
            layout: second,
            name: "plan".into(),
            ..draft.clone()
        };
        assert!(
            state.set_page_setup(&other).is_err(),
            "names are unique ignoring case"
        );
        state.undo();
        state.undo();
        assert_eq!(
            state.session().document().layout(id).unwrap().name,
            "Layout1"
        );
    }

    #[test]
    fn page_setup_needs_a_layout() {
        let mut state = AppState::new();
        state.open_page_setup();
        assert!(state.page_setup.is_none());
    }

    #[test]
    fn the_layers_panel_freezes_a_layer_in_the_target_viewport_only() {
        use crate::layers::LayerEdit;
        let (mut state, id) = state_with_layout();
        assert!(
            state.target_viewport_ref().is_none(),
            "nothing picked on the sheet"
        );
        state.apply_layer_edit(LayerEdit::ToggleInViewport(LayerId::DEFAULT));
        assert!(layouts_frozen(&state, id).is_empty());
        state.mspace();
        assert!(state.target_viewport_ref().is_some());
        state.apply_layer_edit(LayerEdit::ToggleInViewport(LayerId::DEFAULT));
        assert_eq!(layouts_frozen(&state, id), [LayerId::DEFAULT]);
        assert!(
            !state
                .session()
                .document
                .layer(LayerId::DEFAULT)
                .unwrap()
                .frozen
        );
        state.apply_layer_edit(LayerEdit::ToggleInViewport(LayerId::DEFAULT));
        assert!(
            layouts_frozen(&state, id).is_empty(),
            "a second click thaws it"
        );
        state.apply_layer_edit(LayerEdit::ToggleInViewport(LayerId::DEFAULT));
        state.undo();
        assert!(layouts_frozen(&state, id).is_empty());
    }

    fn layouts_frozen(state: &AppState, id: LayoutId) -> Vec<LayerId> {
        state.session().document().layout(id).unwrap().viewports[0]
            .frozen
            .clone()
    }

    #[test]
    fn locking_toggles_and_is_undoable() {
        let (mut state, id) = state_with_layout();
        state.mspace();
        state.toggle_viewport_lock();
        let locked = |s: &AppState| s.session().document().layout(id).unwrap().viewports[0].locked;
        assert!(locked(&state));
        state.undo();
        assert!(!locked(&state));
    }

    #[test]
    fn panning_a_viewport_is_stored_without_an_undo_step() {
        let (mut state, id) = state_with_layout();
        state.mspace();
        let steps = state.session().history().can_undo();
        let mut vp = state.session().document().layout(id).unwrap().viewports[0].clone();
        vp.scale *= 2.0;
        state.apply_viewport_edit(vp.clone());
        assert_eq!(
            state.session().document().layout(id).unwrap().viewports[0],
            vp
        );
        assert_eq!(state.session().history().can_undo(), steps);
    }

    #[test]
    fn erase_removes_the_picked_viewport_only_on_the_sheet() {
        let (mut state, id) = state_with_layout();
        assert!(!state.erase_viewport(), "nothing is picked");
        let viewport = state.session().document().layout(id).unwrap().viewports[0].id;
        state.docs[state.active].view.pick_viewport(viewport);
        assert!(state.erase_viewport());
        assert!(state
            .session()
            .document
            .layout(id)
            .unwrap()
            .viewports
            .is_empty());
        state.undo();
        assert_eq!(
            state
                .session()
                .document()
                .layout(id)
                .unwrap()
                .viewports
                .len(),
            1
        );
    }
}
