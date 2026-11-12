//! The groups of controls the Page Setup and Plot dialogs share.

use eframe::egui;
use opendraft_core::{
    paper_size, paper_size_name, plot_settings::flags, PlotArea, PlotRotation, PlotScale,
    StandardScale, PAPER_SIZES,
};
use opendraft_ui::widgets;

use opendraft_core::plot_settings::{DEVICE_PDF, DEVICE_SVG};

use crate::layouts::PageDraft;
use crate::plot_controls::control_state;

const CUSTOM: &str = "Custom";

mod stored;

pub(in crate::app) use stored::{shade_box, style_box};

/// What can be edited, for the setup being edited.
fn controls_of(draft: &PageDraft) -> crate::plot_controls::PlotControlState {
    control_state(
        &draft.plot,
        draft.space == opendraft_core::plot::PlotSpace::Model,
    )
}

/// The width of the caption column of a form row.
const LABEL_WIDTH: f32 = 46.0;

/// Page Setup's left column: the device, the paper and what is plotted.
pub(in crate::app) fn left_column(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
) {
    widgets::group_box(ui, theme, "Printer/plotter", |ui| {
        let shown = if draft.plot.device_name == "none_device" {
            "None".to_owned()
        } else {
            draft.plot.device_name.clone()
        };
        widgets::dropdown(
            ui,
            theme,
            "plot_device",
            &shown,
            ui.available_width(),
            |ui| {
                for (name, stored) in [
                    ("None", "none_device"),
                    (DEVICE_PDF, DEVICE_PDF),
                    (DEVICE_SVG, DEVICE_SVG),
                ] {
                    if ui
                        .selectable_label(draft.plot.device_name == stored, name)
                        .clicked()
                    {
                        draft.plot.device_name = stored.into();
                    }
                }
            },
        );
        ui.small("System printers are chosen in the Plot dialog.");
    });
    ui.add_space(6.0);
    paper_box(ui, theme, draft);
    ui.add_space(6.0);
    plot_box(ui, theme, draft, None);
}

/// Page Setup's right column: what is stored but not applied yet, and the options.
pub(in crate::app) fn right_column(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
) {
    options_box(ui, theme, draft, true);
    ui.add_space(6.0);
    style_box(ui, theme, draft);
    ui.add_space(6.0);
    shade_box(ui, theme, draft);
}

/// The plot options in two columns. `full` adds the two that no 2D plot
/// applies (plot styles, hiding), shown disabled.
pub(in crate::app) fn options_box(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    full: bool,
) {
    widgets::group_box(ui, theme, "Options", |ui| {
        let controls = controls_of(draft);
        ui.columns(2, |columns| {
            flag_checkbox(
                &mut columns[0],
                draft,
                flags::PRINT_LINEWEIGHTS,
                "Plot lineweights",
            );
            flag_checkbox(
                &mut columns[0],
                draft,
                flags::SCALE_LINEWEIGHTS,
                "Scale lineweights",
            );
            let mut paper_last = !has_flag(draft, flags::DRAW_VIEWPORTS_FIRST);
            let changed = columns[1]
                .add_enabled_ui(controls.can_paperspace_order, |ui| {
                    ui.checkbox(&mut paper_last, "Paperspace last").changed()
                })
                .inner;
            if changed {
                set_flag(draft, flags::DRAW_VIEWPORTS_FIRST, !paper_last);
            }
            columns[1].add_enabled_ui(controls.can_paperspace_order, |ui| {
                flag_checkbox(ui, draft, flags::PLOT_VIEWPORT_BORDERS, "Viewport borders");
            });
            if full {
                columns[0]
                    .add_enabled_ui(controls.can_edit_styles, |ui| {
                        flag_checkbox(ui, draft, flags::PLOT_PLOT_STYLES, "Use plot styles");
                    })
                    .response
                    .on_disabled_hover_text("Plot style tables are not applied yet.");
                columns[1]
                    .add_enabled_ui(controls.can_hide_paperspace, |ui| {
                        flag_checkbox(ui, draft, flags::PLOT_HIDDEN, "Hide paperspace");
                    })
                    .response
                    .on_disabled_hover_text("There are no hidden lines to remove in 2D.");
            }
        });
    });
}

