use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Color32, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use opendraft_core::{
    arc, bulge, width, Bounds, Color, Document, Entity, EntityId, LayerId, LayoutId, Point,
    ViewportId,
};

use crate::camera::Camera;
use crate::grid::{grid_step, line_indices};
use crate::grips::{grip_at, keep_selected, press_grip, with_shared, GripHit};
use crate::hatch_cache::HatchCache;
use crate::hit::{pick_here, pick_visible_here};
use crate::layer_cache::{CacheConfig, LayerCaches};
use crate::snap::SnapModes;
use crate::snap_axis::Ortho;
use crosshair::CursorShape;

mod accessors;
mod acquire;
mod backdrop;
mod background;
mod builder;
mod crosshair;
mod dash_paint;
mod entity_paint;
mod hatch_paint;
mod image_paint;
mod infinite_paint;
#[cfg(test)]
mod input_tests;
mod insert_paint;
mod insert_pieces;
mod layer_paint;
mod layout;
mod paint;
mod path_stroke;
mod point_paint;
mod previous;
mod select_gesture;
#[cfg(test)]
mod select_gesture_tests;
mod select_paint;
mod snap_choice;
mod snap_marker;
mod sweep;
mod text_paint;
#[cfg(test)]
mod text_paint_tests;
pub(crate) mod text_raster;
pub use text_paint::paint_text;
mod types;

use layout::LayoutInput;
pub use layout::Space;
pub use select_gesture::SelectionStyle;
pub use types::{
    CanvasOutput, CanvasStyle, Click, Crosshair, Ghost, GhostStyle, GridSettings, PointPrompt,
    PointerSizes, RegionSelect,
};

/// Minimum on-screen distance between grid lines.
const GRID_MIN_PX: f64 = 16.0;
/// A heavier grid line every this many lines.
const GRID_MAJOR_EVERY: i64 = 5;
/// Zoom per mouse wheel point (exponential, so it feels even at any scale).
const WHEEL_ZOOM_RATE: f32 = 0.003;
/// Width of the translucent highlight under selected/hovered entities.
const HIGHLIGHT_WIDTH: f32 = 6.0;
/// A drag shorter than this (screen points) is not a selection box.
const BOX_MIN_PX: f32 = 4.0;
/// Factor for the Zoom In / Zoom Out commands.
const STEP_ZOOM: f64 = 1.5;

enum Request {
    Extents,
    Zoom(f64),
    /// ZOOM Window: two corners in the space being worked in.
    Window(Point, Point),
}

