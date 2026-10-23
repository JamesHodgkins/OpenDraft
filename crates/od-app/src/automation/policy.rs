//! Which commands an automation connection may run, and how.
//!
//! One table, so the whole policy can be read and reviewed in one place.
//! Every registered command must have a row (a test checks both ways); a
//! command with no row would be denied anyway, but should be decided on
//! purpose. Rules the table must keep (also tested):
//! - only view, mode and selection commands are `ImmediateSafe`;
//! - lifecycle, dialog and file commands are `Denied`;
//! - anything that asks the user for points or choices is
//!   `InteractiveInApp`, never pretended to be immediate.

use od_commands::{AutomationPolicy, Command, CommandEffect};

use AutomationPolicy::{Denied, ImmediateSafe, InteractiveInApp};
use CommandEffect::{
    ApplicationLifecycle as Lifecycle, DrawingMutation as Mutation, FileRead, FileWrite,
    UiState as Ui,
};

pub const TABLE: &[(&str, AutomationPolicy, CommandEffect)] = &[
    // Files, drawings and the application itself.
    ("file.new", Denied, Lifecycle),
    ("file.open", Denied, FileRead),
    ("file.save", Denied, FileWrite),
    ("file.save_as", Denied, FileWrite),
    ("file.export_dxf", Denied, FileWrite),
    ("file.plot", Denied, FileWrite),
    ("file.close", Denied, Lifecycle),
    ("file.exit", Denied, Lifecycle),
    // History acts on whatever is on top, the user's steps included: the
    // `history.undo` and `history.redo` tools check who made the step.
    ("edit.undo", Denied, Mutation),
    ("edit.redo", Denied, Mutation),
    ("edit.cancel", Denied, Ui),
    // The clipboard is the user's own: outside callers leave it alone.
    ("edit.copy_clip", Denied, Ui),
    ("edit.copy_base", Denied, Ui),
    ("edit.cut", Denied, Mutation),
    ("edit.paste", Denied, Mutation),
    ("edit.paste_block", Denied, Mutation),
    ("edit.paste_original", Denied, Mutation),
    ("tool.select", Denied, Ui),
    // Selection.
    ("select.window_polygon", InteractiveInApp, Ui),
    ("select.crossing_polygon", InteractiveInApp, Ui),
    ("select.fence", InteractiveInApp, Ui),
    ("edit.select_all", ImmediateSafe, Ui),
    ("select.power", Denied, Ui),
    // Drawing.
    ("draw.line", InteractiveInApp, Mutation),
    ("draw.polygon", InteractiveInApp, Mutation),
    ("draw.boundary", InteractiveInApp, Mutation),
    ("draw.donut", InteractiveInApp, Mutation),
    ("draw.point", InteractiveInApp, Mutation),
    ("draw.ray", InteractiveInApp, Mutation),
    ("draw.xline", InteractiveInApp, Mutation),
    ("draw.point_multiple", InteractiveInApp, Mutation),
    ("draw.circle_diameter", InteractiveInApp, Mutation),
    ("draw.circle_2p", InteractiveInApp, Mutation),
    ("draw.circle_3p", InteractiveInApp, Mutation),
    ("draw.circle_ttr", InteractiveInApp, Mutation),
    ("draw.arc_sce", InteractiveInApp, Mutation),
    ("draw.circle", InteractiveInApp, Mutation),
    ("draw.arc", InteractiveInApp, Mutation),
    ("draw.ellipse", InteractiveInApp, Mutation),
    ("draw.rectangle", InteractiveInApp, Mutation),
    ("draw.polyline", InteractiveInApp, Mutation),
    ("draw.spline", InteractiveInApp, Mutation),
    ("draw.hatch", InteractiveInApp, Mutation),
    ("annotate.mtext", InteractiveInApp, Mutation),
    ("annotate.mtedit", InteractiveInApp, Mutation),
    ("annotate.mleader", InteractiveInApp, Mutation),
    ("annotate.leader", InteractiveInApp, Mutation),
    ("dim.linear", InteractiveInApp, Mutation),
    ("dim.aligned", InteractiveInApp, Mutation),
    ("dim.arc", InteractiveInApp, Mutation),
    ("dim.radius", InteractiveInApp, Mutation),
    ("dim.diameter", InteractiveInApp, Mutation),
    // Modifying. Erase acts on the selection at once, so it is not
    // interactive; the batch tool deletes by id instead.
    ("modify.hatchedit", InteractiveInApp, Mutation),
    ("modify.move", InteractiveInApp, Mutation),
    ("modify.copy", InteractiveInApp, Mutation),
    ("modify.rotate", InteractiveInApp, Mutation),
    ("modify.mirror", InteractiveInApp, Mutation),
    ("modify.scale", InteractiveInApp, Mutation),
    ("modify.trim", InteractiveInApp, Mutation),
    ("modify.extend", InteractiveInApp, Mutation),
    ("modify.lengthen", InteractiveInApp, Mutation),
    ("modify.offset", InteractiveInApp, Mutation),
    ("modify.join", InteractiveInApp, Mutation),
    ("modify.fillet", InteractiveInApp, Mutation),
    ("modify.chamfer", InteractiveInApp, Mutation),
    ("modify.explode", InteractiveInApp, Mutation),
    ("modify.break", InteractiveInApp, Mutation),
    ("modify.stretch", InteractiveInApp, Mutation),
    ("modify.matchprop", InteractiveInApp, Mutation),
    ("modify.array", InteractiveInApp, Mutation),
    ("modify.array_rect", InteractiveInApp, Mutation),
    ("modify.array_path", InteractiveInApp, Mutation),
    ("modify.array_polar", InteractiveInApp, Mutation),
    ("modify.erase", Denied, Mutation),
    // Blocks and the Block Editor.
    ("block.create", InteractiveInApp, Mutation),
    ("block.insert", InteractiveInApp, Mutation),
    ("block.purge", InteractiveInApp, Mutation),
    ("block.edit", Denied, Lifecycle),
    ("block.save", Denied, Lifecycle),
    ("block.close", Denied, Lifecycle),
    // Layouts: they change which space the user is in, or the sheet.
    ("layout.mspace", Denied, Ui),
    ("layout.pspace", Denied, Ui),
    ("layout.model", Denied, Ui),
    ("layout.new", Denied, Mutation),
    ("layout.layout", Denied, Mutation),
    ("layout.mview", Denied, Mutation),
    ("layout.vpscale", Denied, Mutation),
    ("layout.vplayer", Denied, Mutation),
    ("layout.vplock", Denied, Mutation),
    ("layout.page_setup", Denied, Lifecycle),
    // Dialogs and variables.
    ("tools.setvar", Denied, Lifecycle),
    ("format.units", Denied, Lifecycle),
    ("format.linetype", Denied, Lifecycle),
    ("annotate.textstyle", Denied, Lifecycle),
    ("dim.style", Denied, Lifecycle),
    ("annotate.mleaderstyle", Denied, Lifecycle),
    ("manage.settings", Denied, Lifecycle),
    // Layers: the layer batch tool does these with checks.
    ("layer.properties", Denied, Ui),
    ("layer.hide", Denied, Mutation),
    ("layer.lock", Denied, Mutation),
    ("layer.show", Denied, Mutation),
    ("layer.thaw_all", Denied, Mutation),
    ("layer.freeze", InteractiveInApp, Mutation),
    ("layer.unlock", InteractiveInApp, Mutation),
    ("layer.make_current", InteractiveInApp, Mutation),
    ("layer.match", InteractiveInApp, Mutation),
    // View and modes: nothing in the drawing changes.
    ("view.zoom_extents", ImmediateSafe, Ui),
    ("view.zoom_in", ImmediateSafe, Ui),
    ("view.zoom_out", ImmediateSafe, Ui),
    ("view.grid", ImmediateSafe, Ui),
    ("view.dynamic_input", ImmediateSafe, Ui),
    ("view.snap", ImmediateSafe, Ui),
    ("view.ortho", ImmediateSafe, Ui),
    ("view.polar", ImmediateSafe, Ui),
    ("view.otrack", ImmediateSafe, Ui),
    ("view.snap_endpoint", ImmediateSafe, Ui),
    ("view.snap_midpoint", ImmediateSafe, Ui),
    ("view.snap_center", ImmediateSafe, Ui),
    ("view.snap_node", ImmediateSafe, Ui),
    ("view.snap_quadrant", ImmediateSafe, Ui),
    ("view.snap_intersection", ImmediateSafe, Ui),
    ("view.snap_insertion", ImmediateSafe, Ui),
    ("view.snap_geometric_center", ImmediateSafe, Ui),
    ("view.snap_apparent_intersection", ImmediateSafe, Ui),
    ("view.snap_extension", ImmediateSafe, Ui),
    ("view.snap_parallel", ImmediateSafe, Ui),
    ("view.snap_perpendicular", ImmediateSafe, Ui),
    ("view.snap_tangent", ImmediateSafe, Ui),
    ("view.snap_nearest", ImmediateSafe, Ui),
    ("view.snap_grid", ImmediateSafe, Ui),
    ("view.panel_properties", Denied, Ui),
    ("view.panel_layers", Denied, Ui),
    ("view.command_line", Denied, Ui),
    ("view.reset_layout", Denied, Ui),
];

/// `command` with its row applied; one with no row stays `Denied`.
pub fn classified<C>(command: Command<C>) -> Command<C> {
    match TABLE.iter().find(|(id, ..)| *id == command.id) {
        Some(&(_, policy, effect)) => command.automation(policy, effect),
        None => command,
    }
}
