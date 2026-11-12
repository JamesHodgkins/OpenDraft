//! Page Setup (PAGESETUP), editing a complete per-layout plot-settings draft.

use eframe::egui;
use opendraft_ui::widgets;

use super::OpenDraftApp;
use crate::layouts::PageDraft;

pub(in crate::app) mod boxes;
mod preview;

use boxes::{left_column, right_column};

const WIDTH: f32 = 1040.0;

enum Choice {
    Ok,
    Cancel,
}

impl OpenDraftApp {
    pub(super) fn page_setup_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.state.page_setup.take() else {
            return;
        };
        let theme = &self.theme;
        let (mut open, mut choice) = (true, None);
        let error_id = egui::Id::new("page_setup_error");
        let error: Option<String> = ctx.data(|d| d.get_temp(error_id));
        widgets::dialog(ctx, theme, "Page Setup", &mut open, WIDTH, |ui| {
            ui.horizontal(|ui| {
                widgets::field_label(ui, theme, "Layout name:");
                ui.add_enabled_ui(
                    draft.space != opendraft_core::plot::PlotSpace::Model,
                    |ui| {
                        widgets::text_field(ui, theme, &mut draft.name, "Layout name");
                    },
                );
            });
            ui.add_space(6.0);
            ui.columns(3, |columns| {
                left_column(&mut columns[0], theme, &mut draft);
                right_column(&mut columns[1], theme, &mut draft);
                preview_column(&mut columns[2], theme, &draft);
            });
            if let Some(why) = &error {
                ui.colored_label(theme.colors.accent, why);
            }
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

fn preview_column(ui: &mut egui::Ui, theme: &opendraft_ui::Theme, draft: &PageDraft) {
    widgets::group_box(ui, theme, "Live preview", |ui| {
        let size = egui::vec2(ui.available_width(), 430.0);
        let (canvas, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 2.0, ui.visuals().extreme_bg_color);
        let (pw, ph) = (draft.width.max(1.0) as f32, draft.height.max(1.0) as f32);
        let scale = ((canvas.width() - 28.0) / pw)
            .min((canvas.height() - 54.0) / ph)
            .max(0.0);
        let paper_size = egui::vec2(pw * scale, ph * scale);
        let paper =
            egui::Rect::from_center_size(canvas.center() - egui::vec2(0.0, 10.0), paper_size);
        painter.rect_filled(paper, 1.0, egui::Color32::from_gray(242));
        painter.rect_stroke(
            paper,
            1.0,
            egui::Stroke::new(1.0_f32, egui::Color32::from_gray(100)),
            egui::StrokeKind::Inside,
        );
        let m = draft.plot.margins_mm;
        let printable = egui::Rect::from_min_max(
            paper.min + egui::vec2(m[0] as f32 * scale, m[3] as f32 * scale),
            paper.max - egui::vec2(m[2] as f32 * scale, m[1] as f32 * scale),
        );
        painter.rect_stroke(
            printable,
            0.0,
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(80, 150, 170)),
            egui::StrokeKind::Inside,
        );
        let content = preview::content_rect(draft, printable, scale);
        painter.rect_filled(
            content.intersect(printable),
            0.0,
            egui::Color32::from_rgba_unmultiplied(30, 155, 125, 65),
        );
        painter.rect_stroke(
            content.intersect(printable),
            0.0,
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(20, 135, 110)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            egui::pos2(canvas.center().x, paper.max.y + 10.0),
            egui::Align2::CENTER_TOP,
            format!("{:.0} × {:.0} mm", draft.width, draft.height),
            egui::FontId::proportional(12.0),
            ui.visuals().text_color(),
        );
        ui.small("White: paper · blue: printable area · green: plotted area");
    });
}
