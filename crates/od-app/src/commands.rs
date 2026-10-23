//! The application's commands. Ribbon items, the quick access toolbar and
//! shortcuts all refer to these by id.
//!
//! Interactive commands (draw, modify, select) are functions that take an
//! `Editor` and ask the user as they go; see `draw.rs`, `modify.rs` and
//! `selection.rs`.

use od_commands::{Command, Registry, RemoveEntities};

use crate::array_cmd::{array_objects, array_polar, array_rect};
use crate::array_path::array_path;
use crate::block_cmd::{define_block, purge_blocks};
use crate::break_cmd::break_object;
use crate::corners::{chamfer_objects, fillet_objects};
use crate::dimension_cmd::{dim_aligned, dim_arc, dim_auto, dim_diameter, dim_linear, dim_radius};
use crate::dimension_edit_cmd::{dim_edit, dim_text_edit};
use crate::draw::draw_line;
use crate::edges::{extend_objects, trim_objects};
use crate::ellipse_cmd::draw_ellipse;
use crate::explode::explode_objects;
use crate::hatch_cmd::hatch_objects;
use crate::hatch_edit::hatch_edit;
use crate::insert_cmd::insert_block;
use crate::join::join_objects;
use crate::layout_cmd::{layout_command, mview, vplayer, vpscale};
use crate::leader_cmd::draw_leader;
use crate::lengthen::lengthen_objects;
use crate::match_props::match_properties;
use crate::mleader_cmd::draw_mleader;
use crate::modify::{copy_objects, mirror_objects, move_objects};
use crate::mtext::{draw_mtext, edit_mtext_object};
use crate::offset::offset_objects;
use crate::polygon_cmd::draw_polygon;
use crate::polyline_cmd::draw_polyline;
use crate::rectangle::draw_rectangle;
use crate::scale_rotate::{rotate_objects, scale_objects};
use crate::selection::{crossing_polygon, fence, window_polygon};
use crate::spline_cmd::draw_spline;
use crate::stretch::stretch_objects;

use crate::dock::{LayoutRequest, PanelTab};
use crate::state::AppState;
use crate::text::plural;

pub fn build_registry() -> Registry<AppState> {
    let mut registry = Registry::default();
    // A running command owns the drawing: see `Command::transparent`.
    registry.busy_when(AppState::command_running);
    // The Home page has no drawing behind it: see `Command::global`.
    registry.needs_drawing_when(|s| s.home);
    let tables = [
        crate::clipboard::commands(),
        crate::layer_cmd::commands(),
        crate::circle_arc_commands::commands(),
        crate::primitive_commands::commands(),
    ];
    for command in all().into_iter().chain(tables.into_iter().flatten()) {
        registry
            .register(crate::automation::classified(command))
            .unwrap_or_else(|e| panic!("invalid command table: {e}"));
    }
    registry
}

/// Shorthand that fixes the context type so closures need no annotations.
fn cmd(id: &'static str, label: &'static str, run: fn(&mut AppState)) -> Command<AppState> {
    Command::new(id, label, run)
}

/// A command written against the `Editor`: runs it, and lights up while it
/// runs.
macro_rules! editor_command {
    // `$name` is how the command is called at the command line (WPOLYGON).
    ($id:literal, $label:literal, $name:literal, $run:path) => {
        cmd($id, $label, |s| s.start_command($name, $run))
            .checked(|s| s.is_running($name))
            // Starting another command replaces the running one.
            .transparent()
    };
    ($id:literal, $label:literal, $run:path) => {
        editor_command!($id, $label, $label, $run)
    };
}

/// A command that flips a boolean setting and lights up while it is on.
macro_rules! toggle_command {
    ($id:literal, $label:literal, $($field:ident).+) => {
        cmd($id, $label, |s| s.settings.$($field).+ = !s.settings.$($field).+)
            .checked(|s| s.settings.$($field).+)
    };
}

