//! The Layers panel: a table of the drawing's layers with their switches,
//! colour, linetype and lineweight. Reports what the user asked for; the
//! application state does it.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui};
use od_core::{Document, Layer, LayerId, Lineweight, Viewport};
use od_ui::{icons, menu, widgets, Theme};

use crate::colour_ui::{self, Pick};
use crate::layer_cells::{cell_text, fit_text};
use crate::layers::{LayerEdit, LayerFlag};
use crate::linetype_ui;

const ROW_HEIGHT: f32 = 22.0;
const NAME_MIN: f32 = 90.0;
const STATUS_W: f32 = 22.0;
const SWITCH_W: f32 = 26.0;
const COLOR_W: f32 = 44.0;
const LINETYPE_W: f32 = 92.0;
const WEIGHT_W: f32 = 78.0;
/// Columns every row has (and the VP Freeze one while a viewport is the target);
/// linetype and weight only appear when there is room.
fn core_width(vp: bool) -> f32 {
    STATUS_W + SWITCH_W * if vp { 4.0 } else { 3.0 } + COLOR_W
}
const EXTRA_W: f32 = LINETYPE_W + WEIGHT_W;

/// `viewport` is the viewport being worked on in a layout, if any: it adds a
/// VP Freeze column that freezes layers in that viewport only.
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    viewport: Option<&Viewport>,
) -> Option<LayerEdit> {
    let selected_id = ui.make_persistent_id("layers_selected");
    let selected: Option<LayerId> = ui
        .data(|d| d.get_temp(selected_id))
        .filter(|&id| document.layer(id).is_some());
    let mut edit = None;

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(6.0);
        if widgets::secondary_button(ui, theme, "New")
            .on_hover_text("Create a layer")
            .clicked()
        {
            edit = Some(LayerEdit::New);
        }
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if widgets::secondary_button(ui, theme, "Delete")
                .on_hover_text("Delete the selected layer")
                .clicked()
            {
                edit = selected.map(LayerEdit::Delete);
            }
            if widgets::secondary_button(ui, theme, "Set Current")
                .on_hover_text("Draw new objects on the selected layer")
                .clicked()
            {
                edit = selected.map(LayerEdit::SetCurrent);
            }
        });
    });
    ui.add_space(4.0);

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let core = core_width(viewport.is_some());
            let wide = ui.available_width() >= NAME_MIN + core + EXTRA_W;
            let width = ui.available_width().max(NAME_MIN + core);
            let shape = Layout {
                width,
                wide,
                vp: viewport.is_some(),
            };
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            header(ui, theme, shape);
            for layer in document.layers() {
                let current = layer.id == document.current_layer();
                let in_use = document.layer_in_use(layer.id);
                let picked = row(
                    ui,
                    theme,
                    document,
                    shape,
                    layer,
                    RowState {
                        current,
                        in_use,
                        selected: selected == Some(layer.id),
                        vp_frozen: viewport.map(|v| v.is_frozen(layer.id)),
                    },
                );
                if let Some(action) = picked.edit {
                    edit = Some(action);
                }
                if picked.select {
                    ui.data_mut(|d| d.insert_temp(selected_id, layer.id));
                }
            }
        });
    edit
}

/// How wide a row is and whether the extra columns fit.
#[derive(Clone, Copy)]
struct Layout {
    width: f32,
    wide: bool,
    /// The VP Freeze column is shown.
    vp: bool,
}

/// Column rectangles for a row starting at `origin`. Hidden columns are empty.
struct Columns {
    status: Rect,
    name: Rect,
    on: Rect,
    freeze: Rect,
    /// Zero width unless a viewport is the target.
    vp_freeze: Rect,
    lock: Rect,
    color: Rect,
    linetype: Rect,
    weight: Rect,
}

impl Columns {
    fn new(origin: egui::Pos2, layout: Layout) -> Self {
        let extra = if layout.wide { EXTRA_W } else { 0.0 };
        let name_w = layout.width - core_width(layout.vp) - extra;
        let mut x = origin.x;
        let mut next = |w: f32| {
            let rect = Rect::from_min_size(egui::pos2(x, origin.y), egui::vec2(w, ROW_HEIGHT));
            x += w;
            rect
        };
        Self {
            status: next(STATUS_W),
            name: next(name_w),
            on: next(SWITCH_W),
            freeze: next(SWITCH_W),
            vp_freeze: next(if layout.vp { SWITCH_W } else { 0.0 }),
            lock: next(SWITCH_W),
            color: next(COLOR_W),
            linetype: next(if layout.wide { LINETYPE_W } else { 0.0 }),
            weight: next(if layout.wide { WEIGHT_W } else { 0.0 }),
        }
    }
}

