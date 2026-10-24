//! Pages of the Settings dialog. Each edits a draft [`Settings`]; nothing
//! takes effect until the dialog commits it (OK / Apply).

use eframe::egui;
use od_ui::{widgets, Palette, Theme};

use crate::locations::Location;
use crate::settings::{
    Settings, AUTOSAVE_CHOICES, CROSSHAIR_SIZE_RANGE, GRID_SPACING_RANGE, GRIP_SIZE_RANGE,
    PICK_SIZE_RANGE, SNAP_APERTURE_RANGE, UI_SCALES, ZOOM_SPEED_RANGE,
};

/// A folder the user asked to pick on the page; the dialog shows the picker
/// once the page has been drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderRequest {
    /// Add a folder assistants may use.
    Approve,
    Choose(Location),
}

pub const TABS: &[&str] = &["Display", "Drafting", "Files", "Automation"];

pub fn page(
    ui: &mut egui::Ui,
    theme: &Theme,
    index: usize,
    draft: &mut Settings,
    connections: &mut [crate::automation::ConnectionRow],
    request: &mut Option<FolderRequest>,
) {
    match index {
        0 => display(ui, theme, draft),
        1 => drafting(ui, theme, draft),
        2 => files(ui, theme, draft, request),
        _ => crate::automation::settings_page(ui, theme, draft, connections, request),
    }
}

fn display(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    ui.columns(2, |cols| {
        display_left(&mut cols[0], theme, draft);
        display_right(&mut cols[1], theme, draft);
    });
}

fn display_left(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Color theme", |ui| {
        for palette in Palette::ALL {
            ui.radio_value(&mut draft.palette, palette, palette.label());
        }
    });
}

fn display_right(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Mouse wheel", |ui| {
        let (min, max) = ZOOM_SPEED_RANGE;
        ui.horizontal(|ui| {
            ui.label("Zoom speed");
            ui.add(egui::Slider::new(&mut draft.zoom_speed, min..=max).suffix(" %"));
        });
    });
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Interface scale", |ui| {
        widgets::field_label(ui, theme, "Scale user interface elements to");
        let width = ui.available_width().min(160.0);
        widgets::dropdown(
            ui,
            theme,
            "ui_scale",
            &format!("{}%", draft.ui_scale_percent),
            width,
            |ui| {
                for percent in UI_SCALES {
                    ui.selectable_value(
                        &mut draft.ui_scale_percent,
                        percent,
                        format!("{percent}%"),
                    );
                }
            },
        );
    });
}

fn drafting(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    ui.columns(2, |cols| {
        drafting_left(&mut cols[0], theme, draft);
        drafting_right(&mut cols[1], theme, draft);
    });
}

fn drafting_left(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    dynamic_input(ui, theme, draft);
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Grid", |ui| {
        ui.horizontal(|ui| {
            ui.label("Grid spacing");
            widgets::number_field(ui, theme, &mut draft.grid_spacing, "mm", 0.5);
        });
        let (min, max) = GRID_SPACING_RANGE;
        ui.label(
            egui::RichText::new(format!("Between {min} and {max} mm."))
                .size(theme.font_size_small)
                .color(theme.colors.text_weak),
        );
    });
    ui.add_space(6.0);
    ortho(ui, theme, draft);
    ui.add_space(6.0);
    crosshair(ui, theme, draft);
}

fn drafting_right(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    crate::settings_tracking::tracking_group(ui, theme, draft);
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "AutoSnap", |ui| {
        ui.checkbox(&mut draft.snap.marker, "Marker");
        ui.checkbox(&mut draft.snap.tooltip, "Tooltip");
        ui.checkbox(&mut draft.snap.magnet, "Magnet");
        ui.checkbox(&mut draft.snap.aperture_box, "Display aperture box");
        ui.label(
            egui::RichText::new("Press Tab to cycle between snap points under the cursor.")
                .size(theme.font_size_small)
                .color(theme.colors.text_weak),
        );
    });
    ui.add_space(6.0);
    sizes(ui, theme, draft);
}

fn ortho(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Ortho", |ui| {
        ui.radio_value(
            &mut draft.ortho.strict,
            false,
            "Standard: an object snap overrides ortho",
        );
        ui.radio_value(
            &mut draft.ortho.strict,
            true,
            "Strict: an object snap is used, then held to the axis",
        );
        ui.label(
            egui::RichText::new("Hold Shift to turn ortho the other way while you pick.")
                .size(theme.font_size_small)
                .color(theme.colors.text_weak),
        );
    });
}

fn crosshair(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Crosshair", |ui| {
        let (min, max) = CROSSHAIR_SIZE_RANGE;
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.add(egui::Slider::new(&mut draft.crosshair_size, min..=max).suffix(" %"));
        });
        let mut custom = draft.crosshair_color.is_some();
        if ui.checkbox(&mut custom, "Choose a colour").changed() {
            draft.crosshair_color = custom.then_some([215, 215, 215]);
        }
        if let Some(rgb) = &mut draft.crosshair_color {
            ui.color_edit_button_srgb(rgb);
        }
    });
}

