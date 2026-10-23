//! The Units dialog (UNITS): length and angle format and precision, the
//! direction of angle 0 and the insertion units. It edits a draft copy of the
//! drawing's units; OK commits it as one undo step, Cancel / Escape / X drop it.

use eframe::egui;
use od_core::{units::MAX_PRECISION, AngleFormat, InsertUnits, LengthFormat, Units};
use od_ui::{widgets, Theme};

use super::OpenDraftApp;

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