fn all() -> Vec<Command<AppState>> {
    vec![
        cmd("file.new", "New", |s| {
            s.new_document();
            s.message = format!("Created {}", s.docs[s.active].title);
        })
        .key("Ctrl+N")
        .global(),
        cmd("file.open", "Open", |s| s.open_dialog())
            .key("Ctrl+O")
            .global(),
        cmd("file.save", "Save", |s| {
            s.save_active();
        })
        .key("Ctrl+S"),
        cmd("file.save_as", "Save As", |s| {
            s.save_active_as();
        })
        .key("Ctrl+Shift+S"),
        cmd("file.export_dxf", "Export DXF", |s| {
            s.export_dxf();
        }),
        cmd("file.plot", "Plot", |s| {
            s.plot_layout();
        })
        .when(|s| s.space().layout().is_some()),
        cmd("file.close", "Close Drawing", |s| s.request_close(s.active))
            .key("Ctrl+W")
            .when(AppState::has_drawings),
        cmd("file.exit", "Exit", |s| s.exit_requested = true)
            .key("Ctrl+Q")
            .transparent()
            .global(),
        cmd("edit.undo", "Undo", |s| {
            if let Some(name) = s.undo() {
                s.message = format!("Undo: {name}");
            }
        })
        .key("Ctrl+Z")
        .when(|s| s.session().history().can_undo()),
        cmd("edit.redo", "Redo", |s| {
            if let Some(name) = s.redo() {
                s.message = format!("Redo: {name}");
            }
        })
        .key("Ctrl+Y")
        .when(|s| s.session().history().can_redo()),
        // Back to plain selecting: ends any command in progress.
        cmd("tool.select", "Select", |s| s.abort_command())
            .checked(|s| !s.command_running())
            .transparent(),
        editor_command!(
            "select.window_polygon",
            "Window Polygon",
            "WPolygon",
            window_polygon
        ),
        editor_command!(
            "select.crossing_polygon",
            "Crossing Polygon",
            "CPolygon",
            crossing_polygon
        ),
        editor_command!("select.fence", "Fence", fence),
        editor_command!("draw.line", "Line", draw_line),
        editor_command!("draw.ellipse", "Ellipse", draw_ellipse),
        editor_command!("draw.rectangle", "Rectangle", draw_rectangle),
        editor_command!("draw.polyline", "Polyline", draw_polyline),
        editor_command!("draw.polygon", "Polygon", "POLYGON", draw_polygon),
        editor_command!("draw.spline", "Spline", draw_spline),
        editor_command!("draw.hatch", "Hatch", hatch_objects),
        editor_command!("modify.hatchedit", "Edit Hatch", "HATCHEDIT", hatch_edit),
        editor_command!("annotate.mtext", "Multiline Text", "MTEXT", draw_mtext),
        editor_command!("annotate.mleader", "Multileader", "MLEADER", draw_mleader),
        editor_command!("annotate.leader", "Leader", "LEADER", draw_leader),
        editor_command!("annotate.mtedit", "Edit Text", "MTEDIT", edit_mtext_object),
        editor_command!("dim.dim", "Dimension", "DIM", dim_auto),
        editor_command!("dim.linear", "Linear", "DIMLINEAR", dim_linear),
        editor_command!("dim.aligned", "Aligned", "DIMALIGNED", dim_aligned),
        editor_command!("dim.arc", "Arc Length", "DIMARC", dim_arc),
        editor_command!("dim.radius", "Radius", "DIMRADIUS", dim_radius),
        editor_command!("dim.diameter", "Diameter", "DIMDIAMETER", dim_diameter),
        editor_command!("dim.edit", "Dimension Edit", "DIMEDIT", dim_edit),
        editor_command!(
            "dim.textedit",
            "Dimension Text Edit",
            "DIMTEDIT",
            dim_text_edit
        ),
        editor_command!("modify.move", "Move", move_objects),
        editor_command!("modify.copy", "Copy", copy_objects),
        editor_command!("modify.rotate", "Rotate", rotate_objects),
        editor_command!("modify.mirror", "Mirror", mirror_objects),
        editor_command!("modify.scale", "Scale", scale_objects),
        editor_command!("modify.trim", "Trim", trim_objects),
        editor_command!("modify.extend", "Extend", extend_objects),
        editor_command!("modify.lengthen", "Lengthen", lengthen_objects),
        editor_command!("modify.offset", "Offset", offset_objects),
        editor_command!("modify.join", "Join", join_objects),
        editor_command!("modify.fillet", "Fillet", fillet_objects),
        editor_command!("modify.chamfer", "Chamfer", chamfer_objects),
        editor_command!("modify.explode", "Explode", explode_objects),
        editor_command!("modify.break", "Break", break_object),
        editor_command!("modify.array", "Array", "ARRAY", array_objects),
        editor_command!(
            "modify.array_rect",
            "Rectangular Array",
            "ARRAYRECT",
            array_rect
        ),
        editor_command!("modify.array_path", "Path Array", "ARRAYPATH", array_path),
        editor_command!(
            "modify.array_polar",
            "Polar Array",
            "ARRAYPOLAR",
            array_polar
        ),
        editor_command!(
            "modify.matchprop",
            "Match Properties",
            "MATCHPROP",
            match_properties
        ),
        editor_command!("block.create", "Create Block", "BLOCK", define_block),
        editor_command!("block.insert", "Insert Block", "INSERT", insert_block),
        editor_command!("block.purge", "Purge", "PURGE", purge_blocks),
        // The Block Editor (`block_editor.rs`).
        cmd("block.edit", "Edit Block", |s| s.bedit())
            .when(|s| !s.in_block_editor() && !s.session().document().blocks().is_empty())
            .checked(|s| s.is_running("BEDIT"))
            .transparent(),
        cmd("block.save", "Save Block", |s| {
            s.bsave();
        })
        .when(AppState::in_block_editor),
        cmd("block.close", "Close Block Editor", |s| s.bclose())
            .when(AppState::in_block_editor)
            .checked(|s| s.is_running("BCLOSE")),
        editor_command!("modify.stretch", "Stretch", stretch_objects),
        cmd("modify.erase", "Erase", |s| {
            if s.erase_viewport() {
                return;
            }
            let ids: Vec<_> = s.selection().iter().copied().collect();
            s.message = format!("Erased {}", plural(ids.len(), "object"));
            s.perform(RemoveEntities::new(ids));
        })
        .key("Delete")
        .when(|s| !s.selection().is_empty() || s.viewport_picked()),
        cmd("layout.mspace", "MSPACE", |s| s.mspace()).when(|s| s.space().layout().is_some()),
        cmd("layout.pspace", "PSPACE", |s| s.pspace()).when(|s| s.space().layout().is_some()),
        cmd("layout.model", "Model", |s| s.enter_model()),
        cmd("layout.new", "New Layout", |s| s.new_layout()).when(|s| !s.in_block_editor()),
        cmd("layout.layout", "LAYOUT", |s| {
            let current = s.space().layout();
            s.start_command("LAYOUT", move |ed| layout_command(ed, current));
        })
        .checked(|s| s.is_running("LAYOUT"))
        .when(|s| !s.in_block_editor())
        .transparent(),
        cmd("layout.mview", "MVIEW", |s| match s.space() {
            od_canvas::Space::Paper(layout) => {
                s.start_command("MVIEW", move |ed| mview(ed, layout))
            }
            _ => s.message = "MVIEW works in a layout's paper space".to_owned(),
        })
        .checked(|s| s.is_running("MVIEW"))
        .transparent(),
        cmd("layout.vpscale", "VPSCALE", |s| match s.target_viewport() {
            Some((layout, viewport)) => {
                s.start_command("VPSCALE", move |ed| vpscale(ed, layout, viewport))
            }
            None => s.message = "Pick a viewport, or enter one with MSPACE".to_owned(),
        })
        .checked(|s| s.is_running("VPSCALE"))
        .transparent(),
        cmd("layout.vplayer", "VPLAYER", |s| match s.space() {
            od_canvas::Space::Model => s.message = "VPLAYER works in a layout".to_owned(),
            space => {
                let current = s.target_viewport().map(|(_, v)| v);
                let layout = space.layout().unwrap_or(od_core::LayoutId(0));
                s.start_command("VPLAYER", move |ed| vplayer(ed, layout, current))
            }
        })
        .checked(|s| s.is_running("VPLAYER"))
        .transparent(),
        cmd("layout.vplock", "VPLOCK", |s| s.toggle_viewport_lock()),
        cmd("layout.page_setup", "Page Setup", |s| s.open_page_setup())
            .when(|s| s.space().layout().is_some()),
        cmd("tools.setvar", "Set Variable", |s| s.begin_setvar()),
        cmd("format.units", "Units", |s| s.show_units = true),
        cmd("select.power", "Power Select", |s| {
            s.power_select.open = true
        }),
        cmd("format.linetype", "Linetypes", |s| s.show_linetypes = true),
        cmd("annotate.textstyle", "Text Style", |s| {
            s.show_text_styles = true
        }),
        cmd("dim.style", "Dimension Style", |s| {
            s.show_dim_manager = true
        }),
        cmd("annotate.mleaderstyle", "Multileader Style", |s| {
            s.mleader_dialogs.manager = true
        }),
        cmd("edit.select_all", "Select All", |s| s.select_all())
            .key("Ctrl+A")
            .when(|s| s.session().document().has_objects_here()),
        cmd("edit.cancel", "Cancel", |s| s.cancel())
            .key("Escape")
            .when(|s| s.can_cancel())
            .transparent(),
        cmd("view.zoom_extents", "Zoom Extents", |s| {
            s.docs[s.active].view.request_zoom_extents()
        })
        .transparent(),
        cmd("view.zoom_in", "Zoom In", |s| {
            s.docs[s.active].view.request_zoom_step(true)
        })
        .transparent(),
        cmd("view.zoom_out", "Zoom Out", |s| {
            s.docs[s.active].view.request_zoom_step(false)
        })
        .transparent(),
        toggle_command!("view.grid", "Grid", show_grid)
            .key("F7")
            .transparent(),
        cmd("view.dynamic_input", "Dynamic Input", |s| {
            s.settings.dynamic_input.toggle();
            s.docs[s.active].dynamic_input.clear();
        })
        .key("F12")
        .checked(|s| s.settings.dynamic_input.enabled())
        .transparent(),
        toggle_command!("view.snap", "Snap", snap.enabled)
            .key("F3")
            .transparent(),
        cmd("view.ortho", "Ortho", |s| {
            let on = !s.settings.ortho.enabled;
            s.settings.set_ortho(on);
        })
        .checked(|s| s.settings.ortho.enabled)
        .key("F8")
        .transparent(),
        cmd("view.polar", "Polar Tracking", |s| {
            let on = !s.settings.tracking.polar;
            s.settings.set_polar(on);
        })
        .checked(|s| s.settings.tracking.polar)
        .key("F10")
        .transparent(),
        toggle_command!("view.otrack", "Object Snap Tracking", tracking.otrack)
            .key("F11")
            .transparent(),
        toggle_command!("view.snap_endpoint", "Endpoint Snap", snap.endpoint)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_midpoint", "Midpoint Snap", snap.midpoint)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_center", "Center Snap", snap.center)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_node", "Node Snap", snap.node)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_quadrant", "Quadrant Snap", snap.quadrant)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!(
            "view.snap_intersection",
            "Intersection Snap",
            snap.intersection
        )
        .when(|s| s.settings.snap.enabled)
        .transparent(),
        toggle_command!("view.snap_extension", "Extension Snap", snap.extension)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_parallel", "Parallel Snap", snap.parallel)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!(
            "view.snap_geometric_center",
            "Geometric Center Snap",
            snap.geometric_center
        )
        .when(|s| s.settings.snap.enabled)
        .transparent(),
        toggle_command!(
            "view.snap_apparent_intersection",
            "Apparent Intersection Snap",
            snap.apparent_intersection
        )
        .when(|s| s.settings.snap.enabled)
        .transparent(),
        toggle_command!("view.snap_insertion", "Insertion Snap", snap.insertion)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!(
            "view.snap_perpendicular",
            "Perpendicular Snap",
            snap.perpendicular
        )
        .when(|s| s.settings.snap.enabled)
        .transparent(),
        toggle_command!("view.snap_tangent", "Tangent Snap", snap.tangent)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_nearest", "Nearest Snap", snap.nearest)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        toggle_command!("view.snap_grid", "Grid Snap", snap.grid)
            .when(|s| s.settings.snap.enabled)
            .transparent(),
        cmd("view.panel_properties", "Properties Panel", |s| {
            s.layout_request = Some(LayoutRequest::Toggle(PanelTab::Properties))
        })
        .key("Ctrl+1")
        .transparent()
        .global(),
        cmd("view.panel_layers", "Layers Panel", |s| {
            s.layout_request = Some(LayoutRequest::Toggle(PanelTab::Layers))
        })
        .key("Ctrl+2")
        .transparent()
        .global(),
        cmd("layer.properties", "Layer Properties", |s| {
            s.layout_request = Some(LayoutRequest::Toggle(PanelTab::Layers))
        })
        .transparent(),
        cmd("layer.hide", "Hide", |s| s.hide_selected_layers()).when(|s| !s.selection().is_empty()),
        cmd("layer.lock", "Lock", |s| s.lock_selected_layers()).when(|s| !s.selection().is_empty()),
        cmd("layer.show", "Show", |s| s.show_all_layers()).when(|s| s.any_layer_hidden()),
        cmd("view.command_line", "Command Line", |s| {
            s.layout_request = Some(LayoutRequest::Toggle(PanelTab::CommandLine))
        })
        .key("Ctrl+9")
        .transparent()
        .global(),
        cmd("view.reset_layout", "Reset Layout", |s| {
            s.layout_request = Some(LayoutRequest::Reset)
        })
        .transparent()
        .global(),
        cmd("manage.settings", "Settings", |s| s.show_settings = true)
            .transparent()
            .global(),
    ]
}

