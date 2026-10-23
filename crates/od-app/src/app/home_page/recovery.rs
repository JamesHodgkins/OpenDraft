//! The Recovered drawings section of the Home page: what a crashed run left
//! behind, one group per crashed session, and what recovering did.

use std::time::SystemTime;

use eframe::egui;
use od_ui::widgets;

use super::Pick;
use crate::autosave::RecoveryReport;
use crate::recovery::{Orphan, Recovered};
use crate::text::{age_text, plural};

const ROW_HEIGHT: f32 = 46.0;

/// The offers, then (if any drawing could not be read) what happened. Nothing
/// at all when there is nothing to say.
pub(super) fn recovery_section(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    orphans: &[Orphan],
    report: &RecoveryReport,
    pick: &mut Option<Pick>,
) {
    if orphans.is_empty() && report.failed.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        widgets::section_header(ui, theme, "Recovered drawings");
        if !orphans.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::secondary_button(ui, theme, "Decide later")
                    .on_hover_text("Ask again next time; nothing is deleted")
                    .clicked()
                {
                    *pick = Some(Pick::DeferRecovery);
                }
            });
        }
    });
    if !orphans.is_empty() {
        ui.label(
            egui::RichText::new(
                "OpenDraft closed unexpectedly. These drawings had unsaved changes.",
            )
            .color(theme.colors.text_weak),
        );
        ui.add_space(6.0);
    }
    for (index, orphan) in orphans.iter().enumerate() {
        ui.push_id(orphan.id(), |ui| session(ui, theme, index, orphan, pick));
        ui.add_space(8.0);
    }
    if !report.failed.is_empty() {
        outcome(ui, theme, report);
    }
    ui.add_space(22.0);
}

/// One crashed session: its drawings, and the three things to do with it.
fn session(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    index: usize,
    orphan: &Orphan,
    pick: &mut Option<Pick>,
) {
    let count = orphan.items.len();
    let confirm_id = ui.id().with("confirm_discard");
    let confirming = ui.data(|d| d.get_temp::<bool>(confirm_id)).unwrap_or(false);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "Recovered session  ·  {}",
                plural(count, "drawing")
            ))
            .strong()
            .color(theme.colors.text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if confirming {
                if widgets::secondary_button(ui, theme, "Keep").clicked() {
                    ui.data_mut(|d| d.insert_temp(confirm_id, false));
                }
                if widgets::primary_button(ui, theme, "Discard").clicked() {
                    ui.data_mut(|d| d.insert_temp(confirm_id, false));
                    *pick = Some(Pick::DiscardSession(index));
                }
                ui.label(
                    egui::RichText::new(format!("Delete {} for good?", plural(count, "drawing")))
                        .color(theme.colors.text_weak),
                );
            } else {
                if widgets::primary_button(ui, theme, "Recover session").clicked() {
                    *pick = Some(Pick::RecoverSession(index));
                }
                if widgets::secondary_button(ui, theme, "Discard session").clicked() {
                    ui.data_mut(|d| d.insert_temp(confirm_id, true));
                }
            }
        });
    });
    for item in &orphan.items {
        item_row(ui, theme, item);
    }
}

/// One drawing of a session: read-only, so not a Tab stop.
fn item_row(ui: &mut egui::Ui, theme: &od_ui::Theme, item: &Recovered) {
    let c = &theme.colors;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    let pad = 12.0;
    let from = item.original.as_ref().map_or_else(
        || "Not saved before the interruption".to_owned(),
        |p| p.display().to_string(),
    );
    let when = saved_text(item.saved);
    let when_width = ui
        .painter()
        .layout_no_wrap(
            when.clone(),
            egui::FontId::proportional(theme.font_size_small),
            c.text_weak,
        )
        .size()
        .x;
    let room = (rect.width() - 2.0 * pad - when_width - 16.0).max(40.0);
    let painter = ui.painter().with_clip_rect(egui::Rect::from_min_size(
        rect.min,
        egui::vec2(room + pad, rect.height()),
    ));
    painter.text(
        egui::pos2(rect.left() + pad, rect.top() + 14.0),
        egui::Align2::LEFT_CENTER,
        &item.title,
        egui::FontId::proportional(theme.font_size_body),
        c.text,
    );
    painter.text(
        egui::pos2(rect.left() + pad, rect.top() + 32.0),
        egui::Align2::LEFT_CENTER,
        from,
        egui::FontId::proportional(theme.font_size_small),
        c.text_weak,
    );
    ui.painter().text(
        egui::pos2(rect.right() - pad, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        when,
        egui::FontId::proportional(theme.font_size_small),
        c.text_weak,
    );
}

/// "23-10-2026 19:40  ·  5 minutes ago", in local time; or that it is not known.
fn saved_text(saved: Option<SystemTime>) -> String {
    let Some(at) = saved else {
        return "Time unavailable".to_owned();
    };
    let local: chrono::DateTime<chrono::Local> = at.into();
    let absolute = local.format("%d-%m-%Y %H:%M");
    match SystemTime::now().duration_since(at) {
        Ok(age) => format!("{absolute}  ·  {}", age_text(age)),
        Err(_) => absolute.to_string(),
    }
}

/// The drawings that could not be read, and the totals.
fn outcome(ui: &mut egui::Ui, theme: &od_ui::Theme, report: &RecoveryReport) {
    let c = &theme.colors;
    ui.label(
        egui::RichText::new(format!(
            "Recovered {}, {} could not be read.",
            plural(report.recovered, "drawing"),
            report.failed.len()
        ))
        .strong()
        .color(c.text),
    );
    for (title, error) in &report.failed {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(title).color(c.text));
            ui.label(egui::RichText::new("Recovery failed").color(c.text_weak));
            ui.label(egui::RichText::new(error).color(c.text_weak));
        });
    }
    ui.label(
        egui::RichText::new(
            "The recovery copies of drawings that could not be read were removed after the attempt.",
        )
        .color(c.text_weak),
    );
}
