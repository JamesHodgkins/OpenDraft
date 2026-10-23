use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Color32, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use od_core::{
    arc, bulge, width, Color, Document, Entity, EntityId, LayerId, LayoutId, Point, ViewportId,
};

use crate::camera::Camera;
use crate::grid::{grid_step, line_indices};
use crate::grips::{grip_at, keep_selected, press_grip, with_shared, GripHit};
use crate::hatch_cache::HatchCache;
use crate::hit::{pick_here, pick_visible_here};
use crate::select::Region;
use crate::snap::SnapModes;
use crate::snap_axis::Ortho;
use crosshair::CursorShape;

mod accessors;
mod acquire;
mod backdrop;
mod crosshair;
mod dash_paint;
mod hatch_paint;
mod infinite_paint;
mod insert_paint;
mod layout;
mod paint;
mod point_paint;
mod snap_choice;
mod snap_marker;
mod text_paint;
mod text_raster;
pub use text_paint::paint_text;
mod types;

use layout::LayoutInput;
pub use layout::Space;
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
/// Entities smaller than this on screen are not drawn.
const MIN_DRAW_PX: f64 = 0.75;
/// A drag shorter than this (screen points) is not a selection box.
const BOX_MIN_PX: f32 = 4.0;
/// Factor for the Zoom In / Zoom Out commands.
const STEP_ZOOM: f64 = 1.5;

enum Request {
    Extents,
    Zoom(f64),
}

/// Per-drawing view state.
#[derive(Default)]
pub struct View {
    /// Pick, snap and grip sizes, set by the app from its settings.
    sizes: PointerSizes,
    crosshair: Crosshair,
    ortho: Ortho,
    /// How point objects look: the drawing's, copied in when it is shown.
    point_style: od_core::PointStyle,
    /// Wheel zoom speed as a multiple of the base rate; none means 1.
    zoom_speed: Option<f32>,
    constraint: Option<Arc<dyn Fn(Point) -> Point + Send + Sync>>,
    input_exclusions: Vec<Rect>,
    wheel_through: bool,
    pub camera: Camera,
    /// Where the pointer was last seen while an operation was in progress,
    /// so the rubber band keeps following it past the canvas edge.
    trailing: Option<Point>,
    /// Screen start and current position of a selection box drag.
    drag: Option<(Pos2, Pos2)>,
    request: Option<Request>,
    /// Set while a running command waits for a point.
    point_prompt: Option<PointPrompt>,
    /// Tab cycling between the snap points under the pointer.
    snap_cycle: snap_choice::Cycle,
    /// What the pointer paused over (Extension and Parallel).
    snap_acquire: acquire::Acquire,
    /// Grips picked with Shift, waiting for one of them to be pressed.
    warm: Vec<GripHit>,
    /// A grip was pressed and the button is still down: its release is not a
    /// click on the drawing, and moving is not a box selection.
    grip_press: bool,
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
        self.point_style = document.point_style();
        od_core::infinite::set_pixel_size(1.0 / self.camera.zoom);
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

