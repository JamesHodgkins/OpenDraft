//! A lone press of Ctrl, which cycles a grip edit's options.

use eframe::egui;

use super::OpenDraftApp;

impl OpenDraftApp {
    /// Ctrl pressed and released on its own cycles the options of a grip being
    /// edited (Stretch, Lengthen). Ctrl with a key or a click is a shortcut.
    pub(super) fn watch_ctrl(&mut self, ctx: &egui::Context) {
        let (down, other) = ctx.input(|i| {
            let other = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, key, .. } if !matches!(key, egui::Key::Escape)
                ) || matches!(e, egui::Event::PointerButton { pressed: true, .. })
            });
            (i.modifiers.ctrl, other)
        });
        if down && !self.ctrl_down {
            self.ctrl_alone = true;
        }
        if other {
            self.ctrl_alone = false;
        }
        let free = !self.state.home
            && !self.state.mtext_editing()
            && !self.state.modal_open()
            && self.state.confirm.is_none();
        if !down && self.ctrl_down && self.ctrl_alone && free {
            self.state.cycle_grip_option();
        }
        self.ctrl_down = down;
    }
}
