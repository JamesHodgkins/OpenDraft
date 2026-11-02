//! The Plot dialog (PLOT, PRINT, PREVIEW): the page setup of the layout being
//! worked in, with a picture of exactly what will be written.
//!
//! The picture is the centre of it: a tall preview on the right, and on the
//! left, in the order it is thought of, where it goes, the paper, what is
//! plotted and how big, and the options. A caption and its control share a
//! row, and a control that is not needed is left out rather than greyed.

use eframe::egui;
use od_ui::widgets;

use super::page_setup_dialog::boxes::{options_box, paper_box, plot_box};
use super::OpenDraftApp;
use crate::plot_controls::control_state;
use crate::plot_job::{PlotDialog, Printers, Target};

const WIDTH: f32 = 1040.0;
const LEFT: f32 = 340.0;
const GAP: f32 = 14.0;
/// The fixed picture height. Keeping this independent of the controls in the
/// left column prevents plot-area changes (notably Window's coordinate rows)
/// from resizing both the preview and the dialog.
const PREVIEW_HEIGHT: f32 = 345.0;

enum Action {
    PickWindow,
    Apply,
    Plot,
    Cancel,
}

impl OpenDraftApp {
    pub(super) fn plot_dialog_window(&mut self, ctx: &egui::Context) {
        self.state.take_window_pick();
        let Some(mut dialog) = self.state.command_dialogs.plot.take() else {
            return;
        };
        if dialog.picking.is_some() {
            // Hidden while the drawing asks for the window.
            self.state.command_dialogs.plot = Some(dialog);
            return;
        }
        let theme = &self.theme;
        dialog.printers.poll();
        dialog.settle();
        if matches!(dialog.printers, Printers::Asking(_)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let controls = control_state(&dialog.draft.plot, false);
        self.state.command_dialogs.plot = Some(dialog);
        if self.state.plot_dialog_tick(std::time::Instant::now()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        let Some(mut dialog) = self.state.command_dialogs.plot.take() else {
            return;
        };
        let texture = preview_texture(ctx, &dialog);
        let (mut open, mut action) = (true, None);
        let body_height = (ctx.screen_rect().height() - 190.0).max(240.0);
        widgets::dialog(ctx, theme, "Plot", &mut open, WIDTH, |ui| {
            egui::ScrollArea::vertical()
                .max_height(body_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(LEFT);
                            output_box(ui, theme, &mut dialog);
                            ui.add_space(6.0);
                            paper_box(ui, theme, &mut dialog.draft);
                            ui.add_space(6.0);
                            let mut pick = false;
                            plot_box(ui, theme, &mut dialog.draft, Some(&mut pick));
                            if pick {
                                action = Some(Action::PickWindow);
                            }
                            ui.add_space(6.0);
                            options_box(ui, theme, &mut dialog.draft, false);
                        });
                        ui.add_space(GAP);
                        ui.vertical(|ui| {
                            let width = (WIDTH - LEFT - GAP - 36.0).max(300.0);
                            ui.set_width(width);
                            preview_box(ui, theme, &dialog, texture.as_ref(), PREVIEW_HEIGHT);
                        });
                    });
                });
            ui.add_space(6.0);
            if let Some(why) = dialog.error.as_deref().or(controls.reason) {
                ui.colored_label(theme.colors.accent, why);
            }
            ui.horizontal(|ui| {
                let model = dialog.draft.space == od_core::plot::PlotSpace::Model;
                ui.checkbox(
                    &mut dialog.save_changes,
                    if model {
                        "Save changes to Model tab"
                    } else {
                        "Save changes to layout"
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        action = Some(Action::Cancel);
                    }
                    if widgets::secondary_button(
                        ui,
                        theme,
                        if model {
                            "Apply to Model"
                        } else {
                            "Apply to Layout"
                        },
                    )
                    .clicked()
                    {
                        action = Some(Action::Apply);
                    }
                    ui.add_enabled_ui(controls.can_submit, |ui| {
                        if widgets::primary_button(ui, theme, "Plot").clicked() {
                            action = Some(Action::Plot);
                        }
                    });
                });
            });
        });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = Some(Action::Cancel);
        }
        self.state.command_dialogs.plot = Some(dialog);
        match action {
            Some(Action::PickWindow) => self.state.plot_dialog_pick_window(),
            Some(Action::Apply) => self.state.plot_dialog_apply(),
            Some(Action::Plot) => {
                self.state.plot_dialog_run();
            }
            Some(Action::Cancel) => self.state.command_dialogs.plot = None,
            None => {}
        }
    }
}

