//! The conversation: what you said in a rounded box, what the assistant said
//! as plain text, what it did as dim one-line entries like the command log,
//! and the question it is waiting on as a card with its buttons.

use eframe::egui;
use od_agent::{Answer, PlanOutcome};
use od_ui::Theme;

use super::markdown;
use crate::assistant::describe;
use crate::assistant::{ApprovalLevel, Ask, Assistant, Decision, Entry, Progress, Reply};
use crate::state::AppState;
use crate::text::plural;

pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    assistant: &mut Assistant,
    state: &mut AppState,
    names: &[(String, String)],
) {
    let mut reply = None;
    let mut undo = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let chat = assistant.chat();
                    if chat.entries.is_empty() && chat.pending.is_none() {
                        empty(ui, theme);
                    }
                    for (index, entry) in chat.entries.iter().enumerate() {
                        if entry_ui(ui, theme, entry) {
                            undo = Some(index);
                        }
                    }
                    if let Some(ask) = chat.pending.as_ref().map(|p| p.ask.clone()) {
                        reply = card(ui, theme, &ask, names);
                    }
                });
        });
    if let Some(reply) = reply {
        assistant.answer(reply);
    }
    if let Some(index) = undo {
        assistant.undo(index, state);
    }
}

fn empty(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(
            "Ask about the drawing in view. The assistant can read it, check it for \
             problems and tell you what it finds.",
        )
        .size(theme.font_size_small)
        .color(theme.colors.text_weak),
    );
}

/// One line of the transcript; `true` if its Undo button was pressed.
fn entry_ui(ui: &mut egui::Ui, theme: &Theme, entry: &Entry) -> bool {
    let mut undo = false;
    let c = &theme.colors;
    let small = theme.font_size_small;
    match entry {
        Entry::You(text) => {
            egui::Frame::new()
                .fill(c.panel_raised)
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new(text).color(c.text));
                });
        }
        Entry::Said(text) => {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let job = markdown::job(
                text,
                &font,
                c.text,
                ui.visuals().strong_text_color(),
                ui.available_width(),
            );
            ui.label(job);
        }
        Entry::Did { text, progress } => {
            ui.horizontal(|ui| {
                match progress {
                    Progress::Running => {
                        ui.add(egui::Spinner::new().size(small).color(c.text_weak));
                    }
                    Progress::Done => {
                        ui.label(
                            egui::RichText::new("\u{2022}")
                                .size(small)
                                .color(c.text_weak),
                        );
                    }
                    Progress::Failed => {
                        ui.label(
                            egui::RichText::new("!")
                                .size(small)
                                .strong()
                                .color(ui.visuals().warn_fg_color),
                        );
                    }
                }
                ui.label(egui::RichText::new(text).size(small).color(c.text_weak));
            });
        }
        Entry::Notice(text) => {
            ui.label(
                egui::RichText::new(text)
                    .size(small)
                    .italics()
                    .color(c.text_weak),
            );
        }
        Entry::Made {
            changes, outcome, ..
        } => match outcome {
            Some(said) => {
                ui.label(
                    egui::RichText::new(said)
                        .size(small)
                        .italics()
                        .color(c.text_weak),
                );
            }
            None => {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Made {}.",
                            plural(*changes as usize, "change")
                        ))
                        .size(small)
                        .color(c.text_weak),
                    );
                    if ui
                        .small_button("Undo")
                        .on_hover_text("Take back what this task changed")
                        .clicked()
                    {
                        undo = true;
                    }
                });
            }
        },
    }
    undo
}

/// The question on screen; `Some` once answered.
fn card(ui: &mut egui::Ui, theme: &Theme, ask: &Ask, names: &[(String, String)]) -> Option<Reply> {
    let c = &theme.colors;
    let mut reply = None;
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, c.accent))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            match ask {
                Ask::Change(request) => {
                    ui.label(
                        egui::RichText::new("The assistant wants to change the drawing")
                            .color(c.text_weak),
                    );
                    ui.label(
                        egui::RichText::new(describe::change(request))
                            .strong()
                            .color(c.text),
                    );
                    if let Some(title) = request
                        .arguments
                        .get("document_id")
                        .and_then(|d| d.as_str())
                        .and_then(|id| names.iter().find(|(n, _)| n == id))
                        .filter(|_| names.len() > 1)
                    {
                        ui.label(
                            egui::RichText::new(format!("In {}", title.1))
                                .size(theme.font_size_small)
                                .color(c.text_weak),
                        );
                    }
                    egui::CollapsingHeader::new("Show exactly what will be sent")
                        .id_salt("assistant_change")
                        .show(ui, |ui| {
                            let text = serde_json::to_string_pretty(&request.arguments)
                                .unwrap_or_default();
                            ui.label(
                                egui::RichText::new(text)
                                    .monospace()
                                    .size(theme.font_size_small)
                                    .color(c.text_weak),
                            );
                        });
                    ui.horizontal(|ui| {
                        if ui.button("Allow once").clicked() {
                            reply = Some(Reply::Change(Decision::Approved));
                        }
                        if ui.button("Decline").clicked() {
                            reply = Some(Reply::Change(Decision::Declined));
                        }
                    });
                }
                Ask::Plan(plan, level) => {
                    ui.label(egui::RichText::new("Proposed plan").strong().color(c.text));
                    ui.label(egui::RichText::new(&plan.summary).color(c.text));
                    for (n, step) in plan.steps.iter().enumerate() {
                        ui.label(
                            egui::RichText::new(format!("{}. {step}", n + 1))
                                .size(theme.font_size_small)
                                .color(c.text_weak),
                        );
                    }
                    let allows = match level {
                        ApprovalLevel::ApprovePlan => format!(
                            "Approving lets it make up to {} change(s).",
                            plan.steps.len()
                        ),
                        ApprovalLevel::ReviewEach => {
                            "Approving only lets it go on; each change is still asked.".to_owned()
                        }
                        ApprovalLevel::AutoApprove { .. } => String::new(),
                    };
                    if !allows.is_empty() {
                        ui.label(
                            egui::RichText::new(allows)
                                .size(theme.font_size_small)
                                .italics()
                                .color(c.text_weak),
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Approve").clicked() {
                            reply = Some(Reply::Plan(PlanOutcome::Approved));
                        }
                        if ui.button("Reject").clicked() {
                            reply = Some(Reply::Plan(PlanOutcome::Rejected(None)));
                        }
                    });
                }
                Ask::Question(question) => {
                    ui.label(egui::RichText::new(&question.text).color(c.text));
                    for choice in &question.choices {
                        if ui.button(choice).clicked() {
                            reply = Some(Reply::Answer(Answer::Text(choice.clone())));
                        }
                    }
                    let id = egui::Id::new("assistant_answer");
                    let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text("Or type an answer")
                            .desired_width(f32::INFINITY),
                    );
                    let submitted =
                        edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if submitted && !text.trim().is_empty() {
                        reply = Some(Reply::Answer(Answer::Text(text.trim().to_owned())));
                        text.clear();
                    }
                    ui.data_mut(|d| d.insert_temp(id, text));
                }
            }
        });
    reply
}