fn header(ui: &mut Ui, theme: &Theme, shape: Layout) {
    let (width, wide) = (shape.width, shape.wide);
    let c = &theme.colors;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c.panel_raised);
    let cols = Columns::new(rect.min, shape);
    let text = |rect: Rect, label: &str| {
        ui.painter().text(
            rect.left_center() + egui::vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(theme.font_size_small),
            c.text_weak,
        );
    };
    let glyph = |rect: Rect, icon: char| {
        od_ui::icon_set::paint(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            13.0,
            c.text_weak,
        );
    };
    text(cols.name, "Name");
    glyph(cols.on, icons::LAYER_PANEL_ON);
    glyph(cols.freeze, icons::LAYER_PANEL_FROZEN);
    if shape.vp {
        ui.painter().text(
            cols.vp_freeze.center(),
            Align2::CENTER_CENTER,
            "VP",
            FontId::proportional(theme.font_size_small),
            c.text_weak,
        );
    }
    glyph(cols.lock, icons::LAYER_PANEL_LOCKED);
    text(cols.color, "Color");
    if wide {
        text(cols.linetype, "Linetype");
        text(cols.weight, "Weight");
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.divider),
    );
}

/// What a row reported.
#[derive(Default)]
struct RowResult {
    edit: Option<LayerEdit>,
    /// The row itself was clicked (select it).
    select: bool,
}

/// What a row shows besides the layer's own settings.
struct RowState {
    current: bool,
    in_use: bool,
    selected: bool,
    /// Whether the layer is frozen in the target viewport (`None`: no column).
    vp_frozen: Option<bool>,
}

fn row(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    layout: Layout,
    layer: &Layer,
    state: RowState,
) -> RowResult {
    let RowState {
        current,
        in_use,
        selected,
        vp_frozen,
    } = state;
    let c = &theme.colors;
    let mut result = RowResult::default();
    let (rect, background) =
        ui.allocate_exact_size(egui::vec2(layout.width, ROW_HEIGHT), Sense::click());
    let cols = Columns::new(rect.min, layout);
    let id = ui.make_persistent_id(("layer", layer.id));

    if selected {
        ui.painter()
            .rect_filled(rect, 0.0, c.accent.gamma_multiply(0.3));
    } else if background.hovered() {
        ui.painter().rect_filled(rect, 0.0, c.hover);
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.divider),
    );
    if background.clicked() || background.double_clicked() {
        result.select = true;
    }

    // Layers with objects on them read stronger than empty ones.
    let dim = !layer.is_visible();
    let name_color = if dim || !in_use { c.text_weak } else { c.text };

    // Status: a tick on the current layer. Click to make a layer current.
    let status = ui.interact(cols.status, id.with("status"), Sense::click());
    if current {
        od_ui::icon_set::paint(
            ui.painter(),
            cols.status.center(),
            Align2::CENTER_CENTER,
            icons::LAYER_PANEL_CURRENT,
            13.0,
            c.accent_text,
        );
    }
    if status.on_hover_text("Current layer").clicked() {
        result.edit = Some(LayerEdit::SetCurrent(layer.id));
    }

    // Name: double-click to rename.
    let editing_id = id.with("renaming");
    let editing = ui.data(|d| d.get_temp::<bool>(editing_id)).unwrap_or(false);
    if editing {
        let mut text = ui
            .data(|d| d.get_temp::<String>(editing_id.with("text")))
            .unwrap_or_else(|| layer.name.clone());
        let field = ui.put(
            cols.name.shrink2(egui::vec2(2.0, 1.0)),
            egui::TextEdit::singleline(&mut text).id(editing_id.with("edit")),
        );
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
        }
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if field.lost_focus() {
            if !escape {
                result.edit = Some(LayerEdit::Rename(layer.id, text.clone()));
            }
            ui.data_mut(|d| d.insert_temp(editing_id, false));
        } else {
            ui.data_mut(|d| d.insert_temp(editing_id.with("text"), text));
        }
    } else {
        let cut = fit_text(ui, theme, cols.name, &layer.name, name_color);
        let mut name = ui.interact(cols.name, id.with("name"), Sense::click());
        if cut {
            name = name.on_hover_text(&layer.name);
        }
        if name.clicked() {
            result.select = true;
        }
        if name.double_clicked() && layer.id != LayerId::DEFAULT {
            ui.data_mut(|d| {
                d.insert_temp(editing_id, true);
                d.insert_temp(editing_id.with("text"), layer.name.clone());
            });
        }
    }

    // Switches.
    let switch = |ui: &mut Ui, cell: Rect, name: &str, icon: char, lit: bool, tip: &str| {
        let response = ui.interact(cell, id.with(name), Sense::click());
        let color = if lit {
            c.text
        } else if response.hovered() {
            c.text_weak
        } else {
            c.text_disabled
        };
        od_ui::icon_set::paint(
            ui.painter(),
            cell.center(),
            Align2::CENTER_CENTER,
            icon,
            14.0,
            color,
        );
        response.on_hover_text(tip).clicked()
    };
    let (on_icon, on_tip) = if layer.on {
        (icons::LAYER_PANEL_ON, "On: click to turn off")
    } else {
        (icons::LAYER_PANEL_OFF, "Off: click to turn on")
    };
    if switch(ui, cols.on, "on", on_icon, layer.on, on_tip) {
        result.edit = Some(LayerEdit::Toggle(layer.id, LayerFlag::On));
    }
    if switch(
        ui,
        cols.freeze,
        "freeze",
        icons::LAYER_PANEL_FROZEN,
        layer.frozen,
        if layer.frozen {
            "Frozen: click to thaw"
        } else {
            "Click to freeze"
        },
    ) {
        result.edit = Some(LayerEdit::Toggle(layer.id, LayerFlag::Frozen));
    }
    if let Some(frozen) = vp_frozen {
        let tip = if frozen {
            "Frozen in this viewport: click to thaw"
        } else {
            "Click to freeze in this viewport only"
        };
        if switch(
            ui,
            cols.vp_freeze,
            "vp_freeze",
            icons::LAYER_PANEL_VP_FROZEN,
            frozen,
            tip,
        ) {
            result.edit = Some(LayerEdit::ToggleInViewport(layer.id));
        }
    }
    let (lock_icon, lock_tip) = if layer.locked {
        (icons::LAYER_PANEL_LOCKED, "Locked: click to unlock")
    } else {
        (icons::LAYER_PANEL_UNLOCKED, "Unlocked: click to lock")
    };
    if switch(ui, cols.lock, "lock", lock_icon, layer.locked, lock_tip) {
        result.edit = Some(LayerEdit::Toggle(layer.id, LayerFlag::Locked));
    }

    // Colour.
    let mut color_ui = ui.new_child(egui::UiBuilder::new().max_rect(cols.color));
    if let Some(Pick::Color(color)) =
        colour_ui::swatch_button(&mut color_ui, theme, id.with("color"), layer.color, false)
    {
        result.edit = Some(LayerEdit::SetColor(layer.id, color));
    }

    // Linetype and lineweight: flat text cells that open a list.
    if !layout.wide {
        return result;
    }
    let cell = ui.interact(cols.linetype, id.with("linetype"), Sense::click());
    cell_text(ui, theme, cols.linetype, &layer.linetype);
    let menu_id = id.with("linetype_menu");
    if let Some(name) = linetype_ui::pick(ui, theme, menu_id, &cell, document, &layer.linetype) {
        result.edit = Some(LayerEdit::SetLinetype(layer.id, name));
    }

    let weights: Vec<String> = Lineweight::STANDARD
        .iter()
        .map(|w| weight_label(*w))
        .collect();
    let current_weight = Lineweight::STANDARD
        .iter()
        .position(|w| *w == layer.lineweight);
    let cell = ui.interact(cols.weight, id.with("weight"), Sense::click());
    cell_text(ui, theme, cols.weight, &weight_label(layer.lineweight));
    if let Some(i) = menu::list_menu(
        ui,
        theme,
        id.with("weight_menu"),
        &cell,
        &weights,
        current_weight,
    ) {
        result.edit = Some(LayerEdit::SetLineweight(layer.id, Lineweight::STANDARD[i]));
    }
    result
}

