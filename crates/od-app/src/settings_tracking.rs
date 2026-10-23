//! The tracking group of Settings > Drafting: object snap tracking, polar
//! tracking, their angles and how acquired points and paths are shown.

use eframe::egui;
use od_ui::{widgets, Theme};

use crate::settings::Settings;

/// Polar angle increments the list offers, in degrees (`POLARANG` takes any).
const INCREMENTS: [f64; 9] = [90.0, 45.0, 30.0, 22.5, 18.0, 15.0, 10.0, 5.0, 1.0];

/// `TRACKPATH` choices.
const PATHS: [&str; 4] = [
    "Polar and object snap paths full-screen",
    "Polar paths full-screen, object snap paths to the cursor",
    "Only object snap paths, full-screen",
    "Only object snap paths, to the cursor",
];

pub fn tracking_group(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Tracking", |ui| {
        ui.checkbox(&mut draft.tracking.otrack, "Object snap tracking on (F11)");
        if ui
            .checkbox(&mut draft.tracking.polar, "Polar tracking on (F10)")
            .changed()
            && draft.tracking.polar
        {
            // Ortho and polar tracking exclude each other, as in AutoCAD.
            draft.ortho.enabled = false;
        }
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
            &mut draft.tracking.use_additional,
            "Use the additional angles (set them with POLARADDANG)",
        );
        ui.checkbox(
            &mut draft.tracking.all_polar,
            "Track from kept points along all the polar angles",
        );
        ui.checkbox(
            &mut draft.tracking.shift_to_acquire,
            "Keep points only while Shift is held",
        );
        ui.checkbox(
            &mut draft.tracking.tooltips,
            "Show the distance and angle beside tracking points",
        );
        egui::ComboBox::from_id_salt("track_path")
            .selected_text(PATHS[usize::from(draft.tracking.path_display.min(3))])
            .show_ui(ui, |ui| {
                for (n, text) in PATHS.iter().enumerate() {
                    ui.selectable_value(&mut draft.tracking.path_display, n as u8, *text);
                }
            });
        ui.label(
            egui::RichText::new(
                "Rest the pointer on a snap point to keep it, and again to let it go.",
            )
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
        );
    });
}
