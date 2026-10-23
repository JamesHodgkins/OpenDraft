//! The Text Style dialog (STYLE), laid out as AutoCAD's: the styles with a
//! filter and a preview, the font, size and effects of the selected one, and
//! Set Current, New and Delete. It edits a draft; Apply commits it as one undo
//! step, Cancel / Escape / X drop it.

use eframe::egui::{self, Sense};
use od_core::text::{MAX_WEIGHT, MIN_WEIGHT};
use od_core::{FontDescriptor, TextStyleId};
use od_ui::{widgets, Theme};

use super::text_style_preview::preview;
use super::OpenDraftApp;
use crate::text_style_draft::Draft;

/// Wide enough for the list, settings and buttons (190 + 330 + 96 + two 14 gaps) and the 16 margins.
const WIDTH: f32 = 676.0;
const LIST_WIDTH: f32 = 190.0;
const LIST_HEIGHT: f32 = 190.0;
const ROW_HEIGHT: f32 = 22.0;
const SETTINGS_WIDTH: f32 = 330.0;
const BUTTON_WIDTH: f32 = 96.0;
/// Fonts offered first, ahead of the installed families; any other name can be typed.
const FONTS: [&str; 9] = [
    "DejaVu Sans",
    "OD Roman",
    "Arial",
    "Times New Roman",
    "Courier New",
    "Calibri",
    "txt.shx",
    "simplex.shx",
    "romans.shx",
];
const FACES: [(&str, bool, bool); 4] = [
    ("Regular", false, false),
    ("Bold", true, false),
    ("Italic", false, true),
    ("Bold Italic", true, true),
];

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    InUse,
}

#[derive(Clone, Copy)]
enum Choice {
    Apply,
    Cancel,
    SetCurrent,
    New,
    Delete(TextStyleId),
    NewOk,
    NewCancel,
}