/// The preview as a texture, uploaded again only when the picture changed.
fn preview_texture(ctx: &egui::Context, dialog: &PlotDialog) -> Option<egui::TextureHandle> {
    let preview = dialog.preview.as_ref()?;
    let id = egui::Id::new("plot_preview_texture");
    let kept: Option<(u64, egui::TextureHandle)> = ctx.data(|d| d.get_temp(id));
    if let Some((serial, texture)) = kept {
        if serial == dialog.preview_serial {
            return Some(texture);
        }
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(preview.size, &preview.rgba);
    let texture = ctx.load_texture("plot_preview", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (dialog.preview_serial, texture.clone())));
    Some(texture)
}

/// The wide picture of the plot, the printable area marked on the paper, and
/// a reminder when the settings have moved on from what it shows.
fn preview_box(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    dialog: &PlotDialog,
    texture: Option<&egui::TextureHandle>,
    height: f32,
) {
    widgets::group_box(ui, theme, "Preview", |ui| {
        let size = egui::vec2(ui.available_width(), height);
        let (canvas, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 2.0, ui.visuals().extreme_bg_color);
        match (texture, dialog.preview.as_ref()) {
            (Some(texture), Some(preview)) => {
                let shape = egui::vec2(preview.size[0] as f32, preview.size[1] as f32);
                let room = canvas.shrink(10.0);
                let fit = (room.width() / shape.x).min(room.height() / shape.y);
                let paper = egui::Rect::from_center_size(canvas.center(), shape * fit);
                painter.image(
                    texture.id(),
                    paper,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                painter.rect_stroke(
                    paper,
                    0.0,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_gray(110)),
                    egui::StrokeKind::Outside,
                );
                printable_outline(&painter, paper, dialog);
            }
            _ => {
                painter.text(
                    canvas.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drawing the preview...",
                    egui::FontId::proportional(13.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }
        let stale = dialog.previewed.as_ref() != Some(&dialog.draft);
        if dialog.preview.is_some() && stale {
            ui.small("Updating the preview...");
        } else if let Some(preview) = &dialog.preview {
            for note in &preview.notes {
                ui.small(note);
            }
        }
    });
}

/// The edge of the area the printer can reach, on the paper in the picture.
fn printable_outline(painter: &egui::Painter, paper: egui::Rect, dialog: &PlotDialog) {
    let [left, bottom, right, top] = dialog.draft.plot.margins_mm;
    if [left, bottom, right, top].iter().all(|m| *m == 0.0) {
        return;
    }
    let (w, h) = (dialog.draft.width as f32, dialog.draft.height as f32);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (kx, ky) = (paper.width() / w, paper.height() / h);
    let printable = egui::Rect::from_min_max(
        paper.min + egui::vec2(left as f32 * kx, top as f32 * ky),
        paper.max - egui::vec2(right as f32 * kx, bottom as f32 * ky),
    );
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(80, 150, 170));
    painter.add(egui::Shape::dashed_line(
        &[
            printable.left_top(),
            printable.right_top(),
            printable.right_bottom(),
            printable.left_bottom(),
            printable.left_top(),
        ],
        stroke,
        5.0,
        3.0,
    ));
}

/// The device the plot goes to: OpenDraft's own PDF and SVG, or one of the
/// machine's printers. Copies and resolution appear only for a printer.
fn output_box(ui: &mut egui::Ui, theme: &od_ui::Theme, dialog: &mut PlotDialog) {
    widgets::group_box(ui, theme, "Printer/plotter", |ui| {
        let shown = dialog.target.device_name();
        let width = ui.available_width();
        widgets::dropdown(ui, theme, "plot_device", &shown, width, |ui| {
            for target in [Target::Pdf, Target::Svg] {
                let name = target.device_name();
                if ui
                    .selectable_label(dialog.target == target, &name)
                    .clicked()
                {
                    dialog.target = target;
                }
            }
            if let Printers::Ready(list) = &dialog.printers {
                if !list.is_empty() {
                    ui.separator();
                    ui.small("System printers");
                }
                for p in list {
                    let chosen = dialog.target == Target::Printer(p.id.clone());
                    if ui.selectable_label(chosen, &p.id.0).clicked() {
                        dialog.target = Target::Printer(p.id.clone());
                    }
                }
            }
        });
        match &dialog.printers {
            Printers::Asking(_) => {
                ui.small("Looking for system printers...");
            }
            Printers::Failed(why) => {
                ui.small(format!("System printers are not available: {why}"));
            }
            Printers::Ready(_) => {}
        }
        if matches!(dialog.target, Target::Printer(_)) {
            ui.horizontal(|ui| {
                widgets::field_label(ui, theme, "Copies");
                ui.add(egui::DragValue::new(&mut dialog.copies).range(1..=999));
                ui.add_space(8.0);
                widgets::field_label(ui, theme, "Resolution");
                ui.add(
                    egui::DragValue::new(&mut dialog.dpi)
                        .range(72..=1200)
                        .suffix(" dpi"),
                );
            });
        }
    });
}