#[cfg(test)]
mod tests {
    use od_commands::AddEntity;
    use od_core::{Entity, Point};

    use super::*;

    #[test]
    fn command_table_is_valid() {
        // Panics on duplicate ids or shortcuts.
        build_registry();
    }

    #[test]
    fn every_shortcut_parses() {
        for command in all() {
            if let Some(shortcut) = command.shortcut {
                assert!(
                    od_ui::shortcuts::parse_shortcut(shortcut).is_some(),
                    "unparseable shortcut {shortcut} on {}",
                    command.id
                );
            }
        }
    }

    fn sample(n: f64) -> Entity {
        Entity::Circle {
            center: Point::new(n, n),
            radius: 5.0,
        }
    }

    fn add(state: &mut AppState, entity: Entity) {
        state.perform(AddEntity::new(entity));
    }

    #[test]
    fn selection_commands_run_light_up_and_select_ends_them() {
        let registry = build_registry();
        let mut state = AppState::new();
        assert!(
            registry.is_checked("tool.select", &state),
            "idle is selecting"
        );
        for (id, name) in [
            ("select.window_polygon", "WPolygon"),
            ("select.crossing_polygon", "CPolygon"),
            ("select.fence", "Fence"),
        ] {
            registry.execute(id, &mut state).unwrap();
            state.settle_command();
            assert!(state.is_running(name), "{id}");
            assert!(registry.is_checked(id, &state));
            assert!(!registry.is_checked("tool.select", &state));
            registry.execute("tool.select", &mut state).unwrap();
            assert!(!state.command_running());
            assert!(registry.is_checked("tool.select", &state));
        }
    }

