//! Which dialogs are open.

use od_core::block::attribute_draft::{DefinitionDraft, EditorDraft};
use od_core::block::block_draft::BlockDraft;

use crate::state::AppState;

/// What the dialogs a command opens hold while they are open.
#[derive(Default)]
pub struct CommandDialogs {
    pub definition: Option<DefinitionDraft>,
    pub editor: Option<EditorDraft>,
    pub block: Option<BlockDraft>,
}

impl AppState {
    /// A dialog is open and owns the keyboard.
    pub fn modal_open(&self) -> bool {
        self.show_settings
            || self.show_units
            || self.show_point_style
            || self.power_select.open
            || self.overkill_draft.is_some()
            || self.command_dialogs.definition.is_some()
            || self.command_dialogs.editor.is_some()
            || self.command_dialogs.block.is_some()
            || (self.purge.open && !self.command_running())
            || self.show_linetypes
            || self.show_text_styles
            || self.show_dim_style
            || self.show_dim_manager
            || self.mleader_dialogs.style
            || self.mleader_dialogs.manager
            || self.page_setup.is_some()
    }
}
