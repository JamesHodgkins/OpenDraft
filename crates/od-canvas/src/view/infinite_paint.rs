//! Painting construction lines and rays: the part that is on the screen.

use od_core::infinite;

use super::*;

impl View {
    /// The part of a construction line or ray inside the visible window, as
    /// its two ends; `None` for any other entity or when it is off screen.
    pub(super) fn visible_part(&self, rect: Rect, entity: &Entity) -> Option<(Point, Point)> {
        let (origin, direction, ray) = entity.infinite_line()?;
        let window = self.camera.visible_world(rect);
        infinite::clipped(origin, direction, ray, &window)
    }

    /// Stroke a construction line or ray with its linetype. The pattern is
    /// laid from the base point, so it stays put as the view moves. Returns
    /// false, drawing nothing, for any other kind of object.
    pub(super) fn stroke_infinite(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        object: &od_core::Object,
        stroke: Stroke,
    ) -> bool {
        let Some((origin, direction, _)) = object.entity.infinite_line() else {
            return false;
        };
        let Some((a, b)) = self.visible_part(rect, &object.entity) else {
            return true;
        };
        let line = |start, end| Entity::Line { start, end };
        let pattern = document.dash_pattern(object);
        if let Some((elements, scale)) = pattern {
            let period: f64 = elements.iter().map(|e| e.length()).sum::<f64>() * scale;
            let along = (a.x - origin.x) * direction.x + (a.y - origin.y) * direction.y;
            let start = if period.is_finite() && period > 0.0 {
                let t = (along / period).floor() * period;
                Point::new(origin.x + direction.x * t, origin.y + direction.y * t)
            } else {
                a
            };
            if self.stroke_dashed(
                painter,
                rect,
                &line(start, b),
                elements,
                scale,
                false,
                stroke,
            ) {
                return true;
            }
        }
        self.stroke_entity(painter, rect, &line(a, b), stroke);
        true
    }
}
