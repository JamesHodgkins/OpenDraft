//! The Paragraph dialog: the same indents and tab stops as the ruler, typed
//! as numbers. Like the ruler it only names a `ParagraphEdit`.

use eframe::egui::{self, Pos2, Rect, RichText};
use od_core::text::ParagraphEdit;
use od_ui::Theme;

use super::state::MTextEditor;

/// Show the dialog when it is open. Returns its rectangle, so a click in it
/// is not taken for a click on the drawing.
pub fn show(
    ctx: &egui::Context,
    theme: &Theme,
    editor: &mut MTextEditor,
    anchor: Rect,
    now: f64,
) -> Option<Rect> {
    if !editor.paragraph_dialog {
        return None;
    }
    let view = editor.paragraph();
    let style = view.style;
    let mut open = true;
    let new_tab_id = egui::Id::new("mtext_paragraph_new_tab");
    let mut new_tab: f64 = ctx.data(|d| d.get_temp(new_tab_id)).unwrap_or(0.0);
    let mut edit = ParagraphEdit::default();
    let mut remove = None;
    let mut add = false;
    let shown = egui::Window::new("Paragraph")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(Pos2::new(anchor.right() + 8.0, anchor.top()))
        .show(ctx, |ui| {
            if view.protected {
                ui.label(
                    RichText::new("This paragraph's formatting cannot be edited yet.")
                        .color(theme.colors.text_weak),
                );
            }
            ui.add_enabled_ui(!view.protected, |ui| {
                egui::Grid::new("mtext_paragraph_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("First line");
                        let mut first = style.left_indent + style.first_indent;
                        if ui
                            .add(egui::DragValue::new(&mut first).range(0.0..=1e6).speed(0.1))
                            .changed()
                        {
                            edit.first_indent = Some(first - style.left_indent);
                        }
                        ui.end_row();
                        ui.label("Paragraph");
                        let mut left = style.left_indent;
                        if ui
                            .add(egui::DragValue::new(&mut left).range(0.0..=1e6).speed(0.1))
                            .changed()
                        {
                            edit.left_indent = Some(left);
                        }
                        ui.end_row();
                    });
                ui.separator();
                ui.label(RichText::new("Tab stops").color(theme.colors.text_weak));
                for tab in &style.tabs {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}", tab.offset));
                        if ui.small_button("\u{d7}").on_hover_text("Remove").clicked() {
                            remove = Some(tab.offset);
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut new_tab)
                            .range(0.0..=1e6)
                            .speed(0.1),
                    );
                    add = ui.button("Add").clicked();
                });
            });
        });
    ctx.data_mut(|d| d.insert_temp(new_tab_id, new_tab));
    if add || remove.is_some() {
        let mut tabs: Vec<f64> = style.tabs.iter().map(|t| t.offset).collect();
        match remove {
            Some(gone) => tabs.retain(|t| (t - gone).abs() > 1e-9),
            None => tabs.push(new_tab),
        }
        edit.tabs = Some(tabs);
    }
    if edit != ParagraphEdit::default() {
        editor.set_paragraph(&edit, add || remove.is_some(), now);
    }
    editor.paragraph_dialog = open;
    shown.map(|r| r.response.rect)
}
