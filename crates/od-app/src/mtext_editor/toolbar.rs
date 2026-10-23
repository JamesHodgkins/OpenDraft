//! The editor's toolbar: undo and redo, the formatting controls and OK and
//! Cancel. Each control shows what the selection (or the caret) looks like
//! and, when it is changed, asks the editor for a `CharEdit`; the toolbar
//! never writes a control itself.

use eframe::egui::{self, Color32, Pos2, Rect, RichText};
use od_core::aci::aci_rgb;
use od_core::text::{CharEdit, ListKind, Mixed, ParagraphAlignment, ParagraphEdit, TextColor};
use od_core::TextStyle;
use od_ui::icon_set::{self, Icon};
use od_ui::widgets::icon_text;
use od_ui::{icons, Theme};

use super::input::Outcome;
use super::state::MTextEditor;

/// The room the toolbar needs above the box.
const HEIGHT: f32 = 66.0;
const WIDTH: f32 = 440.0;

/// The standard colours offered, with their AutoCAD indexes.
const COLOURS: [(&str, u16); 7] = [
    ("Red", 1),
    ("Yellow", 2),
    ("Green", 3),
    ("Cyan", 4),
    ("Blue", 5),
    ("Magenta", 6),
    ("White", 7),
];

fn swatch(color: TextColor, fallback: Color32) -> Color32 {
    match color {
        TextColor::Entity | TextColor::Aci(7) => fallback,
        TextColor::Aci(n) => {
            let (r, g, b) = aci_rgb(u8::try_from(n).unwrap_or(7));
            Color32::from_rgb(r, g, b)
        }
        TextColor::Rgb(v) => Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
    }
}

/// A toggle that shows on, off or mixed. Returns the value it would be set
/// to when clicked.
fn toggle(ui: &mut egui::Ui, label: RichText, tip: &str, state: Mixed<bool>) -> Option<bool> {
    let on = state == Mixed::Uniform(true);
    let label = if state == Mixed::Mixed {
        label.weak()
    } else {
        label
    };
    let clicked = ui.selectable_label(on, label).on_hover_text(tip).clicked();
    clicked.then_some(!on)
}

/// A number field. Returns the new value when it was changed.
fn number(
    ui: &mut egui::Ui,
    label: &str,
    state: Mixed<f64>,
    shown_if_mixed: f64,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
) -> Option<f64> {
    ui.label(RichText::new(label).weak());
    let mut value = state.uniform().unwrap_or(shown_if_mixed);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .range(range)
                .speed(speed)
                .max_decimals(4),
        )
        .on_hover_text(if state == Mixed::Mixed {
            "Mixed values: set one for the whole selection"
        } else {
            label
        });
    response.changed().then_some(value)
}

/// Show the toolbar above the box (or below it with no room above). Returns
/// what was chosen and the toolbar's rectangle.
pub fn show(
    ctx: &egui::Context,
    theme: &Theme,
    editor: &mut MTextEditor,
    styles: &[TextStyle],
    bounds: Rect,
    canvas: Rect,
) -> (Option<Outcome>, Rect) {
    let above = bounds.top() - HEIGHT - 6.0;
    let y = if above >= canvas.top() {
        above
    } else {
        (bounds.bottom() + 6.0).min(canvas.bottom() - HEIGHT)
    };
    let x = bounds
        .left()
        .clamp(canvas.left(), (canvas.right() - WIDTH).max(canvas.left()));
    let now = ctx.input(|i| i.time);
    let mut outcome = None;
    let area = egui::Area::new(egui::Id::new("mtext_toolbar"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(x, y))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme.colors.panel_bg)
                .show(ui, |ui| {
                    ui.set_min_width(WIDTH - 24.0);
                    ui.horizontal(|ui| {
                        outcome = top_row(ui, theme, editor, styles, now);
                    });
                    ui.horizontal(|ui| size_row(ui, theme, editor, now));
                });
        });
    (outcome, area.response.rect)
}

