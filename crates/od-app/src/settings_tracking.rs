//! The tracking group of Settings > Drafting: the polar increment and the
//! tracking tooltips. The rest is set with `POLARMODE`, `POLARADDANG` and
//! `TRACKPATH`.

use eframe::egui;
use od_ui::{widgets, Theme};

use crate::settings::Settings;

/// Polar angle increments the list offers, in degrees (`POLARANG` takes any).
const INCREMENTS: [f64; 9] = [90.0, 45.0, 30.0, 22.5, 18.0, 15.0, 10.0, 5.0, 1.0];

pub fn tracking_group(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Tracking", |ui| {
        ui.horizontal(|ui| {
            ui.label("Increment angle");
            egui::ComboBox::from_id_salt("polar_increment")
                .selected_text(format!("{}", draft.tracking.increment))
                .show_ui(ui, |ui| {
                    for step in INCREMENTS {
                        ui.selectable_value(&mut draft.tracking.increment, step, format!("{step}"));
                    }
                });
        });
        ui.checkbox(
            &mut draft.tracking.tooltips,
            "Show the distance and angle beside tracking points",
        );
        ui.label(
            egui::RichText::new(
                "Rest the pointer on a snap point to keep it, and again to let it go.",
            )
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
        );
    });
}