        let mut layout_input = LayoutInput::default();
        if in_layout {
            layout_input = self.navigate_layout(ui, &response, rect, document, self.wheel_rate());
        } else {
            match self.request.take() {
                Some(Request::Extents) => {
                    self.camera.fit(document.bounds(), rect);
                }
                Some(Request::Zoom(factor)) => self.camera.zoom_at(rect.center(), rect, factor),
                None => {}
            }
            if response.dragged_by(PointerButton::Middle) {
                self.camera.pan_by_pixels(response.drag_delta());
            }
        }
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| rect.contains(*p) && !self.blocked(*p));
        // With an editor over the drawing the wheel still zooms, over the
        // editor too, but not over a toolbar, menu or dialog above it.
        let wheel_at = if self.wheel_through {
            let ctx = ui.ctx();
            ui.input(|i| i.pointer.latest_pos()).filter(|p| {
                rect.contains(*p)
                    && ctx
                        .layer_id_at(*p)
                        .is_none_or(|l| l.order == egui::Order::Background)
            })
        } else {
            pointer
        };
        let over = response.hovered() || (self.wheel_through && wheel_at.is_some());
        if let (true, Some(pos), false) = (over, wheel_at, in_layout) {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = f64::from((scroll * self.wheel_rate()).exp() * pinch);
            if (factor - 1.0).abs() > 1e-9 {
                self.camera.zoom_at(pos, rect, factor);
            }
        }

        let tolerance = self.sizes.pick / self.camera.zoom;
        let cursor = pointer.map(|p| self.camera.screen_to_world(p, rect));
        let prompting = self.point_prompt.is_some();
        let selecting = !prompting;
        let shift = ui.input(|i| i.modifiers.shift);
        let picking = self.point_prompt.as_ref().is_some_and(|p| p.pick);
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
            .filter(|_| selecting)
            .and_then(|at| grip_at(document, selection, at, self.sizes.grip / self.camera.zoom));
        let grip_hover = cursor
            .filter(|_| selecting && self.drag.is_none())
            .and_then(|c| grip_at(document, selection, c, self.sizes.grip / self.camera.zoom));
        keep_selected(&mut self.warm, selection);
        let swallow = self.grip_press;
        if grip_pick.is_some() {
            self.grip_press = true;
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.grip_press = false;
        }
        let grips = grip_pick
            .and_then(|pick| press_grip(&mut self.warm, pick, shift))
            .map(|hot| with_shared(document, selection, hot))
            .unwrap_or_default();
        let click = (primary && selecting && grip_pick.is_none() && !swallow)
            .then(|| {
                cursor.map(|c| Click {
                    entity: pick_here(document, c, tolerance),
                    remove: shift,
                })
            })
            .flatten();
        let mut select = None;
        let mut prompted = None;
        let mut picked = None;
        let mut prompt_enter = false;
        let mut snap_menu = None;
        let mut context_menu = None;
        if prompting {
            prompted = placed;
            if picking {
                picked = placed.and_then(|c| pick_object(document, c, tolerance));
            }
            (prompt_enter, snap_menu) = self.right_release(ui, rect);
        } else {
            context_menu = self.right_released(ui, rect).0;
        }
        if selecting {
            // Drag a box: left-to-right is Window, right-to-left is Crossing.
            if response.dragged_by(PointerButton::Primary) && !swallow && grip_pick.is_none() {
                let now = ui
                    .input(|i| i.pointer.latest_pos())
                    .unwrap_or(rect.center());
                let start = self
                    .drag
                    .map(|(start, _)| start)
                    .or_else(|| ui.input(|i| i.pointer.press_origin()))
                    .unwrap_or(now);
                self.drag = Some((start, now));
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                if let Some((start, end)) = self.drag.take() {
                    if (end - start).abs().max_elem() > BOX_MIN_PX {
                        let (a, b) = (
                            self.camera.screen_to_world(start, rect),
                            self.camera.screen_to_world(end, rect),
                        );
                        let corners = Region::rect_corners(a, b);
                        let region = if end.x >= start.x {
                            Region::Window(corners)
                        } else {
                            Region::Crossing(corners)
                        };
                        select = Some(RegionSelect {
                            region,
                            remove: shift,
                        });
                    }
                }
            }
        }

        if in_layout && !prompting {
            self.layout_clicks(ui, &response, rect, document, &mut layout_input);
        }
        if in_layout {
            self.paint_layout(&painter, rect, style, grid, document, selection, hover);
        } else {
            if grid.visible {
                self.paint_grid(&painter, rect, style, grid);
            }
            self.paint_axes(&painter, rect, style);
            self.paint_entities(
                &painter,
                rect,
                style,
                document,
                document.active_layout(),
                &[],
                selection,
                hover,
            );
        }
        // Inside a viewport the model is only drawn within its window.
        let painter = match self.clip {
            Some(clip) => painter.with_clip_rect(clip.intersect(rect)),
            None => painter,
        };
        if selecting && self.drag.is_none() {
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
        self.paint_selection_box(&painter, style);
        if let (true, Some(at)) = (response.hovered(), pointer) {
            let shape = match (prompting, picking) {
                (true, true) => CursorShape::PickBox,
                (true, false) => CursorShape::Crosshair,
                _ => CursorShape::Both,
            };
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
            click,
            double_click,
            select,
            grips,
            prompted,
            picked,
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