impl OpenDraftApp {
    /// The window opened by `annotate.textstyle`.
    pub(super) fn text_style_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_text_styles {
            self.text_style_draft = None;
            self.text_style_new = None;
            return;
        }
        let mut draft = self
            .text_style_draft
            .take()
            .unwrap_or_else(|| Draft::from_document(self.state.session().document()));
        let (mut open, mut choice) = (true, None);
        let naming = self.text_style_new.is_some();
        let document = self.state.session().document();
        let changed = draft.changed(document) && draft.is_valid();
        let deletable = draft.can_delete(document, draft.selected);
        let in_use: Vec<TextStyleId> = draft
            .styles
            .iter()
            .filter(|s| document.text_style_in_use(s.id))
            .map(|s| s.id)
            .collect();
        let theme = &self.theme;
        let filter_id = egui::Id::new("text_style_filter");
        let mut filter = ctx.data(|d| d.get_temp(filter_id)).unwrap_or(Filter::All);
        widgets::dialog(ctx, theme, "Text Style", &mut open, WIDTH, |ui| {
            ui.add_enabled_ui(!naming, |ui| {
                ui.horizontal(|ui| {
                    let name = draft
                        .styles
                        .iter()
                        .find(|s| s.id == draft.current)
                        .map_or("Standard", |s| s.name.as_str());
                    widgets::field_label(ui, theme, &format!("Current text style:  {name}"));
                });
                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    // The gaps are the add_space calls; the default spacing would widen the row.
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.vertical(|ui| {
                        ui.set_width(LIST_WIDTH);
                        widgets::field_label(ui, theme, "Styles:");
                        ui.add_space(2.0);
                        list(ui, theme, &mut draft, filter, &in_use);
                        ui.add_space(6.0);
                        widgets::dropdown(
                            ui,
                            theme,
                            "text_style_filter_box",
                            if filter == Filter::All {
                                "All styles"
                            } else {
                                "Styles in use"
                            },
                            LIST_WIDTH,
                            |ui| {
                                for (label, value) in [
                                    ("All styles", Filter::All),
                                    ("Styles in use", Filter::InUse),
                                ] {
                                    if ui.selectable_label(filter == value, label).clicked() {
                                        filter = value;
                                    }
                                }
                            },
                        );
                        ui.add_space(6.0);
                        preview(ui, theme, &draft);
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.set_width(SETTINGS_WIDTH);
                        settings(ui, theme, &mut draft);
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.set_width(BUTTON_WIDTH);
                        let enabled = |ui: &mut egui::Ui, on: bool, label: &str| {
                            ui.add_enabled_ui(on, |ui| widgets::secondary_button(ui, theme, label))
                                .inner
                                .clicked()
                        };
                        if enabled(ui, draft.selected != draft.current, "Set Current") {
                            choice = Some(Choice::SetCurrent);
                        }
                        ui.add_space(4.0);
                        if enabled(ui, true, "New...") {
                            choice = Some(Choice::New);
                        }
                        ui.add_space(4.0);
                        if enabled(ui, deletable, "Delete") {
                            choice = Some(Choice::Delete(draft.selected));
                        }
                    });
                });
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        choice = Some(Choice::Cancel);
                    }
                    ui.add_space(6.0);
                    let apply = ui
                        .add_enabled_ui(changed, |ui| widgets::primary_button(ui, theme, "Apply"));
                    if apply.inner.clicked() {
                        choice = Some(Choice::Apply);
                    }
                });
            });
        });
        ctx.data_mut(|d| d.insert_temp(filter_id, filter));
        if naming {
            choice = self.new_style_window(ctx, &draft).or(choice);
        } else if !open {
            choice = Some(Choice::Cancel);
        }
        match choice {
            Some(Choice::Apply) => {
                self.state.set_text_styles(&draft);
                draft = Draft::from_document(self.state.session().document());
                draft.selected = draft.current;
                self.text_style_draft = Some(draft);
            }
            Some(Choice::Cancel) => self.state.show_text_styles = false,
            Some(Choice::SetCurrent) => {
                draft.current = draft.selected;
                self.text_style_draft = Some(draft);
            }
            Some(Choice::New) => {
                self.text_style_new = Some(draft.suggested_name());
                self.text_style_draft = Some(draft);
            }
            Some(Choice::Delete(id)) => {
                draft.delete(id);
                self.text_style_draft = Some(draft);
            }
            Some(Choice::NewOk) => {
                let name = self.text_style_new.take().unwrap_or_default();
                let _ = draft.add(&name);
                self.text_style_draft = Some(draft);
            }
            Some(Choice::NewCancel) => {
                self.text_style_new = None;
                self.text_style_draft = Some(draft);
            }
            None => self.text_style_draft = Some(draft),
        }
    }

    /// New Text Style: the name of the style to add.
    fn new_style_window(&mut self, ctx: &egui::Context, draft: &Draft) -> Option<Choice> {
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        let name = self.text_style_new.as_mut()?;
        widgets::dialog(ctx, theme, "New Text Style", &mut open, 300.0, |ui| {
            widgets::field_label(ui, theme, "Style name:");
            ui.add_space(2.0);
            let typed = widgets::text_field(ui, theme, name, "");
            let problem = draft.name_problem(name);
            if let Some(problem) = &problem {
                ui.add_space(4.0);
                widgets::field_label(ui, theme, problem);
            }
            if typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                choice = problem.is_none().then_some(Choice::NewOk);
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let ok = ui.add_enabled_ui(problem.is_none(), |ui| {
                    widgets::primary_button(ui, theme, "OK")
                });
                if ok.inner.clicked() {
                    choice = Some(Choice::NewOk);
                }
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    choice = Some(Choice::NewCancel);
                }
            });
        });
        if !open {
            choice = Some(Choice::NewCancel);
        }
        choice
    }
}

/// The styles, one selected; the current one is marked.
fn list(
    ui: &mut egui::Ui,
    theme: &Theme,
    draft: &mut Draft,
    filter: Filter,
    in_use: &[TextStyleId],
) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, theme.colors.border))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(LIST_HEIGHT)
                .min_scrolled_height(LIST_HEIGHT)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let rows: Vec<(TextStyleId, String)> = draft
                        .styles
                        .iter()
                        .filter(|s| filter == Filter::All || in_use.contains(&s.id))
                        .map(|s| (s.id, s.name.clone()))
                        .collect();
                    for (id, name) in rows {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            Sense::click(),
                        );
                        if response.clicked() {
                            draft.selected = id;
                        }
                        if draft.selected == id {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.accent);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.hover);
                        }
                        let label = if id == draft.current {
                            format!("{name}  (current)")
                        } else {
                            name
                        };
                        ui.painter().text(
                            egui::pos2(rect.left() + 8.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::proportional(theme.font_size_body),
                            theme.colors.text,
                        );
                    }
                });
        });
}

