//! The Alternate Units and Tolerances tabs of the Dimension Style dialog.

use eframe::egui;
use od_core::units::MAX_PRECISION;
use od_core::Units;
use od_ui::{widgets, Theme};

use super::{choice, row, FIELD_WIDTH};
use crate::dim_style_draft::Draft;

pub(in crate::app) fn alternate(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let s = &mut draft.style;
    ui.checkbox(&mut s.alt_on, "Display alternate units");
    ui.add_space(8.0);
    ui.add_enabled_ui(s.alt_on, |ui| {
        widgets::group_box(ui, theme, "Alternate units", |ui| {
            row(ui, theme, "Precision:", |ui| {
                let now = Units::length_precision_label(s.length_format, s.alt_precision);
                widgets::dropdown(ui, theme, "dim_alt_precision", &now, FIELD_WIDTH, |ui| {
                    for p in 0..=MAX_PRECISION {
                        let label = Units::length_precision_label(s.length_format, p);
                        ui.selectable_value(&mut s.alt_precision, p, label);
                    }
                });
            });
            row(ui, theme, "Multiplier for alt units:", |ui| {
                widgets::number_field(ui, theme, &mut s.alt_factor, "", 0.05);
                if !s.alt_factor.is_finite() || s.alt_factor == 0.0 {
                    s.alt_factor = 25.4;
                }
            });
            row(ui, theme, "Suffix:", |ui| {
                widgets::text_field(ui, theme, &mut s.alt_post, "");
            });
        });
    });
}

pub(in crate::app) fn tolerances(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "Tolerance format", |ui| {
        const METHODS: [&str; 4] = ["None", "Symmetrical", "Deviation", "Limits"];
        let mut method = usize::from(s.tol_mode);
        row(ui, theme, "Method:", |ui| {
            choice(ui, theme, "dim_tol_method", &METHODS, &mut method)
        });
        s.tol_mode = method.min(3) as u8;
        ui.add_enabled_ui(s.tol_mode != 0, |ui| {
            row(ui, theme, "Precision:", |ui| {
                let now = Units::length_precision_label(s.length_format, s.tol_precision);
                widgets::dropdown(ui, theme, "dim_tol_precision", &now, FIELD_WIDTH, |ui| {
                    for p in 0..=MAX_PRECISION {
                        let label = Units::length_precision_label(s.length_format, p);
                        ui.selectable_value(&mut s.tol_precision, p, label);
                    }
                });
            });
            row(ui, theme, "Upper value:", |ui| {
                widgets::number_field(ui, theme, &mut s.tol_plus, "", 0.01);
            });
            ui.add_enabled_ui(s.tol_mode != 1, |ui| {
                row(ui, theme, "Lower value:", |ui| {
                    widgets::number_field(ui, theme, &mut s.tol_minus, "", 0.01);
                });
            });
            row(ui, theme, "Scaling for height:", |ui| {
                widgets::number_field(ui, theme, &mut s.tol_scale, "", 0.05);
                if !s.tol_scale.is_finite() || s.tol_scale <= 0.0 {
                    s.tol_scale = 1.0;
                }
            });
        });
        if s.tol_mode == 1 {
            s.tol_minus = s.tol_plus;
        }
    });
}
