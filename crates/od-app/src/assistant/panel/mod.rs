//! The Assistant panel: a header with the history and new-chat buttons, the
//! transcript above, and below it a status line and the input box with its
//! row of options. It looks like the command line's log (small, quiet text on
//! the panel's own colour) with a chat's composer at the bottom.
//!
//! All state is in [`Assistant`](super::Assistant); this only draws it and
//! turns clicks into calls on it.

mod composer;
mod markdown;
mod transcript;

use std::sync::Arc;

use eframe::egui;
use od_ui::icon_set::Icon;
use od_ui::{icons, Theme};

use super::Assistant;
use crate::automation::AutomationCore;
use crate::state::AppState;

pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    assistant: &mut Assistant,
    core: &Arc<AutomationCore>,
    state: &AppState,
) {
    header(ui, theme, assistant);
    egui::TopBottomPanel::bottom("assistant_composer")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .resizable(false)
        .show_inside(ui, |ui| composer::show(ui, theme, assistant, core, state));
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            if assistant.history_open {
                history(ui, theme, assistant);
            } else {
                transcript::show(ui, theme, assistant);
            }
        });
}

fn header(ui: &mut egui::Ui, theme: &Theme, assistant: &mut Assistant) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let title = if assistant.history_open {
                    "Chat history".to_owned()
                } else if assistant.chat().title.is_empty() {
                    "New chat".to_owned()
                } else {
                    assistant.chat().title.clone()
                };
                // Leave room for the two buttons on the right.
                let room = (ui.available_width() - 56.0).max(40.0);
                ui.scope(|ui| {
                    ui.set_max_width(room);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(title).strong().color(theme.colors.text),
                        )
                        .truncate(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let weak = theme.colors.text_weak;
                    let new_chat = ui
                        .add(
                            Icon::new(icons::ASSISTANT_NEW_CHAT, 15.0)
                                .clickable()
                                .color(weak),
                        )
                        .on_hover_text("New chat")
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if new_chat.clicked() {
                        assistant.new_chat();
                    }
                    let history = ui
                        .add(
                            Icon::new(icons::ASSISTANT_HISTORY, 15.0)
                                .toggle(assistant.history_open)
                                .color(weak),
                        )
                        .on_hover_text("Chat history")
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if history.clicked() {
                        assistant.history_open = !assistant.history_open;
                    }
                });
            });
        });
}

/// The earlier chats of this run, newest first.
fn history(ui: &mut egui::Ui, theme: &Theme, assistant: &mut Assistant) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            let list: Vec<(usize, String)> = assistant
                .history()
                .into_iter()
                .map(|(i, t)| (i, t.to_owned()))
                .collect();
            if list.is_empty() {
                ui.label(
                    egui::RichText::new("No chats yet. They are kept until you close OpenDraft.")
                        .size(theme.font_size_small)
                        .color(theme.colors.text_weak),
                );
                return;
            }
            let current = assistant.current_index();
            let mut chosen = None;
            for (index, title) in list {
                let row = egui::Button::new(title)
                    .selected(index == current)
                    .truncate();
                if ui.add_sized([ui.available_width(), 22.0], row).clicked() {
                    chosen = Some(index);
                }
            }
            if let Some(index) = chosen {
                assistant.select(index);
            }
        });
}