/// Font, Size and Effects of the selected style.
fn settings(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let Some(mut style) = draft
        .styles
        .iter()
        .find(|s| s.id == draft.selected)
        .cloned()
    else {
        return;
    };
    let before = style.clone();
    let (family, mut bold, mut italic) = style.font.name_and_flags();
    let shx = style.font.is_shx();
    let stroke = matches!(style.font, FontDescriptor::OdRoman { .. });
    // What is typed is kept as typed (a trailing space is part of a name
    // being typed); the style gets it tidied.
    let typed = match &draft.font_text {
        Some((id, text)) if *id == style.id => text.clone(),
        _ => family.clone(),
    };
    let mut typed = typed;

    widgets::group_box(ui, theme, "Font", |ui| {
        widgets::field_label(ui, theme, "Font Name:");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.allocate_ui(egui::vec2(SETTINGS_WIDTH - 90.0, 28.0), |ui| {
                widgets::text_field(ui, theme, &mut typed, "Font name");
            });
            widgets::dropdown(ui, theme, "text_style_fonts", "", 40.0, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        let installed = crate::system_fonts::families().iter().map(String::as_str);
                        for name in FONTS.into_iter().chain(installed) {
                            if ui.selectable_label(false, name).clicked() {
                                typed = name.to_owned();
                            }
                        }
                    });
            });
        });
        ui.add_space(6.0);
        widgets::field_label(ui, theme, "Font Style:");
        let face = FACES
            .iter()
            .find(|f| f.1 == bold && f.2 == italic)
            .map_or("Regular", |f| f.0);
        ui.add_enabled_ui(!shx, |ui| {
            widgets::dropdown(ui, theme, "text_style_face", face, 150.0, |ui| {
                for (label, b, i) in FACES {
                    if ui.selectable_label(face == label, label).clicked() {
                        (bold, italic) = (b, i);
                    }
                }
            });
        });
        ui.add_space(6.0);
        let mut big = style.big_font.is_some();
        ui.add_enabled_ui(shx, |ui| {
            if ui.checkbox(&mut big, "Use Big Font").changed() {
                style.big_font = big.then(|| "bigfont.shx".to_owned());
            }
            if let Some(name) = style.big_font.as_mut().filter(|_| shx) {
                widgets::text_field(ui, theme, name, "Big font");
            }
        });
        if stroke {
            ui.add_space(6.0);
            widgets::field_label(ui, theme, "Line Weight (% of text height):");
            let mut percent = style.stroke_weight * 100.0;
            widgets::number_field(ui, theme, &mut percent, "%", 0.5);
            let (low, high) = (MIN_WEIGHT * 100.0, MAX_WEIGHT * 100.0);
            style.stroke_weight = percent.clamp(low, high) / 100.0;
        }
        ui.add_space(4.0);
        widgets::field_label(
            ui,
            theme,
            "Only DejaVu Sans and OD Roman are installed; other fonts are kept in the style and drawn with DejaVu Sans.",
        );
    });
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Size", |ui| {
        widgets::field_label(ui, theme, "Height (0 asks each time):");
        let mut height = style.fixed_height.unwrap_or(0.0);
        widgets::number_field(ui, theme, &mut height, "", 0.1);
        style.fixed_height = (height > 0.0).then_some(height);
    });
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Effects", |ui| {
        ui.checkbox(&mut style.upside_down, "Upside down");
        ui.checkbox(&mut style.backward, "Backwards");
        ui.add_enabled(shx, egui::Checkbox::new(&mut style.vertical, "Vertical"));
        ui.add_space(4.0);
        widgets::field_label(ui, theme, "Width Factor:");
        widgets::number_field(ui, theme, &mut style.width_factor, "", 0.01);
        style.width_factor = style.width_factor.clamp(0.01, 100.0);
        ui.add_space(4.0);
        widgets::field_label(ui, theme, "Oblique Angle:");
        let mut degrees = style.oblique_radians.to_degrees();
        widgets::number_field(ui, theme, &mut degrees, "\u{b0}", 1.0);
        style.oblique_radians = degrees.clamp(-85.0, 85.0).to_radians();
    });

    let font = FontDescriptor::from_name(&typed, bold, italic);
    if typed.trim().is_empty() {
        // An empty name keeps the old font until something is typed.
    } else if font != before.font {
        style.font = font;
    }
    draft.font_text = Some((style.id, typed));
    if style != before {
        if let Some(slot) = draft.selected_style() {
            *slot = style;
        }
    }
}