/// "0.25 mm".
pub fn weight_label(weight: Lineweight) -> String {
    format!("{:.2} mm", weight.mm())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_tile_the_row_without_gaps() {
        let width = 480.0;
        let cols = Columns::new(
            egui::pos2(10.0, 0.0),
            Layout {
                width,
                wide: true,
                vp: true,
            },
        );
        assert_eq!(cols.status.left(), 10.0);
        assert_eq!(cols.status.right(), cols.name.left());
        assert_eq!(cols.on.right(), cols.freeze.left());
        assert_eq!(cols.freeze.right(), cols.vp_freeze.left());
        assert_eq!(cols.vp_freeze.right(), cols.lock.left());
        assert_eq!(cols.weight.right(), 10.0 + width);
    }

    #[test]
    fn narrow_rows_drop_the_extra_columns() {
        let cols = Columns::new(
            egui::pos2(0.0, 0.0),
            Layout {
                width: 260.0,
                wide: false,
                vp: false,
            },
        );
        assert_eq!(cols.vp_freeze.width(), 0.0, "hidden without a viewport");
        assert_eq!(cols.linetype.width(), 0.0);
        assert_eq!(cols.weight.width(), 0.0);
        assert_eq!(cols.color.right(), 260.0);
    }

    #[test]
    fn weights_show_in_millimetres() {
        assert_eq!(weight_label(Lineweight(25)), "0.25 mm");
        assert_eq!(weight_label(Lineweight(211)), "2.11 mm");
    }
}
