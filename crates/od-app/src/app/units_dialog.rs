//! The Units dialog (UNITS): length and angle format and precision, the
//! direction of angle 0 and the insertion units. It edits a draft copy of the
//! drawing's units; OK commits it as one undo step, Cancel / Escape / X drop it.
//!
//! The Point Style dialog (DDPTYPE) is here too: twenty pictures of how a
//! point can look and the size, absolute or a share of the screen, edited the
//! same way.

use eframe::egui::{self, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use od_core::{
    units::MAX_PRECISION, AngleFormat, Entity, InsertUnits, LengthFormat, Point, PointStyle, Units,
};
use od_ui::{widgets, Theme};

use super::OpenDraftApp;
use crate::point_vars::Draft;

const WIDTH: f32 = 430.0;
const DIRECTION_WIDTH: f32 = 260.0;
/// The four compass directions of the base angle, in degrees.
const COMPASS: [(&str, f64); 4] = [
    ("East", 0.0),
    ("North", 90.0),
    ("West", 180.0),
    ("South", 270.0),
];

#[derive(Clone, Copy)]
enum Choice {
    Ok,
    Cancel,
    Direction,
}

impl OpenDraftApp {
    /// The window opened by `format.units`.
    pub(super) fn units_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_units {
            self.units_draft = None;
            self.units_direction = None;
            return;
        }
        let mut draft = self
            .units_draft
            .take()
            .unwrap_or_else(|| self.state.units());
        let (mut open, mut choice) = (true, None);
        let direction_open = self.units_direction.is_some();
        widgets::dialog(ctx, &self.theme, "Drawing Units", &mut open, WIDTH, |ui| {
            ui.add_enabled_ui(!direction_open, |ui| {
                ui.columns(2, |columns| {
                    length_box(&mut columns[0], &self.theme, &mut draft);
                    angle_box(&mut columns[1], &self.theme, &mut draft);
                });
                ui.add_space(6.0);
                insertion_box(ui, &self.theme, &mut draft);
                ui.add_space(6.0);
                widgets::group_box(ui, &self.theme, "Sample Output", |ui| {
                    for line in draft.sample_output() {
                        ui.label(line);
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let theme = &self.theme;
                    if widgets::primary_button(ui, theme, "OK").clicked() {
                        choice = Some(Choice::Ok);
                    }
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        choice = Some(Choice::Cancel);
                    }
                    if widgets::secondary_button(ui, theme, "Direction...").clicked() {
                        choice = Some(Choice::Direction);
                    }
                });
            });
        });
        if self.units_direction.is_some() {
            self.direction_window(ctx, &mut draft);
        } else if !open {
            choice = Some(Choice::Cancel);
        }
        match choice {
            Some(Choice::Ok) => {
                self.state.set_units(draft);
                self.state.show_units = false;
            }
            Some(Choice::Cancel) => self.state.show_units = false,
            Some(Choice::Direction) => {
                self.units_direction = Some(draft.base_angle);
                self.units_draft = Some(draft);
            }
            None => self.units_draft = Some(draft),
        }
    }

    /// Direction Control: where angle 0 points. Cancel puts the angle the
    /// dialog was opened with back.
    fn direction_window(&mut self, ctx: &egui::Context, draft: &mut Units) {
        let Some(before) = self.units_direction else {
            return;
        };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        widgets::dialog(
            ctx,
            &self.theme,
            "Direction Control",
            &mut open,
            DIRECTION_WIDTH,
            |ui| {
                let theme = &self.theme;
                widgets::group_box(ui, theme, "Angle 0 direction", |ui| {
                    let mut degrees = draft.base_angle.to_degrees().rem_euclid(360.0);
                    for (label, at) in COMPASS {
                        if ui.radio((degrees - at).abs() < 1e-9, label).clicked() {
                            degrees = at;
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("Angle:");
                        widgets::number_field(ui, theme, &mut degrees, "\u{b0}", 1.0);
                    });
                    draft.base_angle = degrees.rem_euclid(360.0).to_radians();
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ok = widgets::primary_button(ui, theme, "OK").clicked();
                    cancel = widgets::secondary_button(ui, theme, "Cancel").clicked();
                });
            },
        );
        if cancel || !open {
            draft.base_angle = before;
        }
        if ok || cancel || !open {
            self.units_direction = None;
        }
    }
}

fn length_box(ui: &mut egui::Ui, theme: &Theme, draft: &mut Units) {
    widgets::group_box(ui, theme, "Length", |ui| {
        let width = ui.available_width();
        widgets::field_label(ui, theme, "Type:");
        widgets::dropdown(
            ui,
            theme,
            "length_type",
            draft.length.label(),
            width,
            |ui| {
                for format in LengthFormat::ALL {
                    ui.selectable_value(&mut draft.length, format, format.label());
                }
            },
        );
        widgets::field_label(ui, theme, "Precision:");
        let current = Units::length_precision_label(draft.length, draft.length_precision);
        widgets::dropdown(ui, theme, "length_precision", &current, width, |ui| {
            for precision in 0..=MAX_PRECISION {
                let label = Units::length_precision_label(draft.length, precision);
                ui.selectable_value(&mut draft.length_precision, precision, label);
            }
        });
    });
}