    #[test]
    fn drawing_commands_run_and_light_up_until_they_end() {
        let registry = build_registry();
        let mut state = AppState::new();
        for (id, name) in [
            ("draw.line", "Line"),
            ("draw.circle", "Circle"),
            ("draw.arc", "Arc"),
            ("draw.ellipse", "Ellipse"),
            ("draw.point", "POINT"),
            ("draw.point_multiple", "POINT"),
            ("draw.spline", "Spline"),
            ("draw.hatch", "Hatch"),
            ("draw.rectangle", "Rectangle"),
            ("annotate.mtext", "MTEXT"),
            ("annotate.mleader", "MLEADER"),
            ("annotate.leader", "LEADER"),
        ] {
            assert!(registry.is_enabled(id, &state), "drawing needs no objects");
            registry.execute(id, &mut state).unwrap();
            state.settle_command();
            assert!(state.is_running(name));
            assert!(registry.is_checked(id, &state));
            state.cancel();
            assert!(!registry.is_checked(id, &state));
        }
    }

    #[test]
    fn cancel_steps_back_through_command_then_selection() {
        let registry = build_registry();
        let mut state = AppState::new();
        add(&mut state, sample(1.0));
        assert!(!registry.is_enabled("edit.cancel", &state));

        registry.execute("edit.select_all", &mut state).unwrap();
        registry.execute("draw.line", &mut state).unwrap();
        state.settle_command();
        // Command running: first Esc ends it but keeps the selection.
        registry.execute("edit.cancel", &mut state).unwrap();
        assert!(!state.command_running());
        assert_eq!(state.selection().len(), 1);
        // Second Esc clears the selection.
        registry.execute("edit.cancel", &mut state).unwrap();
        assert!(state.selection().is_empty());
        assert!(!registry.is_enabled("edit.cancel", &state));
    }