/// Per-drawing view state.
#[derive(Default)]
pub struct View {
    /// Pick, snap and grip sizes, set by the app from its settings.
    sizes: PointerSizes,
    crosshair: Crosshair,
    ortho: Ortho,
    /// How point objects look: the drawing's, copied in when it is shown.
    point_style: opendraft_core::PointStyle,
    /// Wheel zoom speed as a multiple of the base rate; none means 1.
    zoom_speed: Option<f32>,
    constraint: Option<Arc<dyn Fn(Point) -> Point + Send + Sync>>,
    input_exclusions: Vec<Rect>,
    wheel_through: bool,
    /// An entity the drawing leaves out (a text whose editor is open).
    hidden: Option<EntityId>,
    pub camera: Camera,
    /// Where the pointer was last seen while an operation was in progress,
    /// so the rubber band keeps following it past the canvas edge.
    trailing: Option<Point>,
    /// Screen start and current position of a selection box drag.
    drag: Option<(Pos2, Pos2)>,
    /// What a click on nothing and a drag do (see `select_gesture.rs`).
    selection_style: SelectionStyle,
    /// An implied window or lasso under way (window and lasso selection style).
    pending: Option<select_gesture::Pending>,
    /// Screen path of a press in empty space at a sweep prompt, while the
    /// button is down (see `sweep.rs`).
    sweep: Option<Vec<Pos2>>,
    request: Option<Request>,
    /// The views ZOOM Previous goes back to.
    previous: previous::Previous,
    /// Set while a running command waits for a point.
    point_prompt: Option<PointPrompt>,
    /// Set while a running command waits for objects to be selected.
    selecting_objects: bool,
    /// Tab cycling between the snap points under the pointer.
    snap_cycle: snap_choice::Cycle,
    /// What the pointer paused over (Extension and Parallel).
    snap_acquire: acquire::Acquire,
    /// Grips picked with Shift, waiting for one of them to be pressed.
    warm: Vec<GripHit>,
    /// A grip was pressed, or a prompt's point placed, and the button is
    /// still down: its release is not a click on the drawing, and moving is
    /// not a box selection. A command that ends on that press has stopped
    /// prompting by the release, which would otherwise select what is there.
    press_taken: bool,
    /// Last point given by a click or typed entry; the origin for relative
    /// typed coordinates.
    last_point: Option<Point>,
    /// Model, paper space of a layout, or a layout's viewport (see `layout.rs`).
    space: Space,
    /// The camera of the model tab while a layout is shown.
    model_camera: Camera,
    /// The camera of the sheet; `camera` is it in paper space.
    sheet: Camera,
    /// Frame the whole sheet on the next frame.
    sheet_fit: bool,
    /// The active viewport's window on screen: input outside it is ignored.
    clip: Option<Rect>,
    /// Whether the running middle-drag pans the viewport (decided where it
    /// started, so leaving the viewport does not switch it to the sheet).
    pan_viewport: Option<bool>,
    selected_viewport: Option<ViewportId>,
    /// What each hatch draws, kept between frames (see `hatch_cache.rs`).
    hatch_cache: std::cell::RefCell<HatchCache>,
    /// The plain entities as kept triangles (see `layer_cache`).
    layer_cache: std::cell::RefCell<LayerCaches>,
    /// What the slots have spent so far this frame making new layers, which
    /// they share (see `layer_paint.rs`).
    slice_spent: std::cell::Cell<std::time::Duration>,
    /// What each block reference draws, kept while the drawing is as it was.
    insert_pieces: std::cell::RefCell<insert_pieces::InsertPieces>,
    cache_config: CacheConfig,
    /// What the drawing is now, as `Session::draw_token` says; none until the
    /// app gives it, and then nothing is kept.
    document_token: Option<(u64, u64)>,
    /// The drawing, for making its picture in the background (`builder.rs`);
    /// weak, so that an edit while no picture is being made copies nothing.
    shared_document: std::sync::Weak<Document>,
    builder: std::cell::RefCell<Option<builder::Builder>>,
}

