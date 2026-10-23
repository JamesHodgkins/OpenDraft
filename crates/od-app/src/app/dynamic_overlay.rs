//! Compact drafting tooltips, sharing stable field IDs across moving areas.
use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke};
use od_commands::Wants;
use od_ui::icons;

use super::dynamic_boxes::{area, box_fill, dynamic_filter};
use super::dynamic_input::{field_id, owns_focus};
use super::panels::PanelViewer;
use crate::dynamic_input::{avoid, cursor_clearance, dimensions, shown_text, Field};

const HEIGHT: f32 = 24.0;
const GAP: f32 = 4.0;

impl PanelViewer<'_> {
    pub(super) fn dynamic_overlay(&mut self, ui: &mut egui::Ui) {
        let settings = self.state.settings.dynamic_input.clone();
        if !settings.enabled()
            || self.dynamic_suppressed
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || self.state.mtext_editing()
        {
            self.state.docs[self.state.active]
                .dynamic_input
                .hit_rects
                .clear();
            return;
        }
        let idle = self.state.dynamic_ask().is_none();
        let Some(ask) = self
            .state
            .dynamic_ask()
            .or_else(|| self.state.idle_pointer_ask())
        else {
            return;
        };
        let prompt = self.state.cmd_prompt();
        let units = self.state.units();
        let readonly = ui.ctx().wants_keyboard_input() && !owns_focus(ui.ctx(), self.state);
        let doc = &mut self.state.docs[self.state.active];
        let Some(canvas) = doc.dynamic_input.canvas else {
            return;
        };
        let Some(cursor) = doc.dynamic_input.cursor else {
            return;
        };
        let anchor = doc.view.camera.world_to_screen(cursor, canvas);
        let clearance = cursor_clearance(anchor);
        let fields = doc.dynamic_input.fields(&ask, &settings);
        let fields = if ask.wants == Wants::Point
            && settings.pointer_visibility == 0
            && !(settings.dimensions()
                && ask.dynamic_dimensions != od_commands::DynamicDimensions::None)
            && !doc.dynamic_input.editing
            && doc.dynamic_input.locks.is_empty()
        {
            Vec::new()
        } else {
            fields
        };
        let show_prompt = settings.dynamic_prompts && !idle;
        let font = FontId::proportional(self.theme.font_size_small);
        let prompt_text = if prompt.command.is_empty() {
            prompt.text.to_string()
        } else {
            format!("{} {}", prompt.command, prompt.text)
        };
        let prompt_width = if show_prompt {
            (ui.painter()
                .layout_no_wrap(
                    prompt_text.clone(),
                    font.clone(),
                    self.theme.colors.canvas_bg,
                )
                .size()
                .x
                + if prompt.options.is_empty() {
                    16.0
                } else {
                    38.0
                })
            .clamp(140.0, 340.0)
            .min((canvas.width() - 16.0).max(1.0))
        } else {
            0.0
        };
        let field_width = |field| {
            if matches!(field, Field::Coordinate | Field::Text | Field::Keyword) {
                144.0
            } else {
                dimensions::BOX.x
            }
        };
        let relative = ask.base.is_some() && !settings.pointer_absolute;
        // Dimensions follow the point the answer would give, so a typed or
        // locked length shortens them with the line.
        let end = doc
            .dynamic_input
            .constrained_point(&ask, &settings, &units)
            .map_or(anchor, |p| doc.view.camera.world_to_screen(p, canvas));
        let ghost = ask.base.and_then(|base| {
            let base = doc.view.camera.world_to_screen(base, canvas);
            match fields.as_slice() {
                [Field::Distance, Field::Angle] => dimensions::polar(
                    base,
                    end,
                    -(units.base_angle as f32),
                    if units.clockwise { 1.0 } else { -1.0 },
                ),
                [Field::Distance] => dimensions::linear(base, end),
                [Field::X, Field::Y] if relative => dimensions::cartesian(base, end),
                _ => None,
            }
        });
        // The line being drawn is an obstacle too, as a trail of small squares.
        let rubberband: Vec<Rect> = match (&ghost, ask.base) {
            (Some(_), Some(base)) => {
                let base = doc.view.camera.world_to_screen(base, canvas);
                let steps = (((end - base).length() / 16.0).ceil() as usize).clamp(1, 48);
                (0..=steps)
                    .map(|i| {
                        let at = base + (end - base) * (i as f32 / steps as f32);
                        Rect::from_center_size(at, egui::Vec2::splat(20.0))
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        // Fields on ghost dimensions are seated there, not in the prompt row.
        let total_fields: f32 = if ghost.is_some() {
            0.0
        } else {
            fields.iter().map(|f| field_width(*f) + GAP).sum()
        };
        let wrap = show_prompt && prompt_width + total_fields > canvas.width() - 16.0;
        let row_size = if wrap {
            egui::vec2(prompt_width.max(total_fields), HEIGHT * 2.0 + GAP)
        } else {
            egui::vec2(prompt_width + total_fields, HEIGHT)
        };
        let row = avoid(
            doc.dynamic_input
                .prompt_placement
                .place(anchor, row_size, canvas),
            canvas,
            &[clearance],
        );
        let prompt_rect = Rect::from_min_size(row.min, egui::vec2(prompt_width, HEIGHT));
        let mut rects: Vec<(Field, Rect)> = Vec::new();
        let mut x = if wrap || !show_prompt {
            row.left()
        } else {
            prompt_rect.right() + GAP
        };
        for field in &fields {
            let seat = ghost
                .as_ref()
                .and_then(|g| g.boxes.iter().find(|b| b.0 == *field));
            let rect = if let Some((_, centre)) = seat {
                let mut obstacles = vec![clearance];
                obstacles.extend(rubberband.iter().copied());
                if show_prompt {
                    obstacles.push(prompt_rect.expand(GAP));
                }
                obstacles.extend(rects.iter().map(|(_, r)| r.expand(GAP)));
                avoid(
                    Rect::from_center_size(*centre, dimensions::BOX),
                    canvas,
                    &obstacles,
                )
            } else {
                let rect = Rect::from_min_size(
                    egui::pos2(x, row.top() + if wrap { HEIGHT + GAP } else { 0.0 }),
                    egui::vec2(field_width(*field), HEIGHT),
                );
                x += field_width(*field) + GAP;
                rect
            };
            rects.push((*field, rect));
        }
        if doc.dynamic_input.editing && fields == [Field::Coordinate] {
            doc.dynamic_input.active = Some(Field::Coordinate);
        }
        let colors = &self.theme.colors;
        let light = colors.text;
        let grey = colors.panel_raised.lerp_to_gamma(light, 0.55);
        let ink = colors.canvas_bg;
        if let Some(ghost) = &ghost {
            ghost.paint(ui.painter(), Stroke::new(1.0_f32, grey));
        }
        doc.dynamic_input.hit_rects.clear();
        if show_prompt {
            doc.dynamic_input.hit_rects.push(prompt_rect);
        }
        doc.dynamic_input
            .hit_rects
            .extend(rects.iter().map(|(_, r)| *r));
        let uid = doc.uid;
        let input = &mut doc.dynamic_input;
        if show_prompt {
            area(ui.ctx(), (uid, "prompt"), prompt_rect, |ui, rect| {
                box_fill(ui, rect, grey, ink);
                let text_rect = rect.shrink2(egui::vec2(5.0, 2.0));
                let arrow_width = if prompt.options.is_empty() { 0.0 } else { 24.0 };
                ui.put(
                    Rect::from_min_max(text_rect.min, text_rect.max - egui::vec2(arrow_width, 0.0)),
                    egui::Label::new(
                        egui::RichText::new(&prompt_text)
                            .font(font.clone())
                            .color(ink),
                    )
                    .truncate(),
                );
                if !prompt.options.is_empty() {
                    let button = ui
                        .put(
                            Rect::from_min_max(
                                egui::pos2(rect.right() - 24.0, rect.top()),
                                rect.max,
                            ),
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
        for (field, rect) in rects {
            let active = !readonly
                && !idle
                && input
                    .active
                    .unwrap_or(fields.first().copied().unwrap_or(field))
                    == field;
            let locked = input.locks.contains_key(&field);
            let live = input.live_text(field, &ask, &settings, &units);
            let slot = usize::from(matches!(field, Field::Y | Field::Angle));
            area(ui.ctx(), (uid, slot), rect, |ui, rect| {
                box_fill(
                    ui,
                    rect,
                    if active { light } else { grey },
                    if active && input.error.is_some() {
                        ui.visuals().error_fg_color
                    } else if active {
                        colors.accent
                    } else {
                        ink
                    },
                );
                ui.visuals_mut().override_text_color = Some(ink);
                ui.visuals_mut().selection.bg_fill = colors.accent;
                ui.visuals_mut().selection.stroke.color = colors.text_on_accent;
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
                            .font(font.clone())
                            .text_color(ink)
                            .hint_text(egui::RichText::new(&live).color(ink))
                            .margin(egui::vec2(1.0, 2.0)),
                    );
                    if response.changed() {
                        input.error = None;
                    }
                    ui.memory_mut(|m| m.set_focus_lock_filter(id, dynamic_filter()));
                } else {
                    let text = shown_text(field, &live, &units);
                    ui.painter().text(
                        rect.left_center() + egui::vec2(5.0, 0.0),
                        Align2::LEFT_CENTER,
                        text,
                        font.clone(),
                        ink,
                    );
                    if locked {
                        ui.put(
                            Rect::from_min_size(
                                rect.right_top() - egui::vec2(18.0, 0.0),
                                egui::vec2(16.0, HEIGHT),
                            ),
                            od_ui::icon_set::Icon::new(icons::DYNAMIC_INPUT_LOCKED, 11.0)
                                .color(ink),
                        );
                    }
                    let response = ui.interact(rect, field_id(uid, field), Sense::click());
                    if response.clicked() && !idle {
                        input.begin(field);
                        input.draft = live.clone();
                        ui.memory_mut(|m| m.request_focus(field_id(uid, field)));
                        {
                            let mut state =
                                egui::TextEdit::load_state(ui.ctx(), field_id(uid, field))
                                    .unwrap_or_default();
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(live.chars().count()),
                                )));
                            state.store(ui.ctx(), field_id(uid, field));
                        }
                    }
                }
            });
        }
        // Keyword entry is hidden until typing begins at an object/options prompt.
        if input.editing && fields.is_empty() {
            let rect = input.placements.entry(Field::Keyword).or_default().place(
                anchor + egui::vec2(0.0, HEIGHT + GAP),
                egui::vec2(144.0, HEIGHT),
                canvas,
            );
            let rect = avoid(rect, canvas, &[clearance, row.expand(GAP)]);
            input.hit_rects.push(rect);
            area(ui.ctx(), (uid, Field::Keyword), rect, |ui, rect| {
                box_fill(ui, rect, light, colors.accent);
                let id = field_id(uid, Field::Keyword);
                ui.memory_mut(|m| m.request_focus(id));
                ui.put(
                    rect.shrink(2.0),
                    egui::TextEdit::singleline(&mut input.draft)
                        .id(id)
                        .frame(false)
                        .font(font.clone())
                        .text_color(ink),
                );
                ui.memory_mut(|m| m.set_focus_lock_filter(id, dynamic_filter()));
            });
        }
        let mut chosen = None;
        if input.menu && !prompt.options.is_empty() {
            let rect = input.prompt_placement.menu(
                prompt_rect,
                egui::vec2(
                    prompt_width.max(150.0),
                    HEIGHT * prompt.options.len().min(8) as f32,
                ),
                canvas,
            );
            let mut obstacles = input.hit_rects.clone();
            obstacles.push(clearance);
            let rect = avoid(rect, canvas, &obstacles);
            input.hit_rects.push(rect);
            area(ui.ctx(), (uid, "options"), rect, |ui, rect| {
                box_fill(ui, rect, light, ink);
                egui::ScrollArea::vertical()
                    .max_height(rect.height())
                    .show(ui, |ui| {
                        for (index, option) in prompt.options.iter().enumerate() {
                            let (row, response) = ui.allocate_exact_size(
                                egui::vec2(rect.width(), HEIGHT),
                                Sense::click(),
                            );
                            if response.hovered()
                                && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
                            {
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
                                font.clone(),
                                ink,
                            );
                            ui.painter().hline(
                                row.x_range(),
                                row.bottom(),
                                Stroke::new(1.0_f32, grey),
                            );
                            if response.clicked() {
                                chosen = Some(option.clone());
                            }
                        }
                    });
            });
        }
        if let Some(error) = input.error.as_ref().filter(|_| !input.menu) {
            let rect = input
                .prompt_placement
                .menu(row, egui::vec2(320.0, HEIGHT * 2.0), canvas);
            let mut obstacles = input.hit_rects.clone();
            obstacles.push(clearance);
            let rect = avoid(rect, canvas, &obstacles);
            input.hit_rects.push(rect);
            area(ui.ctx(), (uid, "error"), rect, |ui, rect| {
                box_fill(ui, rect, light, colors.accent);
                ui.put(
                    rect.shrink(4.0),
                    egui::Label::new(egui::RichText::new(error).font(font).color(ink)).wrap(),
                );
            });
        }
        if let Some(option) = chosen {
            self.state.cmd_choose(option);
        }
    }
}