    #[test]
    fn a_running_command_owns_the_drawing() {
        let registry = build_registry();
        let mut state = AppState::new();
        add(&mut state, sample(1.0));
        registry.execute("edit.select_all", &mut state).unwrap();
        registry.execute("draw.line", &mut state).unwrap();
        state.settle_command();
        for id in [
            "edit.undo",
            "modify.erase",
            "edit.select_all",
            "file.save",
            "file.open",
            "file.new",
            "file.close",
            "layer.hide",
        ] {
            assert!(!registry.is_enabled(id, &state), "{id} must wait");
        }
        for id in [
            "edit.cancel",
            "view.zoom_in",
            "view.grid",
            "manage.settings",
        ] {
            assert!(registry.is_enabled(id, &state), "{id} may run alongside");
        }
        // A panel edit is not a registry command; it ends the command first.
        state.perform(od_commands::AddEntity::new(sample(2.0)));
        assert!(!state.command_running());
    }

    #[test]
    fn file_commands_are_registered_with_the_usual_shortcuts() {
        let registry = build_registry();
        for (id, key) in [
            ("file.new", "Ctrl+N"),
            ("file.open", "Ctrl+O"),
            ("file.save", "Ctrl+S"),
            ("file.save_as", "Ctrl+Shift+S"),
            ("file.close", "Ctrl+W"),
            ("file.exit", "Ctrl+Q"),
        ] {
            assert_eq!(registry.get(id).unwrap().shortcut, Some(key), "{id}");
        }
    }

