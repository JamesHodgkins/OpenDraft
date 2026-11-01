//! The Properties panel body: a dense AutoCAD-style table. A type bar on top,
//! then collapsible sections of `label | value` rows separated by hairlines.
//! Values look like plain text until hovered or focused.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui};
use od_core::{Document, Entity, EntityKind, Object, Units};
use od_ui::{icons, menu, Theme};

use crate::attr_rows;
use crate::attribute_rows;
use crate::layers::AttrEdit;
use crate::properties::{self, Field, Row};
use crate::state::AppState;
use crate::style_rows::{self, StyleEdit};

const ROW_HEIGHT: f32 = 21.0;
const HEADER_HEIGHT: f32 = 22.0;
/// Share of the table width taken by the label column.
const LABEL_SHARE: f32 = 0.46;
/// The label column never gets narrower than this, so "Circumference" fits.
const LABEL_MIN: f32 = 92.0;

/// A committed edit. `kind` limits it to one kind of entity when the type
/// filter is set.
pub enum Edit {
    /// A number typed into a geometry row.
    Value {
        field: Field,
        /// Row label, used for the undo name.
        label: &'static str,
        value: f64,
        kind: Option<EntityKind>,
    },
    /// Layer, colour, linetype or lineweight.
    Attr {
        edit: AttrEdit,
        kind: Option<EntityKind>,
    },
    /// The dimension style of dimensions or the text style of text.
    Style(StyleEdit),
    /// The value of an attribute of the selected block references.
    Attribute {
        tag: String,
        value: String,
        kind: Option<EntityKind>,
    },
}

pub fn show(ui: &mut Ui, theme: &Theme, state: &AppState) -> Option<Edit> {
    let mut edit = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let doc = &state.docs[state.active];
                    let document = &doc.session.document();
                    let selected: Vec<&Object> = doc
                        .selection
                        .iter()
                        .filter_map(|&id| document.object(id))
                        .collect();
                    edit = body(ui, theme, document, &selected);
                });
        });
    edit
}

/// An entry in the selector: `None` is "All".
type Choice = Option<EntityKind>;

fn body(ui: &mut Ui, theme: &Theme, document: &Document, selected: &[&Object]) -> Option<Edit> {
    if selected.is_empty() {
        type_bar(ui, theme, "No selection", &[], None);
        return None;
    }
    let mut counts = std::collections::BTreeMap::new();
    for o in selected {
        *counts.entry(o.entity.kind()).or_insert(0usize) += 1;
    }

    // The selector narrows the panel to one kind, as in AutoCAD. A filter
    // whose kind is no longer selected falls away.
    let filter_id = ui.make_persistent_id("property_type_filter");
    let stored = ui.data(|d| d.get_temp::<EntityKind>(filter_id));
    let filter: Choice = stored.filter(|k| counts.contains_key(k) && counts.len() > 1);
    let mixed = counts.len() > 1;

    let title = |kind: Choice| match kind {
        Some(k) => format!("{} ({})", k.title(), counts[&k]),
        None if mixed => format!("All ({})", selected.len()),
        None => match selected.len() {
            1 => selected[0].entity.kind().title().to_owned(),
            n => format!("{} ({n})", selected[0].entity.kind().title()),
        },
    };
    let options: Vec<(Choice, String)> = if mixed {
        std::iter::once((None, title(None)))
            .chain(counts.keys().map(|&k| (Some(k), title(Some(k)))))
            .collect()
    } else {
        Vec::new()
    };
    if let Some(choice) = type_bar(ui, theme, &title(filter), &options, filter) {
        ui.data_mut(|d| match choice {
            Some(k) => d.insert_temp(filter_id, k),
            None => d.remove::<EntityKind>(filter_id),
        });
    }

    let shown: Vec<&Object> = match filter {
        Some(k) => selected
            .iter()
            .copied()
            .filter(|o| o.entity.kind() == k)
            .collect(),
        None => selected.to_vec(),
    };
    let entities: Vec<&Entity> = shown.iter().map(|o| &o.entity).collect();

    let mut edit = None;
    if let Some(attr) = section(ui, theme, "General", |ui| {
        attr_rows::rows(ui, theme, document, &shown)
    }) {
        edit = Some(Edit::Attr {
            edit: attr,
            kind: filter,
        });
    }

    // Geometry rows: the shared ones when kinds are mixed.
    let rows = if mixed && filter.is_none() {
        properties::common_rows(&entities)
    } else {
        properties::rows(entities[0]).to_vec()
    };
    if !rows.is_empty() {
        let geometry = section(ui, theme, "Geometry", |ui| {
            let mut edit = None;
            for row in &rows {
                if let Some(value) = value_row(ui, theme, document.units(), &entities, row) {
                    edit = Some((row.field, row.label, value));
                }
            }
            edit
        });
        if let Some((field, label, value)) = geometry {
            edit = Some(Edit::Value {
                field,
                label,
                value,
                kind: filter,
            });
        }
    }
    // The attributes of block references, when that is all that is shown.
    let inserts: Vec<&od_core::Insert> = shown
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Insert(i) => Some(&**i),
            _ => None,
        })
        .collect();
    if inserts.len() == shown.len() {
        let rows = attribute_rows::rows(&inserts);
        if !rows.is_empty() {
            let changed = section(ui, theme, "Attributes", |ui| {
                attribute_rows::show(ui, theme, &rows)
            });
            if let Some((tag, value)) = changed {
                edit = Some(Edit::Attribute {
                    tag,
                    value,
                    kind: filter,
                });
            }
        }
    }
    // The style of dimensions or of text, when that is all that is shown.
    if let Some(kind) = style_rows::kind(&shown) {
        let styled = section(ui, theme, kind.title(), |ui| {
            style_rows::row(ui, theme, document, &shown, kind)
        });
        if let Some(styled) = styled {
            edit = Some(Edit::Style(styled));
        }
    }
    edit
}

