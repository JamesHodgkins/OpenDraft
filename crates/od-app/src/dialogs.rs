//! Which dialogs are open.

use crate::state::AppState;

impl AppState {
    /// A dialog is open and owns the keyboard.
    pub fn modal_open(&self) -> bool {
        self.show_settings
            || self.show_units
            || self.show_point_style
            || self.power_select.open
            || self.show_linetypes
            || self.show_text_styles
            || self.show_dim_style
            || self.show_dim_manager
            || self.mleader_dialogs.style
            || self.mleader_dialogs.manager
            || self.page_setup.is_some()
    }
}
