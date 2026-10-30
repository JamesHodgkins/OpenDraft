//! The bottom of the panel: what the assistant is doing, then a rounded box
//! with the text field and, under it, the options and the send button (which
//! is Stop while a task runs). With no key, the box asks for one instead.

use std::sync::Arc;

use eframe::egui;
use od_ui::icon_set::Icon;
use od_ui::{icons, Theme};

use crate::assistant::{ApprovalLevel, Assistant, Drawings, Kind};
use crate::automation::AutomationCore;
use crate::state::AppState;

const INPUT_ID: &str = "assistant_input";
const KEY_ID: &str = "assistant_key";

pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    assistant: &mut Assistant,
    core: &Arc<AutomationCore>,
    state: &AppState,
) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            status(ui, theme, assistant);
            if assistant.has_key() {
                input(ui, theme, assistant, core, state);
            } else {
                key_card(ui, theme, assistant);
            }
        });
}

fn status(ui: &mut egui::Ui, theme: &Theme, assistant: &Assistant) {
    let status = &assistant.chat().status;
    if status.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::Spinner::new()
                .size(theme.font_size_small)
                .color(theme.colors.text_weak),
        );
        ui.label(
            egui::RichText::new(status)
                .size(theme.font_size_small)
                .italics()
                .color(theme.colors.text_weak),
        );
    });
}

/// The rounded box every part of the composer sits in.
fn boxed(ui: &mut egui::Ui, theme: &Theme, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme.colors.input_bg)
        .stroke(egui::Stroke::new(1.0_f32, theme.colors.border))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn input(
    ui: &mut egui::Ui,
    theme: &Theme,
    assistant: &mut Assistant,
    core: &Arc<AutomationCore>,
    state: &AppState,
) {
    let running = assistant.chat().is_running();
    let id = egui::Id::new(INPUT_ID);
    let mut send = false;
    boxed(ui, theme, |ui| {
        // Enter sends; Shift+Enter makes a new line. Taken before the field
        // sees it, and only while the field has the keyboard.
        if ui.memory(|m| m.has_focus(id)) {
            ui.input_mut(|i| {
                if !i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                    send = true;
                }
            });
        }
        ui.add(
            egui::TextEdit::multiline(&mut assistant.draft)
                .id(id)
                .hint_text("Ask about this drawing...")
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .frame(false),
        );
        ui.horizontal(|ui| {
            options(ui, theme, assistant);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let empty = assistant.draft.trim().is_empty();
                if round_button(ui, theme, running, empty && !running) {
                    if running {
                        assistant.stop();
                    } else {
                        send = true;
                    }
                }
            });
        });
        // How much to ask about has a row of its own, so the row above keeps
        // room for the send button in a narrow panel.
        if assistant.kind == Kind::Edit {
            ui.horizontal(|ui| level(ui, theme, assistant));
        }
    });
    if send && !running {
        let text = std::mem::take(&mut assistant.draft);
        if assistant.send(&text, state, core) {
            ui.memory_mut(|m| m.request_focus(id));
        } else {
            assistant.draft = text;
        }
    }
}

/// The row under the text: which drawing, what kind of help, how much to
/// ask about. Only what works is offered.
fn options(ui: &mut egui::Ui, theme: &Theme, assistant: &mut Assistant) {
    let small = theme.font_size_small;
    let weak = theme.colors.text_weak;
    ui.spacing_mut().item_spacing.x = 4.0;
    let drawings = match assistant.drawings {
        Drawings::Current => "Current drawing",
        Drawings::All => "All drawings",
    };
    chip(ui, drawings, small, weak, |ui| {
        if ui
            .selectable_label(assistant.drawings == Drawings::Current, "Current drawing")
            .clicked()
        {
            assistant.drawings = Drawings::Current;
            ui.close_menu();
        }
        if ui
            .selectable_label(assistant.drawings == Drawings::All, "All open drawings")
            .on_hover_text("Lets the assistant read and change every drawing you have open")
            .clicked()
        {
            assistant.drawings = Drawings::All;
            ui.close_menu();
        }
    });
    chip(ui, "Key", small, weak, |ui| {
        if ui
            .button("Forget this key")
            .on_hover_text("Removes it from this session and from the computer's credential store")
            .clicked()
        {
            assistant.forget_key();
            ui.close_menu();
        }
    });
    let kind = match assistant.kind {
        Kind::Review => "Review",
        Kind::Edit => "Edit",
    };
    chip(ui, kind, small, weak, |ui| {
        if ui
            .selectable_label(assistant.kind == Kind::Review, "Review: reads and reports")
            .clicked()
        {
            assistant.kind = Kind::Review;
            ui.close_menu();
        }
        if ui
            .selectable_label(assistant.kind == Kind::Edit, "Edit: plans, then changes")
            .clicked()
        {
            assistant.kind = Kind::Edit;
            ui.close_menu();
        }
    });
}

