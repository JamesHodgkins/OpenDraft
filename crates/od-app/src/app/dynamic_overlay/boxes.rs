//! Painting the dynamic-input boxes: the prompt, a field, the keyword entry,
//! the options menu and the error note. Each takes a rectangle already chosen
//! by `layout` and the state it edits.
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke};
use od_commands::Prompt as Ask;
use od_core::Units;
use od_ui::icons;

use super::layout::{GAP, HEIGHT};
use crate::app::dynamic_boxes::{area, box_fill, dynamic_filter};
use crate::app::dynamic_input::field_id;
use crate::cmdline::{CmdOption, Prompt};
use crate::dynamic_input::{avoid, shown_text, DynamicInputState, Field};
use crate::dynamic_settings::DynamicInputSettings;

/// The colours and font every box is drawn with.
pub(super) struct Look {
    pub font: FontId,
    /// The fill of an active box and of a menu.
    pub light: Color32,
    pub grey: Color32,
    pub ink: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
}

/// The prompt line, with the button that opens the options menu.
pub(super) fn prompt_box(
    ctx: &egui::Context,
    uid: u64,
    input: &mut DynamicInputState,
    look: &Look,
    prompt: &Prompt,
    text: &str,
    rect: Rect,
) {
    let (grey, ink) = (look.grey, look.ink);
    area(ctx, (uid, "prompt"), rect, |ui, rect| {
        box_fill(ui, rect, grey, ink);
        let text_rect = rect.shrink2(egui::vec2(5.0, 2.0));
        let arrow_width = if prompt.options.is_empty() { 0.0 } else { 24.0 };
        ui.put(
            Rect::from_min_max(text_rect.min, text_rect.max - egui::vec2(arrow_width, 0.0)),
            egui::Label::new(egui::RichText::new(text).font(look.font.clone()).color(ink))
                .truncate(),
        );
        if !prompt.options.is_empty() {
            let button = ui
                .put(
                    Rect::from_min_max(egui::pos2(rect.right() - 24.0, rect.top()), rect.max),
                    od_ui::icon_set::Icon::new(icons::DYNAMIC_INPUT_OPTIONS, 14.0)
                        .clickable()
                        .color(ink),
                )
                .on_hover_text("Command options");
            if button.clicked() {
                input.menu = !input.menu;
                ui.memory_mut(|m| {
                    if let Some(field) = input.active {
                        m.request_focus(field_id(uid, field));
                    } else {
                        m.stop_text_input();
                    }
                });
            }
        }
    });
}

/// What a field box needs to know besides its state.
pub(super) struct FieldView<'a> {
    pub ask: &'a Ask,
    pub settings: &'a DynamicInputSettings,
    pub units: &'a Units,
    /// The field takes the typing.
    pub active: bool,
    /// Nothing is being asked: the box shows values but takes no clicks.
    pub idle: bool,
}

/// One field: its value, or a text box while it is being typed in.
pub(super) fn field_box(
    ctx: &egui::Context,
    uid: u64,
    input: &mut DynamicInputState,
    look: &Look,
    view: &FieldView,
    field: Field,
    rect: Rect,
) {
    let FieldView {
        ask,
        settings,
        units,
        active,
        idle,
    } = *view;
    let (light, grey, ink) = (look.light, look.grey, look.ink);
    let locked = input.locks.contains_key(&field);
    let live = input.live_text(field, ask, settings, units);
    let slot = usize::from(matches!(field, Field::Y | Field::Angle));
    area(ctx, (uid, slot), rect, |ui, rect| {
        box_fill(
            ui,
            rect,
            if active { light } else { grey },
            if active && input.error.is_some() {
                ui.visuals().error_fg_color
            } else if active {
                look.accent
            } else {
                ink
            },
        );
        ui.visuals_mut().override_text_color = Some(ink);
        ui.visuals_mut().selection.bg_fill = look.accent;
        ui.visuals_mut().selection.stroke.color = look.on_accent;
        ui.visuals_mut().text_cursor.stroke.color = ink;
        if active && input.editing {
            let id = field_id(uid, field);
            if std::mem::take(&mut input.request_focus) {
                ui.memory_mut(|m| m.request_focus(id));
            }
            let response = ui.put(
                rect.shrink2(egui::vec2(3.0, 1.0)),
                egui::TextEdit::singleline(&mut input.draft)
                    .id(id)
                    .frame(false)
                    .font(look.font.clone())
                    .text_color(ink)
                    .hint_text(egui::RichText::new(&live).color(ink))
                    .margin(egui::vec2(1.0, 2.0)),
            );
            if response.changed() {
                input.error = None;
            }
            ui.memory_mut(|m| m.set_focus_lock_filter(id, dynamic_filter()));
        } else {
            let text = shown_text(field, &live, units);
            ui.painter().text(
                rect.left_center() + egui::vec2(5.0, 0.0),
                Align2::LEFT_CENTER,
                text,
                look.font.clone(),
                ink,
            );
            if locked {
                ui.put(
                    Rect::from_min_size(
                        rect.right_top() - egui::vec2(18.0, 0.0),
                        egui::vec2(16.0, HEIGHT),
                    ),
                    od_ui::icon_set::Icon::new(icons::DYNAMIC_INPUT_LOCKED, 11.0).color(ink),
                );
            }
            let response = ui.interact(rect, field_id(uid, field), Sense::click());
            if response.clicked() && !idle {
                input.begin(field);
                input.draft = live.clone();
                ui.memory_mut(|m| m.request_focus(field_id(uid, field)));
                let mut state =
                    egui::TextEdit::load_state(ui.ctx(), field_id(uid, field)).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(live.chars().count()),
                    )));
                state.store(ui.ctx(), field_id(uid, field));
            }
        }
    });
}