/// The selection summary at the top, drawn like AutoCAD's object selector.
/// With `options` it opens a menu; returns the entry picked.
fn type_bar(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    options: &[(Choice, String)],
    current: Choice,
) -> Option<Choice> {
    let id = ui.make_persistent_id("property_type_menu");
    let entries: Vec<String> = options.iter().map(|(_, label)| label.clone()).collect();
    let current = options.iter().position(|(choice, _)| *choice == current);
    let size = egui::vec2(ui.available_width(), 26.0);
    let picked = menu::combo(ui, theme, id, size, title, &entries, current);
    ui.add_space(4.0);
    picked.map(|i| options[i].0)
}

/// A collapsible group: a full-width header bar, then its rows.
fn section<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    add_rows: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    let c = &theme.colors;
    let id = ui.make_persistent_id(("property_section", title));
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HEADER_HEIGHT),
        Sense::click(),
    );
    let fill = if response.hovered() {
        c.hover
    } else {
        c.panel_raised
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().text(
        rect.left_center() + egui::vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(theme.font_size_body),
        c.text,
    );
    let icon = if open {
        icons::PANEL_SECTION_OPEN
    } else {
        icons::PANEL_SECTION_CLOSED
    };
    od_ui::icon_set::paint(
        ui.painter(),
        rect.right_center() - egui::vec2(8.0, 0.0),
        Align2::RIGHT_CENTER,
        icon,
        12.0,
        c.text_weak,
    );
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(id, !open));
    }
    let edit = if open { add_rows(ui) } else { None };
    ui.add_space(4.0);
    edit
}

/// Paints the row background, hairlines and label; returns the label and
/// value cells.
pub(crate) fn row_frame(ui: &mut Ui, theme: &Theme, label: &str) -> (Rect, Rect) {
    let c = &theme.colors;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let split = rect.left()
        + (rect.width() * LABEL_SHARE)
            .max(LABEL_MIN)
            .min(rect.width() * 0.62);
    let label_cell = Rect::from_min_max(rect.min, egui::pos2(split, rect.max.y));
    let value_cell = Rect::from_min_max(egui::pos2(split, rect.min.y), rect.max);
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, c.panel_bg);
    let line = Stroke::new(1.0_f32, c.divider);
    painter.hline(rect.x_range(), rect.max.y, line);
    painter.vline(split, rect.y_range(), line);
    painter.text(
        label_cell.left_center() + egui::vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(theme.font_size_body),
        c.text,
    );
    (label_cell, value_cell)
}

/// One property row. Returns the new value when an edit is committed (Enter
/// or clicking away); Esc discards it. Entities that disagree show "Varies"
/// and typing a value sets all of them.
fn value_row(
    ui: &mut Ui,
    theme: &Theme,
    units: &Units,
    entities: &[&Entity],
    row: &Row,
) -> Option<f64> {
    let shared = properties::shared_value(entities, row.field);
    let shown = shared
        .map(|v| properties::format(units, row.unit, v))
        .unwrap_or_default();
    let (label_cell, cell) = row_frame(ui, theme, row.label);
    ui.interact(
        label_cell,
        ui.id().with(("label", row.field)),
        Sense::hover(),
    )
    .on_hover_text(match row.unit.suffix(units) {
        suffix if suffix.is_empty() => row.label.to_owned(),
        suffix => format!("{} ({suffix})", row.label),
    });
    if !row.editable {
        let text = if shared.is_some() {
            shown.as_str()
        } else {
            "Varies"
        };
        ui.painter().text(
            cell.left_center() + egui::vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(theme.font_size_body),
            theme.colors.text_weak,
        );
        return None;
    }

    let id = egui::Id::new(("property", row.field));
    let committed = text_cell(ui, theme, cell, id, &shown)?;
    properties::parse(units, row.unit, &committed)
}

/// An editable text value in `cell`: flat until hovered, an input well while
/// focused. Returns the text when the edit is committed (Enter or clicking
/// away); Esc discards it.
pub(crate) fn text_cell(
    ui: &mut Ui,
    theme: &Theme,
    cell: Rect,
    id: egui::Id,
    shown: &str,
) -> Option<String> {
    let focused = ui.memory(|m| m.has_focus(id));
    // Flat until hovered, then an input well while editing.
    if focused {
        ui.painter()
            .rect_filled(cell.shrink(1.0), 0.0, theme.colors.input_bg);
        ui.painter().rect_stroke(
            cell.shrink(1.0),
            0.0,
            Stroke::new(1.0_f32, theme.colors.accent),
            egui::StrokeKind::Inside,
        );
    } else if ui.rect_contains_pointer(cell) {
        ui.painter()
            .rect_filled(cell.shrink(1.0), 0.0, theme.colors.hover);
    }
    let mut text = focused
        .then(|| ui.data(|d| d.get_temp::<String>(id)))
        .flatten()
        .unwrap_or_else(|| shown.to_owned());
    let response = ui.put(
        cell.shrink2(egui::vec2(1.0, 1.0)),
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .frame(false)
            .hint_text(egui::RichText::new("Varies").color(theme.colors.text_weak))
            .margin(egui::Margin::symmetric(6, 3))
            .vertical_align(egui::Align::Center)
            .desired_width(cell.width()),
    );
    if response.gained_focus() {
        // Select everything so typing replaces the value.
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
            let end = egui::text::CCursor::new(text.chars().count());
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    end,
                )));
            state.store(ui.ctx(), id);
        }
    }
    if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if response.lost_focus() {
        ui.data_mut(|d| d.remove_temp::<String>(id));
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if !escaped && text != shown {
            return Some(text);
        }
    }
    None
}
