// A Windows release build is a windowed program: without this, double-clicking it
// opens a console window beside it, and closing that window ends the app. A debug
// build keeps its console. (`opendraft-mcp` stays a console program: it talks over
// standard input and output.) Panics are recorded in `crash.log` instead; see
// `crash_log`.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod arc_cmd;
mod array_cmd;
mod array_path;
mod assistant;
mod attdef_cmd;
mod attr_rows;
mod attribute_dialog_cmd;
mod attribute_edit;
#[cfg(test)]
mod attribute_flow_tests;
mod attribute_memory;
mod attribute_prompt;
mod attribute_rows;
mod attribute_sync_cmd;
#[cfg(test)]
mod attribute_test_util;
mod automation;
mod autosave;
mod block_cmd;
mod block_commands;
mod block_dialog_cmd;
mod block_editor;
mod boundary_cmd;
mod break_cmd;
mod build_info;
mod circle;
mod circle_arc_commands;
mod clipboard;
mod cmd_aliases;
mod cmdline;
mod colour_ui;
mod command_record;
mod commands;
mod coordinates;
mod corner_memory;
mod corners;
mod crash_log;
mod defaults;
mod dialogs;
mod dim_override_cmd;
mod dim_style_draft;
mod dim_style_table;
mod dim_vars;
mod dim_vars_extra;
mod dimension_angular_cmd;
mod dimension_centre_cmd;
mod dimension_chain_cmd;
mod dimension_cmd;
mod dimension_commands;
mod dimension_edit_cmd;
mod dimension_jogged_cmd;
mod dimension_ordinate_cmd;
mod dimension_space_cmd;
mod divide_cmd;
mod dock;
mod donut_cmd;
mod draw;
mod dynamic_input;
mod dynamic_settings;
mod edges;
mod ellipse_cmd;
mod explode;
mod field_cmd;
mod field_context;
mod field_picker;
mod file_io;
mod files;
mod grips;
mod hatch_cmd;
mod hatch_edit;
mod hatch_memory;
mod home;
mod home_scan;
mod icon_library;
mod insert_cmd;
mod interact;
mod io;
mod join;
mod layer_cells;
mod layer_cmd;
mod layers;
mod layers_ui;
mod layout_cmd;
mod layout_tabs;
mod layouts;
mod leader_cmd;
mod lengthen;
mod linetype_ui;
mod linetype_vars;
mod locations;
mod match_props;
mod measure_cmd;
mod mleader_cmd;
mod mleader_style_table;
mod modify;
mod mtext;
mod mtext_editor;
mod object_styles;
mod offset;
mod once_input;
mod overkill_cmd;
mod overkill_line;
mod overkill_memory;
mod pedit_cmd;
mod pedit_join;
mod pedit_memory;
mod pedit_vertex;
mod picker;
mod plot_state;
mod point_cmd;
mod point_vars;
mod polygon_cmd;
mod polyline_arc;
mod polyline_cmd;
mod power_select;
mod preview_textures;
mod previews;
mod primitive_commands;
mod profile;
mod prompt_reply;
mod properties;
mod properties_ui;
mod purge_cmd;
mod purge_delete;
mod purge_line;
mod purge_memory;
mod recent;
mod recovery;
mod rect_memory;
mod rectangle;
mod reveal;
mod ribbon_def;
mod ribbon_slots;
mod scale_rotate;
mod selection;
mod settings;
mod settings_tracking;
mod settings_ui;
mod snap_once;
mod snap_vars;
mod spline_cmd;
mod spline_memory;
mod state;
mod stretch;
mod style_rows;
mod system_fonts;
mod sysvar_line;
mod sysvars;
mod template_state;
mod templates;
mod text;
mod text_memory;
mod text_style_draft;
mod units_state;
mod units_vars;
mod updates;
mod whats_new;
mod window_memory;
mod xline_cmd;

use eframe::egui;

/// The window and taskbar icon, drawn from the badge SVG.
fn app_icon() -> Option<egui::IconData> {
    const SIDE: u32 = 256;
    let svg = include_bytes!("../../../assets/badge_logo.svg");
    let rgba = od_ui::icon_set::rgba(svg, SIDE).ok()?;
    Some(egui::IconData {
        rgba,
        width: SIDE,
        height: SIDE,
    })
}

/// How many samples per pixel the window asks for, best first. Filled glyph
/// meshes have no feathered edge of their own, so 4 smooths them; but some
/// graphics setups offer no such configuration (software rendering on a recent
/// Mesa, a few virtual machines and remote desktops), and without a fallback
/// the program would not open at all. 0 draws the same picture with harder
/// edges.
const MULTISAMPLES: [u16; 2] = [4, 0];

/// Builds the application once the window and its OpenGL context exist.
fn app_creator() -> eframe::AppCreator<'static> {
    Box::new(|cc| {
        let settings = settings::Settings::load(cc.storage);
        let defaults = std::sync::Arc::new(defaults::CommandDefaults::load(cc.storage));
        let theme = od_ui::Theme::with_palette(settings.palette);
        theme.apply(&cc.egui_ctx);
        cc.egui_ctx.set_zoom_factor(settings.zoom_factor());
        let dock = dock::load(cc.storage);
        let recent = recent::RecentFiles::load(cc.storage);
        let updates = updates::Updates::new(updates::Updates::load(cc.storage));
        // Drawings named on the command line (or handed over by a file
        // association) open at start.
        let open: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
        Ok(Box::new(app::OpenDraftApp::new(
            theme, settings, defaults, dock, recent, updates, open,
        )))
    })
}

/// Whether `error` is the graphics system having no configuration like the one
/// asked for, which a plainer request may still get.
fn is_no_matching_config(error: &eframe::Error) -> bool {
    matches!(error, eframe::Error::NoGlutinConfigs(..))
}

fn main() -> eframe::Result {
    profile::seed_beta_from_release();
    crash_log::install();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(profile::name())
        .with_app_id(profile::app_id())
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0]);
    viewport = window_memory::WindowMemory::load().apply(viewport);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let mut result = Ok(());
    for samples in MULTISAMPLES {
        let options = eframe::NativeOptions {
            viewport: viewport.clone(),
            // window_memory keeps the restore size that eframe loses when maximised.
            persist_window: false,
            multisampling: samples,
            ..Default::default()
        };
        result = eframe::run_native(profile::name(), options, app_creator());
        if !result.as_ref().is_err_and(is_no_matching_config) {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::MULTISAMPLES;

    #[test]
    fn the_window_asks_for_smooth_edges_first_and_can_settle_for_none() {
        assert_eq!(MULTISAMPLES.first(), Some(&4));
        assert_eq!(
            MULTISAMPLES.last(),
            Some(&0),
            "without a plain last choice the program would not open on a graphics setup that has no multisampled configuration"
        );
    }
}