/// The approval level, shown when the assistant may change things.
fn level(ui: &mut egui::Ui, theme: &Theme, assistant: &mut Assistant) {
    let small = theme.font_size_small;
    let weak = theme.colors.text_weak;
    ui.spacing_mut().item_spacing.x = 4.0;
    let label = match assistant.level {
        ApprovalLevel::ReviewEach => "Review each change",
        ApprovalLevel::ApprovePlan => "Approve the plan",
        ApprovalLevel::AutoApprove { .. } => "Auto-approve",
    };
    chip(ui, label, small, weak, |ui| {
        let levels = [
            (ApprovalLevel::ReviewEach, "Review each change"),
            (ApprovalLevel::ApprovePlan, "Approve the plan"),
            (
                ApprovalLevel::AutoApprove { limit: 5 },
                "Auto-approve (up to 5)",
            ),
        ];
        for (value, name) in levels {
            if ui
                .selectable_label(assistant.level == value, name)
                .clicked()
            {
                assistant.level = value;
                ui.close_menu();
            }
        }
    });
}

/// A small flat dropdown.
fn chip(
    ui: &mut egui::Ui,
    text: &str,
    size: f32,
    colour: egui::Color32,
    menu: impl FnOnce(&mut egui::Ui),
) {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        widgets.inactive.bg_stroke = egui::Stroke::NONE;
        let mut label = egui::text::LayoutJob::default();
        label.append(
            text,
            0.0,
            egui::text::TextFormat {
                font_id: egui::FontId::proportional(size),
                color: colour,
                ..Default::default()
            },
        );
        label.append(
            &od_ui::icons::glyph(icons::ASSISTANT_MENU).to_string(),
            3.0,
            egui::text::TextFormat {
                font_id: egui::FontId::new(size, od_ui::icon_family()),
                color: colour,
                ..Default::default()
            },
        );
        ui.menu_button(label, menu);
    });
}

/// The round send button, or Stop while a task runs. `true` when clicked.
fn round_button(ui: &mut egui::Ui, theme: &Theme, running: bool, dim: bool) -> bool {
    let c = &theme.colors;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    let fill = if dim {
        c.panel_raised
    } else if response.hovered() {
        c.accent_hover
    } else {
        c.accent
    };
    ui.painter().circle_filled(rect.center(), 13.0, fill);
    let (icon, tip) = if running {
        (icons::ASSISTANT_STOP, "Stop")
    } else {
        (icons::ASSISTANT_SEND, "Send (Enter)")
    };
    let colour = if dim {
        c.text_disabled
    } else {
        c.text_on_accent
    };
    ui.put(
        rect.shrink(6.0),
        Icon::new(icon, 14.0).padding(0.0).color(colour),
    );
    response
        .on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
        && !dim
}

/// Where a key goes when there is none: used for the session, and kept in
/// the computer's credential store if the box is ticked.
fn key_card(ui: &mut egui::Ui, theme: &Theme, assistant: &mut Assistant) {
    let id = egui::Id::new(KEY_ID);
    let small = theme.font_size_small;
    boxed(ui, theme, |ui| {
        ui.label(
            egui::RichText::new("Add your Mistral key to start")
                .strong()
                .color(theme.colors.text),
        );
        ui.label(
            egui::RichText::new(
                "It is used until you close OpenDraft. Tick the box to keep it in this                  computer's credential store (never in a file of ours) so you are not asked again.",
            )
            .size(small)
            .color(theme.colors.text_weak),
        );
        let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
        let edit = ui.add(
            egui::TextEdit::singleline(&mut text)
                .password(true)
                .hint_text("Paste the key")
                .desired_width(f32::INFINITY),
        );
        let remember_id = id.with("remember");
        let mut remember: bool = ui.data_mut(|d| d.get_temp(remember_id).unwrap_or(true));
        ui.checkbox(&mut remember, "Remember on this computer");
        ui.data_mut(|d| d.insert_temp(remember_id, remember));
        let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let clicked = ui
            .add_enabled(!text.trim().is_empty(), egui::Button::new("Use this key"))
            .clicked();
        if (submitted || clicked) && !text.trim().is_empty() {
            assistant.set_key(&text, remember);
            text.clear();
        }
        if let Some(note) = &assistant.key_note {
            ui.label(
                egui::RichText::new(note)
                    .size(small)
                    .color(theme.colors.text_weak),
            );
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    });
}
