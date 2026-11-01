//! The Block Editor: BEDIT, BSAVE and BCLOSE.
//!
//! As in AutoCAD, the Block Editor takes over the drawing's tab: the
//! drawing's own session, view and selection are set aside, and the tab
//! edits a drawing that holds only the block's objects, base point at the
//! origin (`Document::block_editor_document`). Every ordinary command, undo
//! and panel works there unchanged. BSAVE writes the block back into the
//! drawing as one undo step of the drawing (`Document::block_edit_save`);
//! BCLOSE puts the drawing back, asking first when there are unsaved
//! changes. File work (save, autosave, the close questions) always uses the
//! drawing's own session (`Doc::drawing`), never the editor's.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use od_canvas::View;
use od_commands::{AddLayer, Cancelled, Editor, ModifyEntities, ModifyLayers, Session, SetBlocks};
use od_core::{BlockId, Entity, EntityId};

use crate::insert_cmd::choose_block;
use crate::state::{AppState, Doc};

/// A block being edited, and the drawing set aside meanwhile.
pub struct BlockEdit {
    pub block: BlockId,
    pub name: String,
    /// The drawing's own session, untouched until a save.
    pub drawing: Session,
    view: View,
    selection: BTreeSet<EntityId>,
    /// Asking whether to save the block's changes before closing.
    pub closing: bool,
}

/// What a command, running on its own thread, asks the app to do with the
/// Block Editor once it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockEditRequest {
    Open(BlockId),
}

pub type BlockEditRequests = Arc<Mutex<Vec<BlockEditRequest>>>;

fn push(requests: &BlockEditRequests, request: BlockEditRequest) {
    if let Ok(mut list) = requests.lock() {
        list.push(request);
    }
}

/// Ask the app to open the Block Editor on `id` when the command ends.
pub(crate) fn request_edit(ed: &Editor, id: BlockId) {
    if let Some(requests) = ed
        .context()
        .get::<std::sync::Mutex<Vec<BlockEditRequest>>>()
    {
        push(&requests, BlockEditRequest::Open(id));
    }
}

impl Doc {
    /// The drawing's own session: the one files are saved from. While a
    /// block is edited, `session` is the Block Editor's.
    pub fn drawing(&self) -> &Session {
        self.block_edit
            .as_ref()
            .map_or(&self.session, |b| &b.drawing)
    }

    pub fn drawing_mut(&mut self) -> &mut Session {
        match self.block_edit.as_mut() {
            Some(edit) => &mut edit.drawing,
            None => &mut self.session,
        }
    }

    /// Leave the Block Editor without saving: the drawing's session, view
    /// and selection come back. Returns the block's name, or `None` when
    /// no block was being edited.
    fn end_block_edit(&mut self) -> Option<String> {
        let BlockEdit {
            name,
            drawing,
            view,
            selection,
            ..
        } = *self.block_edit.take()?;
        self.session = drawing;
        self.view = view;
        self.selection = selection;
        self.prune_selection();
        self.dynamic_input.clear();
        self.mtext_editor = None;
        Some(name)
    }
}

impl AppState {
    /// Whether the active drawing is in the Block Editor.
    pub fn in_block_editor(&self) -> bool {
        self.docs[self.active].block_edit.is_some()
    }

    /// BEDIT: edit the block of the selected reference, or ask which block.
    pub fn bedit(&mut self) {
        if self.in_block_editor() {
            self.message = "The Block Editor is already open.".to_owned();
            return;
        }
        let doc = &self.docs[self.active];
        let picked = doc
            .selection
            .iter()
            .find_map(|id| match doc.session.document().get(*id) {
                Some(Entity::Insert(i)) => Some(i.block.id),
                _ => None,
            });
        if let Some(id) = picked {
            return self.open_block_editor(id);
        }
        let requests = self.block_requests.clone();
        self.start_command("BEDIT", move |ed| {
            if let Some(block) = choose_block(ed)? {
                push(&requests, BlockEditRequest::Open(block.id));
            }
            Ok(())
        });
    }

    /// Open the Block Editor on block `id` in the active drawing.
    pub fn open_block_editor(&mut self, id: BlockId) {
        self.abort_command();
        let doc = &mut self.docs[self.active];
        if doc.block_edit.is_some() {
            return;
        }
        let document = &doc.session.document();
        let (Some(editor), Some(block)) = (document.block_editor_document(id), document.block(id))
        else {
            self.message = "That block is not in this drawing.".to_owned();
            return;
        };
        let name = block.name.clone();
        let drawing = std::mem::replace(&mut doc.session, Session::with_document(editor));
        let view = std::mem::take(&mut doc.view);
        let selection = std::mem::take(&mut doc.selection);
        doc.dynamic_input.clear();
        doc.view.request_zoom_extents();
        doc.block_edit = Some(Box::new(BlockEdit {
            block: id,
            name: name.clone(),
            drawing,
            view,
            selection,
            closing: false,
        }));
        self.tell(format!(
            "Editing block \"{name}\". The origin is its base point. BSAVE saves it, BCLOSE closes the Block Editor."
        ));
    }

