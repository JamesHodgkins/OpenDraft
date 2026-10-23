//! The controls the Home tab draws in its ribbon slots, as in AutoCAD: the
//! layer list with its light bulb, sun and lock switches, and the colour,
//! lineweight and linetype lists. They show the selected objects' values and
//! change them. With nothing selected the layer list shows and sets the
//! current layer, and the other lists show and set the colour, linetype and
//! lineweight new objects are drawn with (ByLayer unless set).

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Ui};
use od_core::{Document, Layer, Object};
use od_ui::{icons, menu, Theme};

use crate::attr_rows::{self, shared};
use crate::colour_ui;
use crate::layers::{AttrEdit, LayerEdit, LayerFlag};
use crate::state::AppState;

pub const LAYER_LIST: &str = "layer.list";
pub const COLOR_LIST: &str = "properties.color";
pub const LINEWEIGHT_LIST: &str = "properties.lineweight";
pub const LINETYPE_LIST: &str = "properties.linetype";

/// The light bulb and sun when on, in AutoCAD's yellow.
const SWITCH_ON: Color32 = Color32::from_rgb(240, 196, 64);
const ROW_H: f32 = 22.0;
/// Width of one switch icon in the layer list.
const SWITCH_W: f32 = 20.0;

/// What a ribbon control asks for.
pub enum SlotEdit {
    Layer(LayerEdit),
    /// A change to the selected objects.
    Attr(AttrEdit),
}

impl AppState {
    pub fn apply_slot_edit(&mut self, edit: SlotEdit) {
        match edit {
            SlotEdit::Layer(edit) => self.apply_layer_edit(edit),
            SlotEdit::Attr(edit) if self.docs[self.active].selection.is_empty() => {
                self.set_new_object_style(&edit)
            }
            SlotEdit::Attr(edit) => self.set_attrs(&edit, None),
        }
    }
}

/// Draw the control of every slot the ribbon laid out; returns the edit
/// made, if any.
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    state: &AppState,
    slots: &[(&'static str, Rect)],
) -> Option<SlotEdit> {
    let doc = &state.docs[state.active];
    let document = doc.session.document();
    let selected: Vec<&Object> = doc
        .selection
        .iter()
        .filter_map(|&id| document.object(id))
        .collect();
    let mut edit = None;
    for &(slot, rect) in slots {
        let id = egui::Id::new(("ribbon_slot", slot));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        // The Home page has no drawing behind it, so the lists are inert.
        if state.home {
            child.disable();
        }
        let made = match slot {
            LAYER_LIST => layer_list(&mut child, theme, id, document, &selected),
            COLOR_LIST | LINEWEIGHT_LIST | LINETYPE_LIST => {
                property_list(&mut child, theme, id, slot, document, &selected).map(SlotEdit::Attr)
            }
            _ => None,
        };
        edit = made.or(edit);
    }
    edit
}

/// One of the three property lists, with its icon at the left.
fn property_list(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    slot: &str,
    document: &Document,
    selected: &[&Object],
) -> Option<AttrEdit> {
    let rect = ui.max_rect();
    let icon = match slot {
        COLOR_LIST => icons::PROPERTIES_COLOR,
        LINEWEIGHT_LIST => icons::PROPERTIES_LINEWEIGHT,
        _ => icons::PROPERTIES_LINETYPE,
    };
    od_ui::icon_set::paint(
        ui.painter(),
        egui::pos2(rect.left() + 9.0, rect.center().y),
        Align2::CENTER_CENTER,
        icon,
        14.0,
        theme.colors.text,
    );
    let area = Rect::from_min_max(egui::pos2(rect.left() + 22.0, rect.top()), rect.max);
    if selected.is_empty() {
        // Nothing to change: the lists show and set what new objects get.
        let (color, linetype, lineweight) = document.new_object_style();
        return match slot {
            COLOR_LIST => {
                let layer = document.layer(document.current_layer()).map(|l| l.color);
                attr_rows::color_value_field(ui, theme, id, area, Some(color.clone()), layer)
            }
            LINEWEIGHT_LIST => {
                attr_rows::lineweight_value_field(ui, theme, id, area, Some(lineweight.clone()))
            }
            _ => attr_rows::linetype_value_field(
                ui,
                theme,
                id,
                area,
                document,
                Some(linetype.clone()),
            ),
        };
    }
    match slot {
        COLOR_LIST => attr_rows::color_field(ui, theme, id, area, document, selected),
        LINEWEIGHT_LIST => attr_rows::lineweight_field(ui, theme, id, area, selected),
        _ => attr_rows::linetype_field(ui, theme, id, area, document, selected),
    }
}

/// The layer list: the selected objects' layer (blank when they differ) or
/// the current layer. Its menu lists every layer with switches that toggle
/// without closing it; picking a layer moves the selection to it, or makes
/// it current when nothing is selected.
fn layer_list(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    document: &Document,
    selected: &[&Object],
) -> Option<SlotEdit> {
    let rect = ui.max_rect();
    let response = ui.interact(rect, id.with("field"), Sense::click());
    let shown = if selected.is_empty() {
        Some(document.current_layer())
    } else {
        shared(selected.iter().map(|o| o.attrs.layer))
    };
    menu::paint_combo(ui, theme, rect, "", response.hovered());
    if let Some(layer) = shown.and_then(|id| document.layer(id)) {
        layer_row_contents(ui, theme, rect, layer, theme.colors.text);
    }

    let mut toggled = None;
    let picked = menu::popup(ui, theme, id.with("menu"), &response, rect.width(), |ui| {
        let rows = |ui: &mut Ui| {
            let mut picked = None;
            for layer in document.layers() {
                let (pick, toggle) = menu_row(ui, theme, layer, shown == Some(layer.id));
                if pick {
                    picked = Some(layer.id);
                }
                if toggle.is_some() {
                    toggled = toggle;
                }
            }
            picked
        };
        egui::ScrollArea::vertical()
            .max_height(ROW_H * 16.0)
            .show(ui, rows)
            .inner
    });
    if let Some(layer) = picked {
        return Some(if selected.is_empty() {
            SlotEdit::Layer(LayerEdit::SetCurrent(layer))
        } else {
            SlotEdit::Attr(AttrEdit::Layer(layer))
        });
    }
    toggled.map(SlotEdit::Layer)
}

/// One layer in the menu. Returns whether the layer was picked and the
/// switch toggled, if any.
fn menu_row(ui: &mut Ui, theme: &Theme, layer: &Layer, current: bool) -> (bool, Option<LayerEdit>) {
    let c = &theme.colors;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::hover());
    let switches = 3.0 * SWITCH_W + 4.0;
    let name = Rect::from_min_max(egui::pos2(rect.left() + switches, rect.top()), rect.max);
    let picked = ui.interact(name, ui.id().with(("layer_pick", layer.id)), Sense::click());
    if current {
        ui.painter().rect_filled(rect, 2.0, c.accent);
    } else if picked.hovered() {
        ui.painter().rect_filled(rect, 2.0, c.hover);
    }
    let text = if current { c.text_on_accent } else { c.text };
    let mut toggled = None;
    for (i, flag) in [LayerFlag::On, LayerFlag::Frozen, LayerFlag::Locked]
        .into_iter()
        .enumerate()
    {
        let at = rect.left() + 2.0 + i as f32 * SWITCH_W;
        let switch = Rect::from_min_size(egui::pos2(at, rect.top()), egui::vec2(SWITCH_W, ROW_H));
        let response = ui.interact(
            switch,
            ui.id().with(("layer_flag", layer.id, i)),
            Sense::click(),
        );
        if response.hovered() {
            ui.painter().rect_stroke(
                switch.shrink(1.0),
                2.0,
                Stroke::new(1.0_f32, c.border),
                egui::StrokeKind::Inside,
            );
        }
        if response.on_hover_text(switch_tip(layer, flag)).clicked() {
            toggled = Some(LayerEdit::Toggle(layer.id, flag));
        }
    }
    layer_row_contents(ui, theme, rect, layer, text);
    (picked.clicked(), toggled)
}

