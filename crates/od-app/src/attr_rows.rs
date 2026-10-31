//! The General section of the Properties panel: layer, colour, linetype and
//! lineweight of the selected objects. Rows show "Varies" when the objects
//! disagree; choosing a value sets it on all of them.

use eframe::egui::{self, Ui};
use od_core::{Attr, Color, Document, LinetypeId, Lineweight, Object};
use od_ui::{menu, Theme};

use crate::colour_ui::{self, Pick};
use crate::layers::{has_outline, AttrEdit};
use crate::layers_ui::weight_label;
use crate::linetype_ui;
use crate::properties_ui::{row_frame, text_cell};

/// Shown when the selected objects follow their layer.
const BY_LAYER: &str = "ByLayer";
const BY_BLOCK: &str = "ByBlock";
pub(crate) const VARIES: &str = "Varies";

/// The value every item agrees on, or `None` when they differ (or there are
/// none).
pub(crate) fn shared<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

/// Draw the four rows for `objects`; returns the edit committed, if any.
pub fn rows(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let mut edit = None;
    edit = layer_row(ui, theme, document, objects).or(edit);
    edit = color_row(ui, theme, document, objects).or(edit);
    edit = linetype_row(ui, theme, document, objects).or(edit);
    edit = lineweight_row(ui, theme, objects).or(edit);
    edit = scale_row(ui, theme, objects).or(edit);
    edit = generation_row(ui, theme, objects).or(edit);
    edit
}

/// A drop-down inside the value cell of a row.
pub(crate) fn cell_combo(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    cell: egui::Rect,
    text: &str,
    entries: &[String],
    current: Option<usize>,
) -> Option<usize> {
    let area = cell.shrink(2.0);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    menu::combo(&mut child, theme, id, area.size(), text, entries, current)
}

fn layer_row(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let (_, cell) = row_frame(ui, theme, "Layer");
    let current = shared(objects.iter().map(|o| o.attrs.layer));
    let names: Vec<String> = document.layers().iter().map(|l| l.name.clone()).collect();
    let selected = current.and_then(|id| document.layer_position(id));
    let text = selected.map_or(VARIES, |i| names[i].as_str());
    let id = ui.id().with("attr_layer");
    let picked = cell_combo(ui, theme, id, cell, text, &names, selected)?;
    Some(AttrEdit::Layer(document.layers()[picked].id))
}

fn color_row(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let (_, cell) = row_frame(ui, theme, "Colour");
    let id = ui.id().with("attr_color");
    color_field(ui, theme, id, cell.shrink(2.0), document, objects)
}

/// The colour list for `objects`, drawn in `area` (also on the ribbon).
pub(crate) fn color_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    area: egui::Rect,
    document: &Document,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let attr = shared(objects.iter().map(|o| o.attrs.color.clone()));
    // The colour actually drawn, when the objects agree on it.
    let resolved = shared(objects.iter().map(|o| document.color_of(o)));
    color_value_field(ui, theme, id, area, attr, resolved)
}

/// The colour list showing `attr` (`None` for "Varies"); `resolved` is the
/// colour drawn when `attr` is ByLayer.
pub(crate) fn color_value_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    area: egui::Rect,
    attr: Option<Attr<Color>>,
    resolved: Option<Color>,
) -> Option<AttrEdit> {
    let (swatch, label) = match (attr, resolved) {
        (Some(Attr::ByLayer), resolved) => (resolved, BY_LAYER.to_owned()),
        (Some(Attr::ByBlock), resolved) => (resolved, BY_BLOCK.to_owned()),
        (Some(Attr::Set(color)), _) => (Some(color), colour_ui::name(color)),
        (None, _) => (None, VARIES.to_owned()),
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let picked = colour_ui::field(&mut child, theme, id, area.size(), swatch, &label, true)?;
    Some(AttrEdit::Color(match picked {
        Pick::ByLayer => Attr::ByLayer,
        Pick::ByBlock => Attr::ByBlock,
        Pick::Color(color) => Attr::Set(color),
    }))
}

fn linetype_row(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let (_, cell) = row_frame(ui, theme, "Linetype");
    let id = ui.id().with("attr_linetype");
    linetype_field(ui, theme, id, cell.shrink(2.0), document, objects)
}

/// The linetype list for `objects`, drawn in `area` (also on the ribbon).
pub(crate) fn linetype_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    area: egui::Rect,
    document: &Document,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let attr = shared(objects.iter().map(|o| o.attrs.linetype.clone()));
    linetype_value_field(ui, theme, id, area, document, attr)
}