/// Whether the sheet is a size of its own rather than a named paper.
fn is_custom(draft: &PageDraft) -> bool {
    draft.plot.canonical_media_name == CUSTOM
        || paper_size_name(draft.width, draft.height).is_none()
}

/// The paper: its name and size on one line, which way up it is on the next.
pub(in crate::app) fn paper_box(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
) {
    widgets::group_box(ui, theme, "Paper", |ui| {
        let custom = is_custom(draft);
        let landscape = draft.width >= draft.height;
        let current = if custom {
            CUSTOM
        } else {
            paper_size_name(draft.width, draft.height).unwrap_or(CUSTOM)
        };
        ui.horizontal(|ui| {
            widgets::dropdown(ui, theme, "paper_size", current, 130.0, |ui| {
                for (name, ..) in PAPER_SIZES {
                    if ui
                        .selectable_label(!custom && *name == current, *name)
                        .clicked()
                    {
                        if let Some((w, h)) = paper_size(name, landscape) {
                            (draft.width, draft.height) = (w, h);
                            draft.plot.canonical_media_name = (*name).into();
                        }
                    }
                }
                if ui.selectable_label(custom, CUSTOM).clicked() {
                    draft.plot.canonical_media_name = CUSTOM.into();
                }
            });
            if custom {
                widgets::number_field_width(ui, theme, &mut draft.width, "", 1.0, 64.0);
                ui.label("×");
                widgets::number_field_width(ui, theme, &mut draft.height, "mm", 1.0, 74.0);
            } else {
                ui.label(
                    egui::RichText::new(format!("{:.0} × {:.0} mm", draft.width, draft.height))
                        .color(theme.colors.text_weak),
                );
            }
        });
        orientation_row(ui, draft);
    });
}

/// Portrait or landscape, and upside-down.
fn orientation_row(ui: &mut egui::Ui, draft: &mut PageDraft) {
    let quarter = matches!(
        draft.plot.rotation,
        PlotRotation::CounterClockwise90 | PlotRotation::Clockwise90
    );
    // What lies across the page: the sheet's shape, turned by a quarter turn
    // if the plot is.
    let landscape = (draft.width >= draft.height) != quarter;
    ui.horizontal(|ui| {
        let (portrait_clicked, landscape_clicked) = (
            ui.radio(!landscape, "Portrait").clicked(),
            ui.radio(landscape, "Landscape").clicked(),
        );
        if (portrait_clicked && landscape) || (landscape_clicked && !landscape) {
            turn_page(draft);
        }
        let mut upside = matches!(
            draft.plot.rotation,
            PlotRotation::Degrees180 | PlotRotation::Clockwise90
        );
        if ui.checkbox(&mut upside, "Upside-down").changed() {
            draft.plot.rotation = match (draft.plot.rotation, upside) {
                (PlotRotation::Degrees0 | PlotRotation::Degrees180, true) => {
                    PlotRotation::Degrees180
                }
                (PlotRotation::Degrees0 | PlotRotation::Degrees180, false) => {
                    PlotRotation::Degrees0
                }
                (_, true) => PlotRotation::Clockwise90,
                (_, false) => PlotRotation::CounterClockwise90,
            };
        }
    });
}