fn switch_tip(layer: &Layer, flag: LayerFlag) -> &'static str {
    match flag {
        LayerFlag::On if layer.on => "On: click to turn off",
        LayerFlag::On => "Off: click to turn on",
        LayerFlag::Frozen if layer.frozen => "Frozen: click to thaw",
        LayerFlag::Frozen => "Thawed: click to freeze",
        LayerFlag::Locked if layer.locked => "Locked: click to unlock",
        LayerFlag::Locked => "Unlocked: click to lock",
    }
}

/// A layer's switches, colour and name across `rect`, as AutoCAD's list.
fn layer_row_contents(ui: &Ui, theme: &Theme, rect: Rect, layer: &Layer, text: Color32) {
    let c = &theme.colors;
    let glyphs = [
        if layer.on {
            (icons::LAYER_LIST_ON, SWITCH_ON)
        } else {
            (icons::LAYER_LIST_OFF, c.text_weak)
        },
        if layer.frozen {
            (icons::LAYER_LIST_FROZEN, c.text_weak)
        } else {
            (icons::LAYER_LIST_THAWED, SWITCH_ON)
        },
        if layer.locked {
            (icons::LAYER_LIST_LOCKED, text)
        } else {
            (icons::LAYER_LIST_UNLOCKED, text)
        },
    ];
    let painter = ui.painter();
    for (i, (glyph, color)) in glyphs.into_iter().enumerate() {
        od_ui::icon_set::paint(
            painter,
            egui::pos2(
                rect.left() + 2.0 + (i as f32 + 0.5) * SWITCH_W,
                rect.center().y,
            ),
            Align2::CENTER_CENTER,
            glyph,
            13.0,
            color,
        );
    }
    let swatch = Rect::from_center_size(
        egui::pos2(rect.left() + 3.0 * SWITCH_W + 12.0, rect.center().y),
        egui::vec2(11.0, 11.0),
    );
    painter.rect_filled(swatch, 1.0, colour_ui::display(theme, layer.color));
    painter.rect_stroke(
        swatch,
        1.0,
        Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );
    // The name stops short of the field's chevron.
    let room = Rect::from_min_max(
        egui::pos2(swatch.right() + 6.0, rect.top()),
        egui::pos2(rect.right() - 22.0, rect.bottom()),
    );
    painter.with_clip_rect(room.intersect(ui.clip_rect())).text(
        room.left_center(),
        Align2::LEFT_CENTER,
        &layer.name,
        FontId::proportional(theme.font_size_body),
        text,
    );
}