/// The text style the text is written in, from the drawing's styles.
fn style_menu(ui: &mut egui::Ui, editor: &mut MTextEditor, styles: &[TextStyle]) {
    let current = editor.style_id();
    let name = styles
        .iter()
        .find(|s| s.id == current)
        .map_or("Standard", |s| s.name.as_str());
    let mut chosen = None;
    egui::ComboBox::from_id_salt("mtext_style")
        .selected_text(name)
        .width(110.0)
        .show_ui(ui, |ui| {
            for style in styles {
                if ui
                    .selectable_label(style.id == current, &style.name)
                    .clicked()
                {
                    chosen = Some(style);
                }
            }
        })
        .response
        .on_hover_text("Text style");
    if let Some(style) = chosen {
        editor.set_style(style);
    }
}

fn top_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    editor: &mut MTextEditor,
    styles: &[TextStyle],
    now: f64,
) -> Option<Outcome> {
    let mut outcome = None;
    ui.label(RichText::new("Text Editor").color(theme.colors.text_weak));
    style_menu(ui, editor, styles);
    let undo = Icon::new(icons::TEXT_UNDO, 14.0).button();
    if ui
        .add_enabled(editor.can_undo(), undo)
        .on_hover_text("Undo (Ctrl+Z)")
        .clicked()
    {
        editor.undo();
    }
    let redo = Icon::new(icons::TEXT_REDO, 14.0).button();
    if ui
        .add_enabled(editor.can_redo(), redo)
        .on_hover_text("Redo (Ctrl+Y)")
        .clicked()
    {
        editor.redo();
    }
    ui.separator();
    let format = editor.format();
    let bold = toggle(
        ui,
        RichText::new("B").strong(),
        "Bold (Ctrl+B)",
        format.bold,
    );
    let italic = toggle(
        ui,
        RichText::new("I").italics(),
        "Italic (Ctrl+I)",
        format.italic,
    );
    let underline = toggle(
        ui,
        RichText::new("U").underline(),
        "Underline (Ctrl+U)",
        format.underline,
    );
    let overline = toggle(ui, RichText::new("\u{14c}"), "Overline", format.overline);
    let strike = toggle(
        ui,
        RichText::new("S").strikethrough(),
        "Strikethrough",
        format.strike,
    );
    let mut edit = CharEdit {
        bold,
        italic,
        underline,
        overline,
        strike,
        ..CharEdit::default()
    };
    colour_menu(ui, theme, format.color, &mut edit);
    if !edit.is_empty() {
        editor.set_format(edit, now);
    }
    let stack = ui
        .add_enabled(editor.has_selection(), egui::Button::new("a/b"))
        .on_hover_text("Stack or unstack the selected text (1/2, 1^2, 1#2)");
    if stack.clicked() {
        editor.toggle_stack(now);
    }
    let paragraph = ui
        .selectable_label(editor.paragraph_dialog, "¶")
        .on_hover_text("Paragraph: indents and tab stops");
    if paragraph.clicked() {
        editor.paragraph_dialog = !editor.paragraph_dialog;
    }
    ui.separator();
    if ui.button("OK").on_hover_text("Ctrl+Enter").clicked() {
        outcome = Some(Outcome::Accept);
    }
    if ui.button("Cancel").on_hover_text("Esc").clicked() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

fn colour_menu(ui: &mut egui::Ui, theme: &Theme, current: Mixed<TextColor>, edit: &mut CharEdit) {
    let (mark, tint) = match current {
        Mixed::Uniform(c) => ("\u{25a0}", swatch(c, theme.colors.text)),
        Mixed::Mixed => ("*", theme.colors.text_weak),
    };
    ui.menu_button(RichText::new(mark).color(tint), |ui| {
        if ui.button("ByLayer").clicked() {
            edit.color = Some(TextColor::Entity);
            ui.close_menu();
        }
        for (name, index) in COLOURS {
            let tint = swatch(TextColor::Aci(index), Color32::WHITE);
            let label = RichText::new(format!("\u{25a0} {name}")).color(tint);
            if ui.button(label).clicked() {
                edit.color = Some(TextColor::Aci(index));
                ui.close_menu();
            }
        }
        ui.separator();
        let mut rgb = match current {
            Mixed::Uniform(TextColor::Rgb(v)) => [(v >> 16) as u8, (v >> 8) as u8, v as u8],
            _ => [255, 255, 255],
        };
        ui.horizontal(|ui| {
            ui.label("Custom");
            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                let value =
                    (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]);
                edit.color = Some(TextColor::Rgb(value));
            }
        });
    })
    .response
    .on_hover_text("Colour");
}