impl View {
    /// Fill the rest of `ui` with the canvas: handle pan/zoom input, then
    /// paint grid, axes and entities.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        style: &CanvasStyle,
        grid: &GridSettings,
        document: &Document,
        selection: &BTreeSet<EntityId>,
        snap_modes: &SnapModes,
    ) -> CanvasOutput {
        let rect = ui.available_rect_before_wrap();
        self.slice_spent.set(std::time::Duration::ZERO);
        self.point_style = document.point_style();
        opendraft_core::infinite::set_pixel_size(1.0 / self.camera.zoom);
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, style.background);
        let in_layout = self.space != Space::Model;
        if in_layout {
            painter.rect_filled(
                rect,
                0.0,
                style.background.lerp_to_gamma(Color32::BLACK, 0.4),
            );
        }
        self.begin_layout_frame(document, rect);
        // A double click of the wheel zooms to the extents, as in AutoCAD.
        // It only moves the view, so a running command carries on.
        if response.double_clicked_by(PointerButton::Middle) {
            self.request_zoom_extents();
        }

        let mut layout_input = LayoutInput::default();
        if in_layout {
            layout_input = self.navigate_layout(ui, &response, rect, document, self.wheel_rate());
        } else {
            match self.request.take() {
                Some(Request::Extents) => {
                    self.camera.fit(document.bounds(), rect);
                }
                Some(Request::Zoom(factor)) => self.camera.zoom_at(rect.center(), rect, factor),
                Some(Request::Window(a, b)) => {
                    self.camera.show_window(Bounds::from_points(a, b), rect);
                }
                None => {}
            }
            if response.dragged_by(PointerButton::Middle) {
                self.camera.pan_by_pixels(response.drag_delta());
            } else {
                self.pan_through_editor(ui, rect);
            }
        }
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| rect.contains(*p) && !self.blocked(*p) && !Self::covered(ui.ctx(), *p));
        if !in_layout {
            self.wheel_zoom(ui, rect, response.hovered(), pointer);
        }
        self.note_view(ui.input(|i| i.time));

        let tolerance = self.sizes.pick / self.camera.zoom;
        let cursor = pointer.map(|p| self.camera.screen_to_world(p, rect));
        let prompting = self.point_prompt.is_some();
        let selecting = !prompting;
        let shift = ui.input(|i| i.modifiers.shift);
        let picking = self.point_prompt.as_ref().is_some_and(|p| p.pick);
        // Grips belong to no command: a command asking for objects shows the
        // selected ones highlighted, without grips to press, as in AutoCAD.
        let grips_on = selecting && !self.selecting_objects;
        // What a click on an object prompt can pick (LAYULK reaches locked layers).
        let pick_object = match &self.point_prompt {
            Some(p) if p.pick && p.pick_locked => pick_visible_here,
            _ => pick_here,
        };
        // While drawing, the cursor is pulled onto snap points (and, with
        // ortho on, held to the axis from the prompt's base; Shift flips
        // that); everything below (preview, clicks, readout) then uses the
        // resolved position.
        self.snap_cycle.rect = Some(rect);
        let (aim, resolved) = self.aim_at(ui, document, snap_modes, grid, cursor, shift);
        let snapped = resolved.and_then(|(_, found)| found);
        let cursor = resolved.map(|(at, _)| at).or(cursor);
        // While drawing, the rubber band follows the pointer beyond the canvas
        // edge (clipped to it) instead of vanishing; if the pointer has left
        // the window it stays where it was last seen.
        self.trailing = if prompting {
            cursor
                .or_else(|| {
                    ui.input(|i| i.pointer.latest_pos())
                        .filter(|p| !self.input_exclusions.iter().any(|r| r.contains(*p)))
                        .filter(|p| !Self::covered(ui.ctx(), *p))
                        .map(|p| self.camera.screen_to_world(p, rect))
                })
                .or(self.trailing)
        } else {
            None
        };
        let preview_cursor = cursor.or(self.trailing);
        let constrain = |at| self.constraint.as_ref().map_or(at, |f| f(at));
        let preview_to = preview_cursor.map(constrain);
        let hover = cursor
            .filter(|_| selecting || picking)
            .and_then(|c| pick_object(document, c, tolerance));
        let primary = response.clicked_by(PointerButton::Primary);
        // A point is placed the moment the button goes down, where the pointer
        // was then (snapped like the cursor), not where it is by the time the
        // button comes up: a quick move-and-click would otherwise land late.
        let pressed_at = ui
            .input(|i| {
                i.pointer
                    .button_pressed(PointerButton::Primary)
                    .then(|| i.pointer.press_origin())
                    .flatten()
            })
            .filter(|p| rect.contains(*p) && response.hovered() && !self.blocked(*p))
            .map(|p| self.camera.screen_to_world(p, rect));
        let placed_at = pressed_at.and_then(|at| {
            if prompting && !picking {
                aim.place(at).map(|(p, snap)| (constrain(p), snap))
            } else {
                Some((at, None))
            }
        });
        let placed = placed_at.map(|(p, _)| p);
        self.snap_acquire.clear_if(placed.is_some());
        // A press on a grip of a selected object takes it instead of selecting.
        let grip_pick = pressed_at
            .filter(|_| grips_on && self.pending.is_none())
            .and_then(|at| grip_at(document, selection, at, self.sizes.grip / self.camera.zoom));
        let grip_hover = cursor
            .filter(|_| grips_on && self.drag.is_none())
            .and_then(|c| grip_at(document, selection, c, self.sizes.grip / self.camera.zoom));
        keep_selected(&mut self.warm, selection);
        let swallow = self.press_taken;
        if grip_pick.is_some() || (prompting && pressed_at.is_some()) {
            self.press_taken = true;
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.press_taken = false;
        }
        let grips = grip_pick
            .and_then(|pick| press_grip(&mut self.warm, pick, shift))
            .map(|hot| with_shared(document, selection, hot))
            .unwrap_or_default();
        let mut prompted = None;
        let mut picked = None;
        let mut path = None;
        let mut prompt_enter = false;
        let mut snap_menu = None;
        let mut context_menu = None;
        let sweeps = picking && self.point_prompt.as_ref().is_some_and(|p| p.sweep);
        if !sweeps {
            self.sweep = None;
        }
        if prompting {
            prompted = placed;
            if picking {
                picked = placed.and_then(|c| pick_object(document, c, tolerance));
            }
            path = self.sweep_answer(ui, rect, sweeps && picked.is_none(), &mut prompted);
            (prompt_enter, snap_menu) = self.right_release(ui, rect);
        } else {
            context_menu = self.right_released(ui, rect).0;
        }
        let mut selected = select_gesture::Selected::default();
        if selecting {
            let press = select_gesture::Press {
                cursor,
                tolerance,
                shift,
                taken: swallow || grip_pick.is_some(),
            };
            selected = self.select_frame(ui, &response, rect, document, press);
        } else {
            self.pending = None;
            self.drag = None;
        }

        if in_layout && !prompting {
            self.layout_clicks(ui, &response, rect, document, &mut layout_input);
            // A click on a viewport's border takes the viewport alone.
            if primary && self.selected_viewport.is_some() {
                selected.click = None;
                if selected.started_window {
                    self.drop_new_window();
                }
            }
        }
        if in_layout {
            self.paint_layout(&painter, rect, style, grid, document, selection, hover);
        } else {
            if grid.visible {
                self.paint_grid(&painter, rect, style, grid);
            }
            self.paint_axes(&painter, rect, style);
            self.paint_model(&painter, rect, style, document, selection, hover);
        }
        // Inside a viewport the model is only drawn within its window.
        let painter = match self.clip {
            Some(clip) => painter.with_clip_rect(clip.intersect(rect)),
            None => painter,
        };
        if grips_on && self.drag.is_none() && self.pending.is_none() {
            let marks = paint::GripMarks {
                hovered: grip_hover,
                warm: &self.warm,
            };
            self.paint_grips(&painter, rect, style, document, selection, marks);
        }

        if let (Some(prompt), Some(to)) = (&self.point_prompt, preview_to) {
            self.paint_prompt_preview(&painter, rect, style, prompt, to);
        }
        self.paint_acquired(&painter, rect, style, snapped);
        if let Some(snapped) = snapped {
            self.paint_snap_marker(&painter, rect, style, snap_modes, snapped, cursor);
        }
        let latest = ui.input(|i| i.pointer.latest_pos());
        self.paint_selection(&painter, rect, style, latest);
        self.paint_sweep(&painter, style);
        if let (true, Some(at)) = (response.hovered(), pointer) {
            let shape = CursorShape::for_state(prompting, picking, self.selecting_objects);
            let boxed = prompting && !picking && snap_modes.aperture_box;
            self.paint_aperture_box(&painter, at, style, boxed);
            let at = self.pulled_to(rect, at, snapped, snap_modes);
            self.paint_cursor(ui, rect, at, shape);
        }

        let double_click = (selecting
            && !in_layout
            && grip_pick.is_none()
            && response.double_clicked_by(PointerButton::Primary))
        .then(|| cursor.and_then(|c| pick_here(document, c, tolerance)))
        .flatten();
        CanvasOutput {
            rect: Some(rect),
            preview_cursor,
            cursor,
            click: selected.click,
            double_click,
            select: selected.region,
            grips,
            prompted,
            picked,
            path,
            pick_shift: shift,
            prompt_enter,
            snap_menu,
            context_menu,
            refused: pressed_at.is_some() && prompting && placed.is_none(),
            placed_snap: placed_at.and_then(|(_, snap)| snap),
            snapped,
            viewport_edit: layout_input.viewport_edit,
            enter_viewport: layout_input.enter_viewport,
            leave_viewport: layout_input.leave_viewport,
        }
    }
}
