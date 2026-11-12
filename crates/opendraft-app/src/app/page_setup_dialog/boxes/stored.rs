//! The groups of controls that hold settings no plot applies yet. They are
//! shown disabled in Page Setup (and not at all in the Plot dialog) with the
//! stored values kept.

use eframe::egui;
use opendraft_core::{plot_settings::flags, ShadePlotMode, ShadeResolution};
use opendraft_ui::widgets;

use super::flag_checkbox;
use crate::layouts::PageDraft;
use crate::plot_controls::control_state;

pub(in crate::app) fn style_box(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
) {
    widgets::group_box(ui, theme, "Plot style table (pen assignments)", |ui| {
        let editable = super::controls_of(draft).can_edit_styles;
        ui.add_enabled_ui(editable, |ui| {
            widgets::text_field(ui, theme, &mut draft.plot.style_sheet, "None");
            flag_checkbox(ui, draft, flags::SHOW_PLOT_STYLES, "Display plot styles");
        });
        ui.small("Plot style tables are not applied yet; any stored name is kept.");
    });
}

pub(in crate::app) fn shade_box(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
) {
    widgets::group_box(ui, theme, "Shaded viewport options", |ui| {
        let editable = control_state(&draft.plot, false).can_edit_shade_dpi;
        ui.add_enabled_ui(editable, |ui| shade_controls(ui, theme, draft))
            .response
            .on_disabled_hover_text(
                "Shaded viewports do not exist in 2D; the stored values are kept.",
            );
        ui.small("Shaded viewports do not exist in 2D; stored values are kept.");
    });
}

fn shade_controls(ui: &mut egui::Ui, theme: &opendraft_ui::Theme, draft: &mut PageDraft) {
    {
        let mode = match draft.plot.shade.mode {
            ShadePlotMode::AsDisplayed => "As displayed",
            ShadePlotMode::Wireframe => "Wireframe",
            ShadePlotMode::Hidden => "Hidden",
            ShadePlotMode::Rendered => "Rendered",
        };
        widgets::dropdown(ui, theme, "shade_mode", mode, ui.available_width(), |ui| {
            for (name, value) in [
                ("As displayed", ShadePlotMode::AsDisplayed),
                ("Wireframe", ShadePlotMode::Wireframe),
                ("Hidden", ShadePlotMode::Hidden),
                ("Rendered", ShadePlotMode::Rendered),
            ] {
                if ui.selectable_label(name == mode, name).clicked() {
                    draft.plot.shade.mode = value;
                }
            }
        });
        let quality = quality_name(draft.plot.shade.resolution);
        widgets::dropdown(
            ui,
            theme,
            "shade_quality",
            quality,
            ui.available_width(),
            |ui| {
                for (name, value) in [
                    ("Draft", ShadeResolution::Draft),
                    ("Preview", ShadeResolution::Preview),
                    ("Normal", ShadeResolution::Normal),
                    ("Presentation", ShadeResolution::Presentation),
                    ("Maximum", ShadeResolution::Maximum),
                    ("Custom", ShadeResolution::Custom),
                ] {
                    if ui.selectable_label(name == quality, name).clicked() {
                        draft.plot.shade.resolution = value;
                    }
                }
            },
        );
        if draft.plot.shade.resolution == ShadeResolution::Custom {
            let mut dpi = f64::from(draft.plot.shade.custom_dpi);
            widgets::number_field(ui, theme, &mut dpi, "DPI", 25.0);
            draft.plot.shade.custom_dpi = dpi.round().clamp(100.0, 32767.0) as u16;
        }
    }
}

fn quality_name(value: ShadeResolution) -> &'static str {
    match value {
        ShadeResolution::Draft => "Draft",
        ShadeResolution::Preview => "Preview",
        ShadeResolution::Normal => "Normal",
        ShadeResolution::Presentation => "Presentation",
        ShadeResolution::Maximum => "Maximum",
        ShadeResolution::Custom => "Custom",
    }
}