    /// BSAVE: write the edited block back into the drawing, as one undo step
    /// of the drawing, and keep editing. False (with a message) when it
    /// cannot be saved.
    pub fn bsave(&mut self) -> bool {
        let doc = &mut self.docs[self.active];
        let Some(edit) = doc.block_edit.as_mut() else {
            return false;
        };
        let save = match edit
            .drawing
            .document()
            .block_edit_save(doc.session.document(), edit.block)
        {
            Ok(save) => save,
            Err(why) => {
                self.tell(why.to_owned());
                return false;
            }
        };
        let step = format!("Save block {}", edit.name);
        let saved = edit.drawing.transaction(&step, move |tx| {
            for layer in save.new_layers {
                tx.perform(AddLayer::new(layer));
            }
            if !save.changed_layers.is_empty() {
                tx.perform(ModifyLayers::new("Layers", save.changed_layers));
            }
            tx.perform(SetBlocks::new("Block", save.blocks));
            if !save.objects.is_empty() {
                tx.perform(ModifyEntities::new("Redefine block", save.objects));
            }
            Ok::<_, Cancelled>(())
        });
        if saved.is_err() {
            return false;
        }
        doc.session.mark_saved();
        let name = edit.name.clone();
        self.tell(format!("Block \"{name}\" saved."));
        true
    }

    /// BCLOSE: leave the Block Editor, asking first whether to save
    /// unsaved changes.
    pub fn bclose(&mut self) {
        let doc = &self.docs[self.active];
        if doc.block_edit.is_none() {
            return;
        }
        if !doc.session.is_modified() {
            return self.close_block_editor(false);
        }
        self.ask_to_save_block(self.active);
    }

    /// Ask, in a dialog, what to do with the block's unsaved changes.
    fn ask_to_save_block(&mut self, index: usize) {
        self.abort_command();
        if let Some(edit) = self.docs.get_mut(index).and_then(|d| d.block_edit.as_mut()) {
            edit.closing = true;
        }
    }

    /// The answer to that question: Save closes the Block Editor after
    /// saving, Discard closes it without, Cancel stays in it.
    pub fn answer_block_close(&mut self, choice: crate::files::Choice) {
        use crate::files::Choice;
        let Some(edit) = self.docs[self.active].block_edit.as_mut() else {
            return;
        };
        edit.closing = false;
        match choice {
            Choice::Save => self.close_block_editor(true),
            Choice::Discard => self.close_block_editor(false),
            Choice::Cancel => {}
        }
    }

    /// Put the drawing back, saving the block first when `save` (and staying
    /// in the Block Editor if that fails).
    pub fn close_block_editor(&mut self, save: bool) {
        if save && !self.bsave() {
            return;
        }
        self.abort_command();
        if let Some(name) = self.docs[self.active].end_block_edit() {
            self.message = format!("Closed the Block Editor for \"{name}\".");
        }
    }

    /// Before drawing `index` closes or the app quits: leave its Block
    /// Editor if nothing there is unsaved. Otherwise switch to it and say
    /// what to do, and return false: the close waits.
    pub(crate) fn leave_block_editor(&mut self, index: usize) -> bool {
        let Some(doc) = self.docs.get_mut(index) else {
            return true;
        };
        let Some(name) = doc.block_edit.as_ref().map(|e| e.name.clone()) else {
            return true;
        };
        if !doc.session.is_modified() {
            if index == self.run_doc {
                self.abort_command();
            }
            self.docs[index].end_block_edit();
            return true;
        }
        self.active = index;
        self.ask_to_save_block(index);
        self.tell(format!(
            "Block \"{name}\" has unsaved changes: save or discard them, then close again."
        ));
        false
    }

    /// Carry out what commands asked of the Block Editor. Called after the
    /// running command is polled.
    pub(crate) fn take_block_requests(&mut self) {
        let requests = match self.block_requests.lock() {
            Ok(mut list) => std::mem::take(&mut *list),
            Err(_) => return,
        };
        for request in requests {
            match request {
                BlockEditRequest::Open(id) => self.open_block_editor(id),
            }
        }
    }
}

#[cfg(test)]
#[path = "block_editor_tests.rs"]
mod tests;