fn angle_box(ui: &mut egui::Ui, theme: &Theme, draft: &mut Units) {
    widgets::group_box(ui, theme, "Angle", |ui| {
        let width = ui.available_width();
        widgets::field_label(ui, theme, "Type:");
        widgets::dropdown(ui, theme, "angle_type", draft.angle.label(), width, |ui| {
            for format in AngleFormat::ALL {
                ui.selectable_value(&mut draft.angle, format, format.label());
            }
        });
        widgets::field_label(ui, theme, "Precision:");
        let current = Units::angle_precision_label(draft.angle, draft.angle_precision);
        widgets::dropdown(ui, theme, "angle_precision", &current, width, |ui| {
            for precision in 0..=MAX_PRECISION {
                let label = Units::angle_precision_label(draft.angle, precision);
                ui.selectable_value(&mut draft.angle_precision, precision, label);
            }
        });
        ui.checkbox(&mut draft.clockwise, "Clockwise");
    });
}

fn insertion_box(ui: &mut egui::Ui, theme: &Theme, draft: &mut Units) {
    widgets::group_box(ui, theme, "Insertion scale", |ui| {
        widgets::field_label(ui, theme, "Units to scale inserted content:");
        let width = ui.available_width();
        widgets::dropdown(
            ui,
            theme,
            "insert_units",
            draft.insert.label(),
            width,
            |ui| {
                for unit in InsertUnits::all() {
                    ui.selectable_value(&mut draft.insert, unit, unit.label());
                }
            },
        );
    });
}

const POINT_WIDTH: f32 = 400.0;
const CELL: f32 = 54.0;
/// The pictures, left to right and top to bottom: each of the five shapes,
/// then with a circle, a square, and both.
const MODES: [[i32; 5]; 4] = [
    [0, 1, 2, 3, 4],
    [32, 33, 34, 35, 36],
    [64, 65, 66, 67, 68],
    [96, 97, 98, 99, 100],
];

impl OpenDraftApp {
    /// The window opened by `format.point_style`.
    pub(super) fn point_style_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_point_style {
            self.state.point_style_draft = None;
            return;
        }
        let mut draft = self
            .state
            .point_style_draft
            .take()
            .unwrap_or_else(|| Draft::of(self.state.point_style()));
        let (mut open, mut ok, mut cancel) = (true, false, false);
        widgets::dialog(
            ctx,
            &self.theme,
            "Point Style",
            &mut open,
            POINT_WIDTH,
            |ui| {
                let theme = &self.theme;
                for row in MODES {
                    ui.horizontal(|ui| {
                        for mode in row {
                            if swatch(ui, theme, mode, draft.mode == mode).clicked() {
                                draft.mode = mode;
                            }
                        }
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    widgets::field_label(ui, theme, "Point Size:");
                    let suffix = if draft.relative { "%" } else { "" };
                    widgets::number_field(ui, theme, &mut draft.value, suffix, 0.5);
                });
                ui.radio_value(&mut draft.relative, true, "Set Size Relative to Screen");
                ui.radio_value(&mut draft.relative, false, "Set Size in Absolute Units");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ok = widgets::primary_button(ui, theme, "OK").clicked();
                    cancel = widgets::secondary_button(ui, theme, "Cancel").clicked();
                });
            },
        );
        if ok {
            self.state.set_point_style(draft.style());
        }
        if ok || cancel || !open {
            self.state.show_point_style = false;
            self.state.point_style_draft = None;
        } else {
            self.state.point_style_draft = Some(draft);
        }
    }
}

/// One picture: the mode drawn in a frame, lit when it is the one chosen.
fn swatch(ui: &mut egui::Ui, theme: &Theme, mode: i32, chosen: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::click());
    let c = &theme.colors;
    let line = if chosen { c.accent } else { c.text };
    let painter = ui.painter_at(rect);
    painter.rect_stroke(
        rect.shrink(1.0),
        2.0,
        Stroke::new(if chosen { 2.0_f32 } else { 1.0_f32 }, line),
        StrokeKind::Inside,
    );
    let size = f64::from(CELL) * 0.5;
    let style = PointStyle { mode, size };
    let marks = style.marks(Point::new(0.0, 0.0), size);
    let at = |p: Point| Pos2::new(rect.center().x + p.x as f32, rect.center().y - p.y as f32);
    let stroke = Stroke::new(1.5_f32, line);
    if marks.dot {
        painter.circle_filled(rect.center(), 2.0, line);
    }
    for shape in &marks.shapes {
        paint(&painter, &at, shape, stroke);
    }
    response.on_hover_text(format!("PDMODE {mode}"))
}

/// The lines, circle and square a point's mode is made of.
fn paint(painter: &egui::Painter, at: &dyn Fn(Point) -> Pos2, shape: &Entity, stroke: Stroke) {
    match shape {
        Entity::Line { start, end } => {
            painter.line_segment([at(*start), at(*end)], stroke);
        }
        Entity::Circle { center, radius } => {
            painter.circle_stroke(at(*center), *radius as f32, stroke);
        }
        Entity::Rectangle { min, max } => {
            let rect = Rect::from_two_pos(at(*min), at(*max));
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Middle);
        }
        _ => {}
    }
}
