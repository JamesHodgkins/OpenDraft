//! Which dialogs are open.

use opendraft_core::block::attribute_draft::{DefinitionDraft, EditorDraft};
use opendraft_core::block::block_draft::BlockDraft;
use opendraft_core::block::FieldDraft;
use opendraft_core::drawing_properties::DrawingProperties;

use crate::state::AppState;

/// The print backend, shared with the threads that talk to it.
#[derive(Clone)]
pub struct PrintHandle(pub std::sync::Arc<dyn crate::print_backend::PrintBackend>);

impl Default for PrintHandle {
    fn default() -> Self {
        Self(std::sync::Arc::from(crate::print_backend::system_backend()))
    }
}

/// What the dialogs a command opens hold while they are open.
#[derive(Default)]
pub struct CommandDialogs {
    /// Where paper plots go; the printers of this machine unless a test says otherwise.
    pub print: PrintHandle,
    pub definition: Option<DefinitionDraft>,
    pub editor: Option<EditorDraft>,
    pub block: Option<BlockDraft>,
    pub field: Option<FieldDraft>,
    /// The Drawing Properties dialog, while it is open.
    pub properties: Option<DrawingProperties>,
    /// The Plot dialog (`PLOT`, `PREVIEW`), while it is open.
    pub plot: Option<crate::plot_job::PlotDialog>,
    /// What finished -PLOT commands asked for.
    pub plot_requests: crate::plot_line::PlotRequests,
}

impl AppState {
    /// A dialog is open and owns the keyboard.
    pub fn modal_open(&self) -> bool {
        self.show_settings
            || self.show_units
            || self.command_dialogs.properties.is_some()
            || self.show_point_style
            || self.power_select.open
            || self.overkill_draft.is_some()
            || self.command_dialogs.definition.is_some()
            || self.command_dialogs.editor.is_some()
            || self.command_dialogs.block.is_some()
            || self.command_dialogs.field.is_some()
            || (self.purge.open && !self.command_running())
            || self.show_linetypes
            || self.show_text_styles
            || self.show_dim_style
            || self.show_dim_manager
            || self.mleader_dialogs.style
            || self.mleader_dialogs.manager
            || self.page_setup.is_some()
            || self
                .command_dialogs
                .plot
                .as_ref()
                .is_some_and(|d| d.picking.is_none())
    }
}
