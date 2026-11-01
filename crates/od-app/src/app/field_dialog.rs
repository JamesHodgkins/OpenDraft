//! The Field dialog FIELD opens. OK hands the field to the command, Select
//! object closes the dialog for a pick (it opens again with the object), and
//! Cancel cancels the command.

use eframe::egui;
use od_commands::{Reply, Wants};

use super::OpenDraftApp;
use crate::field_picker::{show, Chosen};

impl OpenDraftApp {
    pub(super) fn field_dialog_window(&mut self, ctx: &egui::Context) {
        let wanted = self
            .state
            .waiting()
            .filter(|ask| ask.wants == Wants::FieldDialog)
            .map(|ask| ask.field_dialog.clone());
        let Some(start) = wanted else {
            self.state.command_dialogs.field = None;
            return;
        };
        if self.state.command_dialogs.field.is_none() {
            self.state.command_dialogs.field = start.map(|d| *d);
        }
        let Some(mut draft) = self.state.command_dialogs.field.take() else {
            return;
        };
        let context = self.state.field_context();
        let (chosen, _) = show(ctx, &self.theme, &mut draft, &context);
        match chosen {
            Chosen::Open => self.state.command_dialogs.field = Some(draft),
            Chosen::Insert(field) => self.state.reply(Reply::Field(field)),
            Chosen::PickObject => self.state.reply(Reply::FieldPick(Box::new(draft))),
            Chosen::Cancel => self.state.cancel_command(),
        }
    }
}
