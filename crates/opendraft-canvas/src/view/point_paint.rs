//! Painting a point object in the drawing's `PDMODE` and `PDSIZE`.

use super::*;

impl View {
    /// A point at `at`: a dot, or the shapes of its mode at a size that is
    /// absolute or a share of the screen height (`PDSIZE`).
    pub(super) fn stroke_point(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        at: Point,
        stroke: Stroke,
    ) {
        let style = self.point_style;
        let screen_height = f64::from(rect.height()) / self.camera.zoom;
        let marks = style.marks(at, style.size_in_units(screen_height));
        if marks.dot {
            painter.circle_filled(self.camera.world_to_screen(at, rect), 1.5, stroke.color);
        }
        for shape in &marks.shapes {
            self.stroke_entity(painter, rect, shape, stroke);
        }
    }
}
