//! The Page Setup dialog (PAGESETUP): a layout's name, paper size and
//! orientation. It edits a draft held in `AppState::page_setup`; OK commits it
//! as one undo step, Cancel / Escape / X drop it.

use eframe::egui;
use od_core::{paper_size, paper_size_name, PAPER_SIZES};
use od_ui::widgets;

use super::OpenDraftApp;
use crate::layouts::PageDraft;

const WIDTH: f32 = 380.0;
const CUSTOM: &str = "Custom";

enum Choice {
    Ok,
    Cancel,
}

impl OpenDraftApp {
    /// The window opened by `layout.page_setup`.
    pub(super) fn page_setup_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.state.page_setup.take() else {
            return;
        };
        let theme = &self.theme;
        let (mut open, mut choice) = (true, None);
        let error_id = egui::Id::new("page_setup_error");
        let error: Option<String> = ctx.data(|d| d.get_temp(error_id));
        widgets::dialog(ctx, theme, "Page Setup", &mut open, WIDTH, |ui| {
            widgets::field_label(ui, theme, "Layout name:");
            widgets::text_field(ui, theme, &mut draft.name, "Layout name");
            ui.add_space(6.0);
            sheet_box(ui, theme, &mut draft);
            if let Some(why) = &error {
                ui.add_space(6.0);
                ui.colored_label(theme.colors.accent, why);
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, theme, "OK").clicked() {
                    choice = Some(Choice::Ok);
                }
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
        if !open {
            choice = Some(Choice::Cancel);
        }
        match choice {
            Some(Choice::Ok) => match self.state.set_page_setup(&draft) {
                Ok(()) => {
                    ctx.data_mut(|d| d.remove_temp::<String>(error_id));
                }
                Err(why) => {
                    ctx.data_mut(|d| d.insert_temp(error_id, why));
                    self.state.page_setup = Some(draft);
                }
            },
            Some(Choice::Cancel) => {
                ctx.data_mut(|d| d.remove_temp::<String>(error_id));
            }
            None => self.state.page_setup = Some(draft),
        }
    }
}

/// Paper size, orientation and the size in millimetres.
fn sheet_box(ui: &mut egui::Ui, theme: &od_ui::Theme, draft: &mut PageDraft) {
    widgets::group_box(ui, theme, "Paper", |ui| {
        let landscape = draft.width >= draft.height;
        let current = paper_size_name(draft.width, draft.height).unwrap_or(CUSTOM);
        let width = ui.available_width();
        widgets::field_label(ui, theme, "Paper size:");
        widgets::dropdown(ui, theme, "paper_size", current, width, |ui| {
            for (name, ..) in PAPER_SIZES {
                if ui.selectable_label(*name == current, *name).clicked() {
                    if let Some((w, h)) = paper_size(name, landscape) {
                        (draft.width, draft.height) = (w, h);
                    }
                }
            }
            if current == CUSTOM {
                let _ = ui.selectable_label(true, CUSTOM);
            }
        });
        ui.horizontal(|ui| {
            if ui.radio(landscape, "Landscape").clicked() && !landscape {
                std::mem::swap(&mut draft.width, &mut draft.height);
            }
            if ui.radio(!landscape, "Portrait").clicked() && landscape {
                std::mem::swap(&mut draft.width, &mut draft.height);
            }
        });
        ui.horizontal(|ui| {
            widgets::field_label(ui, theme, "Width:");
            widgets::number_field(ui, theme, &mut draft.width, "mm", 1.0);
            widgets::field_label(ui, theme, "Height:");
            widgets::number_field(ui, theme, &mut draft.height, "mm", 1.0);
        });
    });
}
