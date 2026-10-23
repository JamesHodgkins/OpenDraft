mod app;
mod arc_cmd;
mod array_cmd;
mod array_path;
mod attr_rows;
mod automation;
mod autosave;
mod block_cmd;
mod block_editor;
mod boundary_cmd;
mod break_cmd;
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
mod match_props;
mod mleader_cmd;
mod mleader_style_table;
mod modify;
mod mtext;
mod mtext_editor;
mod object_styles;
mod offset;
mod once_input;
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
mod primitive_commands;
mod prompt_reply;
mod properties;
mod properties_ui;
mod recent;
mod recovery;
mod rect_memory;
mod rectangle;
mod ribbon_def;
mod ribbon_slots;
mod scale_rotate;
mod selection;
mod settings;
mod settings_legacy;
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
mod text;
mod text_memory;
mod text_style_draft;
mod units_state;
mod units_vars;
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

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("OpenDraft")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0]);
    viewport = window_memory::WindowMemory::load().apply(viewport);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        // window_memory keeps the restore size that eframe loses when maximised.
        persist_window: false,
        // Filled glyph meshes have no feathered edge of their own.
        multisampling: 4,
        ..Default::default()
    };

    eframe::run_native(
        "OpenDraft",
        options,
        Box::new(|cc| {
            let settings = settings::Settings::load(cc.storage);
            corner_memory::load(cc.storage);
            rect_memory::load(cc.storage);
            spline_memory::load(cc.storage);
            hatch_memory::load(cc.storage);
            text_memory::load(cc.storage);
            let theme = od_ui::Theme::with_palette(settings.palette);
            theme.apply(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(settings.zoom_factor());
            let dock = dock::load(cc.storage);
            let recent = recent::RecentFiles::load(cc.storage);
            // Drawings named on the command line (or handed over by a file
            // association) open at start.
            let open: Vec<std::path::PathBuf> =
                std::env::args_os().skip(1).map(Into::into).collect();
            Ok(Box::new(app::OpenDraftApp::new(
                theme, settings, dock, recent, open,
            )))
        }),
    )
}
