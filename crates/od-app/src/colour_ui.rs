//! Colour pickers shared by the Layers and Properties panels: a swatch or a
//! swatch-and-name field that opens a palette.

use eframe::egui::{self, Align2, Color32, FontId, Id, Rect, Response, Sense, Stroke, Ui};
use od_core::Color;
use od_ui::{menu, Theme};

/// What the user picked in the palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    ByLayer,
    Color(Color),
}

/// The standard colours offered: the AutoCAD index colours, then lighter
/// shades.
const PALETTE: [(u8, u8, u8); 16] = [
    (255, 0, 0),
    (255, 255, 0),
    (0, 255, 0),
    (0, 255, 255),
    (0, 0, 255),
    (255, 0, 255),
    (255, 127, 0),
    (128, 128, 128),
    (192, 192, 192),
    (255, 128, 128),
    (255, 255, 128),
    (128, 255, 128),
    (128, 255, 255),
    (128, 128, 255),
    (255, 128, 255),
    (165, 82, 0),
];

const CELL: f32 = 20.0;
const COLUMNS: usize = 8;

/// The colour as drawn on screen.
pub fn display(theme: &Theme, color: Color) -> Color32 {
    match color {
        Color::Foreground => theme.colors.text,
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

/// Human name: "Foreground", "Red" or "#FF8000".
pub fn name(color: Color) -> String {
    match color {
        Color::Foreground => "Foreground".to_owned(),
        Color::Rgb(r, g, b) => match PALETTE_NAMES.iter().find(|(c, _)| *c == (r, g, b)) {
            Some((_, name)) => (*name).to_owned(),
            None => format!("#{r:02X}{g:02X}{b:02X}"),
        },
    }
}

const PALETTE_NAMES: [((u8, u8, u8), &str); 8] = [
    ((255, 0, 0), "Red"),
    ((255, 255, 0), "Yellow"),
    ((0, 255, 0), "Green"),
    ((0, 255, 255), "Cyan"),
    ((0, 0, 255), "Blue"),
    ((255, 0, 255), "Magenta"),
    ((255, 127, 0), "Orange"),
    ((128, 128, 128), "Grey"),
];

/// Parse "#RRGGBB" or "RRGGBB".
pub fn parse_hex(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color::Rgb(byte(0)?, byte(2)?, byte(4)?))
}

fn swatch(ui: &Ui, theme: &Theme, rect: Rect, color: Color32) {
    ui.painter().rect_filled(rect, 2.0, color);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, theme.colors.border),
        egui::StrokeKind::Inside,
    );
}

/// A square swatch that opens the palette (layer table cells). `by_layer`
/// adds a ByLayer entry, for objects.
pub fn swatch_button(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    color: Color,
    by_layer: bool,
) -> Option<Pick> {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), Sense::click());
    let inner = rect.shrink2(egui::vec2(3.0, 2.0));
    swatch(ui, theme, inner, display(theme, color));
    if response.hovered() {
        ui.painter().rect_stroke(
            inner.expand(1.0),
            2.0,
            Stroke::new(1.0_f32, theme.colors.text_weak),
            egui::StrokeKind::Outside,
        );
    }
    palette_popup(ui, theme, id, &response, by_layer)
}

/// A drop-down field showing an optional swatch and a label ("Red", "ByLayer",
/// "Varies"). Opens the palette.
pub fn field(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    size: egui::Vec2,
    swatch_color: Option<Color>,
    label: &str,
    by_layer: bool,
) -> Option<Pick> {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    menu::paint_combo(ui, theme, rect, "", response.hovered());
    let mut text_x = rect.left() + 8.0;
    if let Some(color) = swatch_color {
        let box_rect = Rect::from_center_size(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            egui::vec2(12.0, 12.0),
        );
        swatch(ui, theme, box_rect, display(theme, color));
        text_x = rect.left() + 28.0;
    }
    ui.painter().text(
        egui::pos2(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(theme.font_size_body),
        theme.colors.text,
    );
    palette_popup(ui, theme, id, &response, by_layer)
}

fn palette_popup(
    ui: &Ui,
    theme: &Theme,
    id: Id,
    anchor: &Response,
    by_layer: bool,
) -> Option<Pick> {
    let width = COLUMNS as f32 * (CELL + 2.0) + 4.0;
    menu::popup(ui, theme, id, anchor, width, |ui| {
        let mut picked = None;
        if by_layer && menu::menu_row(ui, theme, "ByLayer", false) {
            picked = Some(Pick::ByLayer);
        }
        if menu::menu_row(ui, theme, "Foreground", false) {
            picked = Some(Pick::Color(Color::Foreground));
        }
        ui.add_space(4.0);
        for row in PALETTE.chunks(COLUMNS) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                for &(r, g, b) in row {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(CELL, CELL), Sense::click());
                    swatch(ui, theme, rect, Color32::from_rgb(r, g, b));
                    if response.hovered() {
                        ui.painter().rect_stroke(
                            rect,
                            2.0,
                            Stroke::new(1.5_f32, theme.colors.text),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if response.clicked() {
                        picked = Some(Pick::Color(Color::Rgb(r, g, b)));
                    }
                }
            });
            ui.add_space(2.0);
        }
        let hex_id = id.with("hex");
        let mut text = ui
            .data(|d| d.get_temp::<String>(hex_id))
            .unwrap_or_default();
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text("#RRGGBB")
                .desired_width(width - 8.0),
        );
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(color) = parse_hex(&text) {
                picked = Some(Pick::Color(color));
                text.clear();
            }
        }
        ui.data_mut(|d| d.insert_temp(hex_id, text));
        picked
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_with_or_without_hash() {
        assert_eq!(parse_hex("#FF8000"), Some(Color::Rgb(255, 128, 0)));
        assert_eq!(parse_hex(" 00ff00 "), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_hex("#FFF"), None);
        assert_eq!(parse_hex("GG0000"), None);
        assert_eq!(parse_hex("\u{e9}\u{e9}\u{e9}"), None);
    }

    #[test]
    fn names_use_the_palette_then_hex() {
        assert_eq!(name(Color::Foreground), "Foreground");
        assert_eq!(name(Color::Rgb(255, 0, 0)), "Red");
        assert_eq!(name(Color::Rgb(1, 2, 3)), "#010203");
    }
}
