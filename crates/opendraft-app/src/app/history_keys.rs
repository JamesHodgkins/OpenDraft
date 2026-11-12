//! Up and Down at the command line with no command running: walk the
//! commands run before, as AutoCAD does.

use eframe::egui;

use super::{dock, OpenDraftApp, PanelTab};

impl OpenDraftApp {
    /// Returns true when a key was used. Works whether the canvas or the
    /// command line has focus; another text field keeps its own arrow keys.
    pub(super) fn browse_command_history(&mut self, ctx: &egui::Context) -> bool {
        let input_id = egui::Id::new("command_line_input");
        let other_text_focus = ctx.wants_keyboard_input() && !ctx.memory(|m| m.has_focus(input_id));
        if self.state.home
            || other_text_focus
            || !dock::is_open(&self.dock, PanelTab::CommandLine)
            || self.state.command_running()
            || self.state.mtext_editing()
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || !(self.state.cmdline.input.is_empty() || self.state.cmdline.history_at.is_some())
        {
            return false;
        }
        let line = &mut self.state.cmdline;
        let mut moved = false;
        let mut input = std::mem::take(&mut line.input);
        ctx.input_mut(|i| {
            for (key, older) in [(egui::Key::ArrowUp, true), (egui::Key::ArrowDown, false)] {
                if i.consume_key(egui::Modifiers::NONE, key) {
                    moved |= line.browse_history(older, &mut input);
                }
            }
        });
        line.input = input;
        if moved {
            line.focus = true;
            // The caret goes to the end of the line brought up.
            let end = egui::text::CCursor::new(line.input.chars().count());
            if let Some(mut state) = egui::TextEdit::load_state(ctx, input_id) {
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ctx, input_id);
            }
            ctx.request_repaint();
        }
        moved
    }
}
