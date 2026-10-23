//! The small setters and readers of a view's state: prompts, constraints,
//! the last point, picked grips, and the zoom requests.

use std::sync::Arc;

use egui::Rect;
use od_core::{EntityId, Point};

use super::{PointPrompt, Request, View, STEP_ZOOM};

impl View {
    /// UI overlays can constrain previews and keep their clicks out of drawing input.
    pub fn set_cursor_constraint(
        &mut self,
        constraint: Option<Arc<dyn Fn(Point) -> Point + Send + Sync>>,
    ) {
        self.constraint = constraint;
    }

    pub fn set_input_exclusions(&mut self, rects: Vec<Rect>) {
        self.input_exclusions = rects;
    }

    /// Let the wheel zoom where clicks are excluded (an in-place editor
    /// covers the drawing but the view should stay navigable).
    pub fn set_wheel_through_exclusions(&mut self, through: bool) {
        self.wheel_through = through;
    }
    /// Make the canvas collect a point for a running command (`Some`), or
    /// stop doing so (`None`).
    pub fn set_point_prompt(&mut self, prompt: Option<PointPrompt>) {
        self.point_prompt = prompt;
    }

    /// Whether a running command is waiting for a point.
    pub fn is_prompting(&self) -> bool {
        self.point_prompt.is_some()
    }

    pub fn last_point(&self) -> Option<Point> {
        self.last_point
    }

    /// Remember a point given by typed entry, as the origin for relative
    /// coordinates typed next.
    pub fn set_last_point(&mut self, at: Point) {
        self.last_point = Some(at);
    }

    /// Frame the whole drawing on the next frame.
    /// Whether any grips are picked and waiting.
    pub fn has_warm_grips(&self) -> bool {
        !self.warm.is_empty()
    }

    /// Forget the picked grips (Esc).
    pub fn clear_warm_grips(&mut self) {
        self.warm.clear();
    }

    /// Forget what these hatches drew, so they are worked out again. The
    /// cache also checks its entries against the hatch, so this only frees
    /// memory early.
    pub fn invalidate_hatches(&self, ids: impl IntoIterator<Item = EntityId>) {
        self.hatch_cache.borrow_mut().invalidate(ids);
    }

    pub fn request_zoom_extents(&mut self) {
        self.request = Some(Request::Extents);
    }

    /// Zoom about the viewport center on the next frame.
    pub fn request_zoom_step(&mut self, zoom_in: bool) {
        let factor = if zoom_in { STEP_ZOOM } else { 1.0 / STEP_ZOOM };
        self.request = Some(Request::Zoom(factor));
    }
}