/// What is plotted and how big: the area, the scale and the offset as one
/// form, a caption and its controls on each row.
/// With `pick`, a Window area gets a Pick button that sets it to true.
pub(in crate::app) fn plot_box(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    pick: Option<&mut bool>,
) {
    widgets::group_box(ui, theme, "Plot", |ui| {
        let controls = controls_of(draft);
        let wide = (ui.available_width() - LABEL_WIDTH - 8.0).max(120.0);
        egui::Grid::new("plot_form")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                widgets::field_label(ui, theme, "Area");
                let window = matches!(draft.plot.area, PlotArea::Window { .. });
                ui.horizontal(|ui| {
                    let room = if pick.is_some() && window { 70.0 } else { 0.0 };
                    area_dropdown(ui, theme, draft, wide - room);
                    if let (Some(pick), true) = (pick, window) {
                        if widgets::secondary_button(ui, theme, "Pick").clicked() {
                            *pick = true;
                        }
                    }
                });
                ui.end_row();
                if let PlotArea::Window { first, second } = &mut draft.plot.area {
                    for (caption, point) in [("From", first), ("To", second)] {
                        widgets::field_label(ui, theme, caption);
                        ui.horizontal(|ui| {
                            widgets::number_field_width(ui, theme, &mut point.x, "", 1.0, 90.0);
                            widgets::number_field_width(ui, theme, &mut point.y, "", 1.0, 90.0);
                        });
                        ui.end_row();
                    }
                }
                widgets::field_label(ui, theme, "Scale");
                scale_row(ui, theme, draft, &controls);
                ui.end_row();
                widgets::field_label(ui, theme, "Offset");
                offset_row(ui, theme, draft, &controls);
                ui.end_row();
            });
        let m = draft.plot.margins_mm;
        if m.iter().any(|v| *v != 0.0) {
            ui.small(format!(
                "Printable margins L {:.1}, B {:.1}, R {:.1}, T {:.1} mm",
                m[0], m[1], m[2], m[3]
            ));
        }
    });
}

fn area_dropdown(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    width: f32,
) {
    let current = area_name(&draft.plot.area);
    widgets::dropdown(ui, theme, "plot_area", current, width, |ui| {
        // Display and named views cannot be plotted yet, so they are not
        // offered; a model has no sheet to plot and no limits.
        let model = draft.space == opendraft_core::plot::PlotSpace::Model;
        let choices: &[&str] = if model {
            &["Extents", "Window"]
        } else {
            &["Layout", "Extents", "Limits", "Window"]
        };
        for name in choices {
            if ui.selectable_label(*name == current, *name).clicked() {
                draft.plot.area = match *name {
                    "Extents" => PlotArea::Extents,
                    "Limits" => PlotArea::Limits,
                    "Window" => PlotArea::Window {
                        first: opendraft_core::Point::new(0.0, 0.0),
                        second: opendraft_core::Point::new(draft.width, draft.height),
                    },
                    _ => PlotArea::Layout,
                };
            }
        }
    });
}

/// Fit to paper, or plotted millimetres = drawing units.
fn scale_row(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    controls: &crate::plot_controls::PlotControlState,
) {
    ui.horizontal(|ui| {
        let mut fit = matches!(
            draft.plot.scale,
            PlotScale::Standard {
                kind: StandardScale::Fit,
                ..
            }
        );
        let changed = ui
            .add_enabled_ui(controls.can_fit, |ui| {
                ui.checkbox(&mut fit, "Fit").changed()
            })
            .inner;
        if changed {
            if fit {
                if let PlotScale::Custom {
                    paper_units,
                    drawing_units,
                } = draft.plot.scale
                {
                    draft.kept_scale = (paper_units, drawing_units);
                }
                draft.plot.scale = PlotScale::Standard {
                    kind: StandardScale::Fit,
                    value: 1.0,
                };
            } else {
                draft.plot.scale = PlotScale::Custom {
                    paper_units: draft.kept_scale.0,
                    drawing_units: draft.kept_scale.1,
                };
            }
            set_flag(draft, flags::USE_STANDARD_SCALE, fit);
        }
        ui.add_enabled_ui(controls.can_edit_scale, |ui| {
            let (paper, drawing) = match &mut draft.plot.scale {
                PlotScale::Custom {
                    paper_units,
                    drawing_units,
                } => (paper_units, drawing_units),
                _ => (&mut draft.kept_scale.0, &mut draft.kept_scale.1),
            };
            widgets::number_field_width(ui, theme, paper, "mm", 0.1, 62.0);
            ui.label("=");
            widgets::number_field_width(ui, theme, drawing, "units", 0.1, 74.0);
        });
    });
}