/// The linetype list showing `attr` (`None` for "Varies").
pub(crate) fn linetype_value_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    area: egui::Rect,
    document: &Document,
    attr: Option<Attr<LinetypeId>>,
) -> Option<AttrEdit> {
    let choices = linetype_ui::choices(document);
    // ByLayer and ByBlock first, then the table's linetypes, each with a picture.
    let entries: Vec<String> = [BY_LAYER.to_owned(), BY_BLOCK.to_owned()]
        .into_iter()
        .chain(choices.iter().map(|r| r.name.clone()))
        .collect();
    let current = attr.as_ref().and_then(|a| match a {
        Attr::ByLayer => Some(0),
        Attr::ByBlock => Some(1),
        Attr::Set(id) => linetype_ui::position(&choices, *id).map(|i| i + 2),
    });
    let text = match (&attr, current) {
        // A reference the table cannot answer (a valid drawing has none).
        (Some(Attr::Set(_)), None) => "?",
        (_, Some(i)) => entries[i].as_str(),
        _ => VARIES,
    };
    let sample = |painter: &egui::Painter, area: egui::Rect, i: usize, color: egui::Color32| {
        if let Some(record) = i.checked_sub(2).and_then(|i| choices.get(i)) {
            linetype_ui::paint_sample(painter, area, &record.elements, color);
        }
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let picked = menu::combo_sampled(
        &mut child,
        theme,
        id,
        area.size(),
        text,
        &entries,
        current,
        Some((linetype_ui::SAMPLE_WIDTH, &sample)),
    )?;
    Some(AttrEdit::Linetype(match picked {
        0 => Attr::ByLayer,
        1 => Attr::ByBlock,
        i => Attr::Set(choices[i - 2].id),
    }))
}

fn lineweight_row(ui: &mut Ui, theme: &Theme, objects: &[&Object]) -> Option<AttrEdit> {
    let (_, cell) = row_frame(ui, theme, "Lineweight");
    let id = ui.id().with("attr_lineweight");
    lineweight_field(ui, theme, id, cell.shrink(2.0), objects)
}

/// The lineweight list for `objects`, drawn in `area` (also on the ribbon).
pub(crate) fn lineweight_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    area: egui::Rect,
    objects: &[&Object],
) -> Option<AttrEdit> {
    let attr = shared(objects.iter().map(|o| o.attrs.lineweight.clone()));
    lineweight_value_field(ui, theme, id, area, attr)
}

/// The lineweight list showing `attr` (`None` for "Varies").
pub(crate) fn lineweight_value_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    area: egui::Rect,
    attr: Option<Attr<Lineweight>>,
) -> Option<AttrEdit> {
    let entries: Vec<String> = [BY_LAYER.to_owned(), BY_BLOCK.to_owned()]
        .into_iter()
        .chain(Lineweight::STANDARD.iter().map(|w| weight_label(*w)))
        .collect();
    let current = attr.as_ref().and_then(|a| match a {
        Attr::ByLayer => Some(0),
        Attr::ByBlock => Some(1),
        Attr::Set(w) => Lineweight::STANDARD
            .iter()
            .position(|s| s == w)
            .map(|i| i + 2),
    });
    let text = current.map_or(VARIES, |i| entries[i].as_str());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let picked = menu::combo(&mut child, theme, id, area.size(), text, &entries, current)?;
    Some(AttrEdit::Lineweight(match picked {
        0 => Attr::ByLayer,
        1 => Attr::ByBlock,
        i => Attr::Set(Lineweight::STANDARD[i - 2]),
    }))
}

/// The objects' own linetype scale, typed in. Not the drawing's `LTSCALE`.
fn scale_row(ui: &mut Ui, theme: &Theme, objects: &[&Object]) -> Option<AttrEdit> {
    let (_, cell) = row_frame(ui, theme, "Linetype scale");
    let scale =
        shared(objects.iter().map(|o| o.attrs.linetype_scale.to_bits())).map(f64::from_bits);
    let shown = scale.map_or(String::new(), |s| format!("{s:.4}"));
    let id = ui.id().with("attr_linetype_scale");
    let typed = text_cell(ui, theme, cell, id, &shown)?;
    let value = typed.trim().parse::<f64>().ok()?;
    (value.is_finite() && value > 0.0).then_some(AttrEdit::LinetypeScale(value))
}

/// Linetype Generation, for polylines and rectangles among the objects.
fn generation_row(ui: &mut Ui, theme: &Theme, objects: &[&Object]) -> Option<AttrEdit> {
    let outlined: Vec<&&Object> = objects.iter().filter(|o| has_outline(&o.entity)).collect();
    if outlined.is_empty() {
        return None;
    }
    let (_, cell) = row_frame(ui, theme, "Linetype generation");
    let current = shared(outlined.iter().map(|o| o.attrs.continuous_generation)).map(usize::from);
    let entries = ["Disabled".to_owned(), "Enabled".to_owned()];
    let text = current.map_or(VARIES, |i| entries[i].as_str());
    let id = ui.id().with("attr_linetype_generation");
    let picked = cell_combo(ui, theme, id, cell, text, &entries, current)?;
    Some(AttrEdit::LinetypeGeneration(picked == 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::Color;

    #[test]
    fn shared_value_needs_every_item_to_agree() {
        assert_eq!(shared([3, 3, 3].into_iter()), Some(3));
        assert_eq!(shared([3, 4].into_iter()), None);
        assert_eq!(shared(std::iter::empty::<i32>()), None);
    }

    #[test]
    fn colour_attrs_compare_for_sharing() {
        let red = Attr::Set(Color::Rgb(255, 0, 0));
        assert_eq!(shared([red.clone(), red.clone()].into_iter()), Some(red));
        assert_eq!(
            shared([Attr::ByLayer, Attr::Set(Color::Foreground)].into_iter()),
            None
        );
    }
}
