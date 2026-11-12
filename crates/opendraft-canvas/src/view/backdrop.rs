//! The grid and the axes behind the entities.

use super::*;

impl View {
    pub(super) fn paint_grid(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        grid: &GridSettings,
    ) {
        let step = grid_step(grid.spacing, self.camera.zoom, GRID_MIN_PX);
        let visible = self.camera.visible_world(rect);
        let minor = Stroke::new(1.0_f32, style.grid_minor);
        let major = Stroke::new(1.0_f32, style.grid_major);
        let pick = |i: i64| {
            if i.rem_euclid(GRID_MAJOR_EVERY) == 0 {
                major
            } else {
                minor
            }
        };
        let at = |x: f64, y: f64| self.camera.world_to_screen(Point::new(x, y), rect);
        for i in line_indices(visible.min.x, visible.max.x, step) {
            let x = at(i as f64 * step, 0.0).x;
            painter.vline(x.round() + 0.5, rect.y_range(), pick(i));
        }
        for i in line_indices(visible.min.y, visible.max.y, step) {
            let y = at(0.0, i as f64 * step).y;
            painter.hline(rect.x_range(), y.round() + 0.5, pick(i));
        }
    }

    pub(super) fn paint_axes(&self, painter: &egui::Painter, rect: Rect, style: &CanvasStyle) {
        let o = self.camera.world_to_screen(Point::new(0.0, 0.0), rect);
        // Each axis runs from the origin along its positive direction only.
        let x_start = o.x.max(rect.left());
        if x_start < rect.right() {
            painter.hline(
                x_start..=rect.right(),
                o.y.round() + 0.5,
                Stroke::new(1.0_f32, style.axis_x),
            );
        }
        let y_start = o.y.min(rect.bottom());
        if y_start > rect.top() {
            painter.vline(
                o.x.round() + 0.5,
                rect.top()..=y_start,
                Stroke::new(1.0_f32, style.axis_y),
            );
        }
    }
}