/// Centre the plot, or its offset from the printable corner.
fn offset_row(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    controls: &crate::plot_controls::PlotControlState,
) {
    ui.horizontal(|ui| {
        ui.add_enabled_ui(controls.can_center, |ui| {
            flag_checkbox(ui, draft, flags::PLOT_CENTERED, "Center");
        })
        .response
        .on_disabled_hover_text("A layout plots from the corner of its printable area.");
        ui.add_enabled_ui(controls.can_edit_offset, |ui| {
            widgets::field_label(ui, theme, "X");
            widgets::number_field_width(ui, theme, &mut draft.plot.origin_mm.x, "", 1.0, 54.0);
            widgets::field_label(ui, theme, "Y");
            widgets::number_field_width(ui, theme, &mut draft.plot.origin_mm.y, "mm", 1.0, 64.0);
        });
    });
}

/// Turn the page a quarter: swap the sheet's sides, or, when the plot is
/// already turned on the sheet, undo that turn (keeping upside-down).
fn turn_page(draft: &mut PageDraft) {
    match draft.plot.rotation {
        PlotRotation::CounterClockwise90 => draft.plot.rotation = PlotRotation::Degrees0,
        PlotRotation::Clockwise90 => draft.plot.rotation = PlotRotation::Degrees180,
        _ => std::mem::swap(&mut draft.width, &mut draft.height),
    }
}

pub(super) fn area_name(area: &PlotArea) -> &'static str {
    match area {
        PlotArea::Display => "Display",
        PlotArea::Extents => "Extents",
        PlotArea::Limits => "Limits",
        PlotArea::View(_) => "View",
        PlotArea::Window { .. } => "Window",
        PlotArea::Layout => "Layout",
    }
}

pub(super) fn has_flag(draft: &PageDraft, flag: u16) -> bool {
    draft.plot.flags & flag != 0
}
pub(super) fn set_flag(draft: &mut PageDraft, flag: u16, on: bool) {
    if on {
        draft.plot.flags |= flag;
    } else {
        draft.plot.flags &= !flag;
    }
}
pub(super) fn flag_checkbox(ui: &mut egui::Ui, draft: &mut PageDraft, flag: u16, label: &str) {
    let mut on = has_flag(draft, flag);
    if ui.checkbox(&mut on, label).changed() {
        set_flag(draft, flag, on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::{LayoutId, PlotSettings};

    fn draft(width: f64, height: f64, rotation: PlotRotation) -> PageDraft {
        let mut plot = PlotSettings::for_layout(width, height);
        plot.rotation = rotation;
        PageDraft {
            space: opendraft_core::plot::PlotSpace::Layout(LayoutId(1)),
            name: "A".into(),
            width,
            height,
            plot,
            kept_scale: (1.0, 1.0),
        }
    }

    #[test]
    fn turning_the_page_swaps_the_sheet_and_never_also_rotates_the_plot() {
        let mut d = draft(420.0, 297.0, PlotRotation::Degrees0);
        turn_page(&mut d);
        assert_eq!(
            (d.width, d.height, d.plot.rotation),
            (297.0, 420.0, PlotRotation::Degrees0)
        );
    }

    #[test]
    fn a_plot_already_turned_on_the_sheet_is_turned_back_first() {
        let mut d = draft(420.0, 297.0, PlotRotation::CounterClockwise90);
        turn_page(&mut d);
        assert_eq!((d.width, d.plot.rotation), (420.0, PlotRotation::Degrees0));
        let mut d = draft(420.0, 297.0, PlotRotation::Clockwise90);
        turn_page(&mut d);
        assert_eq!(d.plot.rotation, PlotRotation::Degrees180);
    }
}
