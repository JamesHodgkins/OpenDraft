//! The attribute list in the Attribute Editor's first tab: Tag, Prompt and
//! Value of every attribute, the selected one picked out in grey (the accent
//! colour is kept for the OK button).

use eframe::egui;
use od_core::block::attribute_draft::EditorDraft;
use od_ui::Theme;

const ROW: f32 = 24.0;
const HEIGHT: f32 = 148.0;
const PAD: f32 = 8.0;
/// Widths of the Tag and Prompt columns; the Value column takes the rest.
const COLUMNS: [f32; 2] = [110.0, 130.0];

/// Draw the list and select the attribute that is clicked.
pub(super) fn list(ui: &mut egui::Ui, theme: &Theme, draft: &mut EditorDraft) {
    let c = &theme.colors;
    let font = egui::FontId::proportional(theme.font_size_body);
    egui::Frame::new()
        .fill(c.input_bg)
        .stroke(egui::Stroke::new(1.0_f32, c.border))
        .corner_radius(egui::CornerRadius::same(theme.radius))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let width = ui.available_width();
            let (head, _) = ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::hover());
            let cells = columns(head, width);
            for (cell, title) in cells.iter().zip(["Tag", "Prompt", "Value"]) {
                ui.painter().text(
                    cell.left_center() + egui::vec2(PAD, 0.0),
                    egui::Align2::LEFT_CENTER,
                    title,
                    font.clone(),
                    c.text_weak,
                );
            }
            ui.painter()
                .hline(head.x_range(), head.bottom(), (1.0, c.border));
            egui::ScrollArea::vertical()
                .max_height(HEIGHT - ROW)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for i in 0..draft.rows.len() {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::click());
                        if draft.selected == i {
                            ui.painter().rect_filled(rect, 0.0, c.pressed);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 0.0, c.hover);
                        }
                        let row = &draft.rows[i];
                        for (cell, text) in
                            columns(rect, width)
                                .iter()
                                .zip([&row.tag, &row.prompt, &row.value])
                        {
                            let clip = cell.shrink2(egui::vec2(PAD, 0.0));
                            ui.painter().with_clip_rect(clip).text(
                                clip.left_center(),
                                egui::Align2::LEFT_CENTER,
                                text,
                                font.clone(),
                                c.text,
                            );
                        }
                        if response.clicked() {
                            draft.selected = i;
                        }
                    }
                });
        });
}

/// The three cells of a row.
fn columns(row: egui::Rect, width: f32) -> [egui::Rect; 3] {
    let first = egui::Rect::from_min_size(row.min, egui::vec2(COLUMNS[0], row.height()));
    let second = egui::Rect::from_min_size(first.right_top(), egui::vec2(COLUMNS[1], row.height()));
    let third = egui::Rect::from_min_max(
        second.right_top(),
        egui::pos2(row.min.x + width, row.bottom()),
    );
    [first, second, third]
}