fn size_row(ui: &mut egui::Ui, theme: &Theme, editor: &mut MTextEditor, now: f64) {
    let format = editor.format();
    let nominal = editor.text().nominal_height;
    let mut edit = CharEdit::default();
    let height = number(
        ui,
        "Height",
        format.height,
        nominal,
        1e-6..=1e6,
        0.05 * nominal,
    );
    let oblique = number(ui, "Oblique", format.oblique, 0.0, -85.0..=85.0, 0.5);
    let tracking = number(ui, "Tracking", format.tracking, 1.0, 0.1..=10.0, 0.01);
    let width = number(ui, "Width", format.width, 1.0, 0.1..=10.0, 0.01);
    (edit.height, edit.oblique) = (height, oblique);
    (edit.tracking, edit.width) = (tracking, width);
    if editor.uses_stroke_font() {
        // The pen width in percent of the height: the run's own, or the style's.
        let style = editor.style_weight() * 100.0;
        let shown = match format.weight {
            Mixed::Uniform(w) => Mixed::Uniform(w.map_or(style, |w| f64::from(w) / 10.0)),
            Mixed::Mixed => Mixed::Mixed,
        };
        let percent = number(ui, "Weight %", shown, style, 1.0..=40.0, 0.5);
        edit.weight = percent.map(|p| (p * 10.0).round() as u16);
    }
    editor.adjust_format(edit, now);
    alignment_buttons(ui, editor, now);
    list_menu(ui, editor, now);
    if let Some(why) = &editor.notice {
        ui.colored_label(theme.colors.text_weak, why);
    }
}

/// Left, centre, right and justified, for the paragraph the caret is in.
fn alignment_buttons(ui: &mut egui::Ui, editor: &mut MTextEditor, now: f64) {
    let view = editor.paragraph();
    let current = match view.style.alignment {
        ParagraphAlignment::Default => ParagraphAlignment::Left,
        other => other,
    };
    ui.separator();
    ui.add_enabled_ui(!view.protected, |ui| {
        let choices = [
            (
                icons::TEXT_ALIGN_LEFT,
                ParagraphAlignment::Left,
                "Align left",
            ),
            (
                icons::TEXT_ALIGN_CENTER,
                ParagraphAlignment::Center,
                "Centre",
            ),
            (
                icons::TEXT_ALIGN_RIGHT,
                ParagraphAlignment::Right,
                "Align right",
            ),
            (
                icons::TEXT_ALIGN_JUSTIFY,
                ParagraphAlignment::Justified,
                "Justify",
            ),
        ];
        for (icon, alignment, tip) in choices {
            let button = Icon::new(icon, 14.0).toggle(current == alignment);
            if ui.add(button).on_hover_text(tip).clicked() && current != alignment {
                let edit = ParagraphEdit {
                    alignment: Some(alignment),
                    ..ParagraphEdit::default()
                };
                editor.set_paragraph(&edit, false, now);
            }
        }
    });
}

/// Bullets and numbering for the selected paragraphs.
fn list_menu(ui: &mut egui::Ui, editor: &mut MTextEditor, now: f64) {
    let current = editor.list_kind();
    let protected = editor.paragraph().protected;
    ui.add_enabled_ui(!protected, |ui| {
        let menu = |ui: &mut egui::Ui| {
            let choices = [
                ("Off", None),
                ("Bulleted", Some(ListKind::Bullet)),
                ("Numbered", Some(ListKind::Number)),
                ("Lettered (lowercase)", Some(ListKind::LowerLetter)),
                ("Lettered (uppercase)", Some(ListKind::UpperLetter)),
            ];
            for (name, kind) in choices {
                if ui.selectable_label(current == kind, name).clicked() {
                    editor.set_list(kind, now);
                    ui.close_menu();
                }
            }
        };
        // The list icon's drawing if it has one, else its glyph.
        let color = ui.visuals().text_color();
        let button = match icon_set::image(ui.ctx(), icons::TEXT_LIST, 14.0) {
            Some(image) => ui.menu_image_button(image.tint(color), menu),
            None => ui.menu_button(icon_text(icons::TEXT_LIST, 14.0), menu),
        };
        button.response.on_hover_text("Bullets and numbering");
    });
}