/// Pick, snap and grip target sizes.
fn sizes(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Sizes", |ui| {
        for (label, value, (min, max)) in [
            ("Pick size", &mut draft.pick_size, PICK_SIZE_RANGE),
            (
                "Snap aperture",
                &mut draft.snap_aperture,
                SNAP_APERTURE_RANGE,
            ),
            ("Grip size", &mut draft.grip_size, GRIP_SIZE_RANGE),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::Slider::new(value, min..=max).suffix(" px"));
            });
        }
    });
}

fn dynamic_input(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Dynamic Input", |ui| {
        let dynamic = &mut draft.dynamic_input;
        let sign = if dynamic.enabled() { 1 } else { -1 };
        let mut pointer = dynamic.dynmode.abs() & 1 != 0;
        let mut dimensions = dynamic.dynmode.abs() & 2 != 0;
        ui.checkbox(&mut pointer, "Pointer input");
        ui.checkbox(&mut dimensions, "Dimension input");
        dynamic.dynmode = sign * (i32::from(pointer) | (i32::from(dimensions) << 1));
        ui.checkbox(
            &mut dynamic.dynamic_prompts,
            "Show command prompting near crosshairs",
        );
        ui.horizontal(|ui| {
            ui.radio_value(&mut dynamic.pointer_absolute, false, "Relative coordinates");
            ui.radio_value(&mut dynamic.pointer_absolute, true, "Absolute coordinates");
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut dynamic.pointer_cartesian, false, "Polar format");
            ui.radio_value(&mut dynamic.pointer_cartesian, true, "Cartesian format");
        });
        egui::ComboBox::from_id_salt("dynamic_pointer_visibility")
            .selected_text(
                ["After typing", "At point prompts", "Always"]
                    [usize::from(dynamic.pointer_visibility.min(2))],
            )
            .show_ui(ui, |ui| {
                for (value, label) in ["After typing", "At point prompts", "Always"]
                    .iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut dynamic.pointer_visibility, value as u8, *label);
                }
            });
    });
}

/// How an autosave interval reads in the list.
fn autosave_label(minutes: u32) -> String {
    match minutes {
        0 => "Off".to_owned(),
        1 => "1 minute".to_owned(),
        n => format!("{n} minutes"),
    }
}

/// Width of the name column of the folder rows.
const NAME_WIDTH: f32 = 130.0;

/// One folder on one row: its name, where it points now, then Browse and
/// Default; the note sits beneath.
fn location_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    draft: &mut Settings,
    location: Location,
    request: &mut Option<FolderRequest>,
) {
    let (text, set) = match location.chosen(draft) {
        Some(folder) => (folder.display().to_string(), true),
        None => (
            location.default_folder().map_or_else(
                || "Chosen by the system".to_owned(),
                |d| d.display().to_string(),
            ),
            false,
        ),
    };
    ui.horizontal(|ui| {
        ui.add_sized(
            [NAME_WIDTH, ui.spacing().interact_size.y],
            egui::Label::new(location.label()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(set, |ui| {
                if widgets::secondary_button(ui, theme, "Default").clicked() {
                    location.set(draft, None);
                }
            });
            if widgets::secondary_button(ui, theme, "Browse...").clicked() {
                *request = Some(FolderRequest::Choose(location));
            }
            let color = if set {
                theme.colors.text
            } else {
                theme.colors.text_weak
            };
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).color(color)).truncate());
            });
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(NAME_WIDTH + ui.spacing().item_spacing.x);
        ui.label(
            egui::RichText::new(location.note())
                .size(theme.font_size_small)
                .color(theme.colors.text_weak),
        );
    });
}

fn files(
    ui: &mut egui::Ui,
    theme: &Theme,
    draft: &mut Settings,
    request: &mut Option<FolderRequest>,
) {
    widgets::group_box(ui, theme, "Folders", |ui| {
        for (i, location) in Location::ALL.into_iter().enumerate() {
            if i > 0 {
                ui.add_space(8.0);
            }
            location_row(ui, theme, draft, location, request);
        }
    });
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Autosave", |ui| {
        widgets::field_label(ui, theme, "Keep a recovery copy of unsaved drawings every");
        let width = ui.available_width().min(160.0);
        widgets::dropdown(
            ui,
            theme,
            "autosave_minutes",
            &autosave_label(draft.autosave_minutes),
            width,
            |ui| {
                for minutes in AUTOSAVE_CHOICES {
                    ui.selectable_value(
                        &mut draft.autosave_minutes,
                        minutes,
                        autosave_label(minutes),
                    );
                }
            },
        );
        ui.label(
            egui::RichText::new(
                "If the app closes unexpectedly, you are offered these copies at the next start.",
            )
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
        );
    });
}
