//! What the linetype lists share: the choices the table offers and a picture
//! of each pattern to show beside its name.

use eframe::egui::{self, Color32, Painter, Rect};
use od_core::linetype::dash::{fit, Mark};
use od_core::{Document, Linetype, LinetypeId, StrokeElement};

/// How wide the picture of a pattern is in a list row.
pub const SAMPLE_WIDTH: f32 = 84.0;
/// About how long one repeat of a pattern is drawn in a picture.
const REPEAT_PX: f64 = 22.0;

/// The linetypes a list offers: the table's, in order, without `BYLAYER` and
/// `BYBLOCK` (which are where an object's linetype comes from, not linetypes).
pub fn choices(document: &Document) -> Vec<&Linetype> {
    document
        .linetypes()
        .records()
        .iter()
        .filter(|r| r.id != LinetypeId::BYLAYER && r.id != LinetypeId::BYBLOCK)
        .collect()
}

/// Where `name` is among `choices`, ignoring case.
pub fn position(choices: &[&Linetype], name: &str) -> Option<usize> {
    choices
        .iter()
        .position(|r| r.name.eq_ignore_ascii_case(name))
}

/// A list of the table's linetypes, each with a picture, under `anchor`;
/// returns the name picked. `current` is the name now in use.
pub fn pick(
    ui: &egui::Ui,
    theme: &od_ui::Theme,
    id: egui::Id,
    anchor: &egui::Response,
    document: &Document,
    current: &str,
) -> Option<String> {
    let choices = choices(document);
    let names: Vec<String> = choices.iter().map(|r| r.name.clone()).collect();
    let sample = |painter: &Painter, area: Rect, i: usize, color: Color32| {
        if let Some(record) = choices.get(i) {
            paint_sample(painter, area, &record.elements, color);
        }
    };
    let picked = od_ui::menu::list_menu_sampled(
        ui,
        theme,
        id,
        anchor,
        &names,
        position(&choices, current),
        Some((SAMPLE_WIDTH, &sample)),
    )?;
    Some(names[picked].clone())
}

/// Draw `elements` as a line across `area`, at a size that shows a few
/// repeats whatever units the pattern is in. A continuous line is solid.
pub fn paint_sample(painter: &Painter, area: Rect, elements: &[StrokeElement], color: Color32) {
    let y = area.center().y;
    let width = f64::from(area.width());
    let period: f64 = elements.iter().map(|e| e.length()).sum();
    let stroke = egui::Stroke::new(1.5_f32, color);
    let solid = || {
        painter.hline(area.x_range(), y, stroke);
    };
    if period <= 0.0 {
        return solid();
    }
    let scale = REPEAT_PX.min(width / 2.0) / period;
    let Some(marks) = fit(width, false, elements, scale) else {
        return solid();
    };
    for mark in marks {
        match mark {
            Mark::Dash { from, to } => {
                let (a, b) = (area.left() + from as f32, area.left() + to as f32);
                painter.hline(a..=b, y, stroke);
            }
            Mark::Dot(at) => {
                painter.circle_filled(egui::pos2(area.left() + at as f32, y), 1.4, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choices_leave_out_the_two_sources_and_names_match_loosely() {
        let document = Document::default();
        let all = choices(&document);
        let names: Vec<&str> = all.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names[0], "CONTINUOUS");
        assert!(!names.contains(&"BYLAYER") && !names.contains(&"BYBLOCK"));
        assert_eq!(position(&all, "dashed"), Some(1));
        assert_eq!(position(&all, "dot"), position(&all, "DOT"));
        assert_eq!(position(&all, "nothing"), None);
    }
}