    #[test]
    fn the_file_commands_reach_the_file_logic() {
        let registry = build_registry();
        let mut state = AppState::new();
        add(&mut state, sample(1.0));
        // The test picker answers "cancelled" to both dialogs.
        registry.execute("file.save", &mut state).unwrap();
        assert!(state.docs[0].is_modified());
        registry.execute("file.open", &mut state).unwrap();
        registry.execute("file.exit", &mut state).unwrap();
        assert!(state.exit_requested);
        // Closing a drawing with changes asks first.
        assert!(registry.is_enabled("file.close", &state));
        registry.execute("file.new", &mut state).unwrap();
        state.active = 0;
        registry.execute("file.close", &mut state).unwrap();
        assert!(state.confirm.is_some(), "unsaved changes are asked about");
        assert_eq!(state.docs.len(), 2);
    }

    #[test]
    fn undo_redo_through_the_registry() {
        let registry = build_registry();
        let mut state = AppState::new();

        assert!(!registry.is_enabled("edit.undo", &state));
        add(&mut state, sample(1.0));
        assert!(registry.is_enabled("edit.undo", &state));

        registry.execute("edit.undo", &mut state).unwrap();
        assert!(state.session().document().is_empty());
        assert!(registry.is_enabled("edit.redo", &state));

        registry.execute("edit.redo", &mut state).unwrap();
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn erase_is_undoable_and_needs_a_selection() {
        let registry = build_registry();
        let mut state = AppState::new();
        add(&mut state, sample(1.0));
        assert!(
            !registry.is_enabled("modify.erase", &state),
            "nothing selected"
        );

        registry.execute("edit.select_all", &mut state).unwrap();
        registry.execute("modify.erase", &mut state).unwrap();
        assert!(state.session().document().is_empty());

        registry.execute("edit.undo", &mut state).unwrap();
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn undo_history_is_per_document() {
        let registry = build_registry();
        let mut state = AppState::new();
        add(&mut state, sample(1.0));
        registry.execute("file.new", &mut state).unwrap();
        // New drawing is active and has nothing to undo.
        assert!(!registry.is_enabled("edit.undo", &state));
    }

    #[test]
    fn erase_removes_only_the_selection_and_undo_restores_order() {
        let registry = build_registry();
        let mut state = AppState::new();
        for n in 1..=3 {
            add(&mut state, sample(f64::from(n)));
        }
        let before = state.session().document().clone();
        let second = state.session().document().objects()[1].id;
        state.click_select(Some(second), false);
        registry.execute("modify.erase", &mut state).unwrap();
        assert_eq!(state.session().document().len(), 2);
        assert!(state.selection().is_empty());

        registry.execute("edit.undo", &mut state).unwrap();
        assert_eq!(*state.session().document(), before);
    }

    #[test]
    fn grid_and_snap_toggles_flip_the_settings() {
        let registry = build_registry();
        let mut state = AppState::new();
        let (grid, snap) = (state.settings.show_grid, state.settings.snap.enabled);
        registry.execute("view.grid", &mut state).unwrap();
        registry.execute("view.snap", &mut state).unwrap();
        assert_eq!(state.settings.show_grid, !grid);
        assert_eq!(state.settings.snap.enabled, !snap);
        registry.execute("view.snap", &mut state).unwrap();
        assert_eq!(state.settings.snap.enabled, snap);
    }

    #[test]
    fn ortho_starts_off_and_the_command_and_variable_agree() {
        let registry = build_registry();
        let mut state = AppState::new();
        let table = crate::sysvars::table();
        assert_eq!(
            table.get("ORTHOMODE", &state),
            Ok(od_commands::Value::Int(0))
        );
        registry.execute("view.ortho", &mut state).unwrap();
        assert!(state.settings.ortho.enabled);
        assert_eq!(
            table.get("ORTHOMODE", &state),
            Ok(od_commands::Value::Int(1))
        );
        table.set("ORTHOMODE", "0", &mut state).unwrap();
        assert!(!state.settings.ortho.enabled);
        assert_eq!(
            table.get("ORTHOLOCK", &state),
            Ok(od_commands::Value::Int(0))
        );
        table.set("ORTHOLOCK", "1", &mut state).unwrap();
        assert!(state.settings.ortho.strict);
        assert!(table.set("ORTHOLOCK", "2", &mut state).is_err());
    }

    #[test]
    fn individual_snap_toggles_work_only_while_snap_is_on() {
        let registry = build_registry();
        let mut state = AppState::new();
        registry.execute("view.snap_endpoint", &mut state).unwrap();
        assert!(!state.settings.snap.endpoint);
        registry.execute("view.snap_endpoint", &mut state).unwrap();
        assert!(state.settings.snap.endpoint);

        registry.execute("view.snap", &mut state).unwrap();
        for id in [
            "view.snap_endpoint",
            "view.snap_midpoint",
            "view.snap_center",
            "view.snap_grid",
        ] {
            assert!(!registry.is_enabled(id, &state), "{id}");
        }
        // The saved per-mode choices survive the master switch.
        assert!(state.settings.snap.endpoint && state.settings.snap.grid);
    }

    #[test]
    fn layout_commands_leave_a_request_for_the_app() {
        let registry = build_registry();
        let mut state = AppState::new();
        registry
            .execute("view.panel_properties", &mut state)
            .unwrap();
        assert_eq!(
            state.layout_request,
            Some(LayoutRequest::Toggle(PanelTab::Properties))
        );
        registry.execute("view.reset_layout", &mut state).unwrap();
        assert_eq!(state.layout_request, Some(LayoutRequest::Reset));
    }

    #[test]
    fn view_commands_are_registered_and_enabled() {
        let registry = build_registry();
        let mut state = AppState::new();
        for id in ["view.zoom_extents", "view.zoom_in", "view.zoom_out"] {
            assert!(registry.is_enabled(id, &state), "{id}");
            registry.execute(id, &mut state).unwrap();
        }
    }
}