/// Where the pointer is and what to keep clear of, for the boxes that avoid it.
#[derive(Clone, Copy)]
pub(super) struct Around {
    pub canvas: Rect,
    pub anchor: egui::Pos2,
    pub clearance: Rect,
    /// The prompt and field row.
    pub row: Rect,
}

/// Keyword entry, hidden until typing begins at an object or options prompt.
pub(super) fn keyword_box(
    ctx: &egui::Context,
    uid: u64,
    input: &mut DynamicInputState,
    look: &Look,
    around: Around,
) {
    let Around {
        canvas,
        anchor,
        clearance,
        row,
    } = around;
    let rect = input.placements.entry(Field::Keyword).or_default().place(
        anchor + egui::vec2(0.0, HEIGHT + GAP),
        egui::vec2(144.0, HEIGHT),
        canvas,
    );
    let rect = avoid(rect, canvas, &[clearance, row.expand(GAP)]);
    input.hit_rects.push(rect);
    area(ctx, (uid, Field::Keyword), rect, |ui, rect| {
        box_fill(ui, rect, look.light, look.accent);
        let id = field_id(uid, Field::Keyword);
        ui.memory_mut(|m| m.request_focus(id));
        ui.put(
            rect.shrink(2.0),
            egui::TextEdit::singleline(&mut input.draft)
                .id(id)
                .frame(false)
                .font(look.font.clone())
                .text_color(look.ink),
        );
        ui.memory_mut(|m| m.set_focus_lock_filter(id, dynamic_filter()));
    });
}

/// The prompt box, for the menu to open under.
#[derive(Clone, Copy)]
pub(super) struct PromptBox {
    pub rect: Rect,
    pub width: f32,
}

/// The list of command options under the prompt; returns the one clicked.
pub(super) fn options_menu(
    ctx: &egui::Context,
    uid: u64,
    input: &mut DynamicInputState,
    look: &Look,
    prompt: &Prompt,
    under: PromptBox,
    around: Around,
) -> Option<CmdOption> {
    let (light, grey, ink) = (look.light, look.grey, look.ink);
    let rect = input.prompt_placement.menu(
        under.rect,
        egui::vec2(
            under.width.max(150.0),
            HEIGHT * prompt.options.len().min(8) as f32,
        ),
        around.canvas,
    );
    let mut obstacles = input.hit_rects.clone();
    obstacles.push(around.clearance);
    let rect = avoid(rect, around.canvas, &obstacles);
    input.hit_rects.push(rect);
    let mut chosen = None;
    area(ctx, (uid, "options"), rect, |ui, rect| {
        box_fill(ui, rect, light, ink);
        egui::ScrollArea::vertical()
            .max_height(rect.height())
            .show(ui, |ui| {
                for (index, option) in prompt.options.iter().enumerate() {
                    let (row, response) =
                        ui.allocate_exact_size(egui::vec2(rect.width(), HEIGHT), Sense::click());
                    if response.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                        input.selected = index;
                    }
                    if input.selected == index {
                        ui.painter().rect_filled(row, 0.0, grey);
                        ui.painter().circle_filled(
                            row.left_center() + egui::vec2(9.0, 0.0),
                            2.5,
                            ink,
                        );
                    }
                    ui.painter().text(
                        row.left_center() + egui::vec2(19.0, 0.0),
                        Align2::LEFT_CENTER,
                        &option.label,
                        look.font.clone(),
                        ink,
                    );
                    ui.painter()
                        .hline(row.x_range(), row.bottom(), Stroke::new(1.0_f32, grey));
                    if response.clicked() {
                        chosen = Some(option.clone());
                    }
                }
            });
    });
    chosen
}

/// The note under the prompt that says why the last entry was refused.
pub(super) fn error_box(
    ctx: &egui::Context,
    uid: u64,
    input: &mut DynamicInputState,
    look: &Look,
    error: &str,
    around: Around,
) {
    let rect =
        input
            .prompt_placement
            .menu(around.row, egui::vec2(320.0, HEIGHT * 2.0), around.canvas);
    let mut obstacles = input.hit_rects.clone();
    obstacles.push(around.clearance);
    let rect = avoid(rect, around.canvas, &obstacles);
    input.hit_rects.push(rect);
    area(ctx, (uid, "error"), rect, |ui, rect| {
        box_fill(ui, rect, look.light, look.accent);
        ui.put(
            rect.shrink(4.0),
            egui::Label::new(
                egui::RichText::new(error)
                    .font(look.font.clone())
                    .color(look.ink),
            )
            .wrap(),
        );
    });
}
