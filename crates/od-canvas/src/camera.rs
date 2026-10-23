use egui::{Pos2, Rect, Vec2};
use od_core::{Bounds, Point};

pub const MIN_ZOOM: f64 = 0.001;
pub const MAX_ZOOM: f64 = 100_000.0;

/// Maps drawing units to screen points. `center` is the world point shown at
/// the middle of the viewport; `zoom` is screen points per drawing unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center: Point,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Point::new(150.0, 100.0),
            zoom: 2.0,
        }
    }
}

impl Camera {
    pub fn world_to_screen(&self, p: Point, viewport: Rect) -> Pos2 {
        let c = viewport.center();
        Pos2::new(
            c.x + ((p.x - self.center.x) * self.zoom) as f32,
            c.y - ((p.y - self.center.y) * self.zoom) as f32,
        )
    }

    pub fn screen_to_world(&self, pos: Pos2, viewport: Rect) -> Point {
        let c = viewport.center();
        Point::new(
            self.center.x + f64::from(pos.x - c.x) / self.zoom,
            self.center.y - f64::from(pos.y - c.y) / self.zoom,
        )
    }

    /// Drag the drawing along with the pointer.
    pub fn pan_by_pixels(&mut self, delta: Vec2) {
        self.center.x -= f64::from(delta.x) / self.zoom;
        self.center.y += f64::from(delta.y) / self.zoom;
    }

    /// Zoom by `factor`, keeping the world point under `pos` fixed.
    pub fn zoom_at(&mut self, pos: Pos2, viewport: Rect, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.screen_to_world(pos, viewport);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(pos, viewport);
        self.center.x += before.x - after.x;
        self.center.y += before.y - after.y;
    }

    /// Frame `bounds` with a margin; an empty drawing resets to the default
    /// view.
    pub fn fit(&mut self, bounds: Option<Bounds>, viewport: Rect) {
        let Some(b) = bounds else {
            *self = Self::default();
            return;
        };
        let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
        let (vw, vh) = (f64::from(viewport.width()), f64::from(viewport.height()));
        if vw < 1.0 || vh < 1.0 {
            return;
        }
        let usable = 0.9;
        let zoom = match (w > 1e-9, h > 1e-9) {
            (true, true) => (vw * usable / w).min(vh * usable / h),
            (true, false) => vw * usable / w,
            (false, true) => vh * usable / h,
            (false, false) => Self::default().zoom,
        };
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = Point::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
    }

    /// World rectangle currently visible in `viewport`.
    pub fn visible_world(&self, viewport: Rect) -> Bounds {
        Bounds::from_points(
            self.screen_to_world(viewport.left_bottom(), viewport),
            self.screen_to_world(viewport.right_top(), viewport),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 600.0))
    }

    #[test]
    fn round_trips_between_world_and_screen() {
        let cam = Camera {
            center: Point::new(12.0, -7.0),
            zoom: 3.5,
        };
        let p = Point::new(40.0, 25.0);
        let back = cam.screen_to_world(cam.world_to_screen(p, viewport()), viewport());
        assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
    }

    #[test]
    fn y_axis_points_up() {
        let cam = Camera::default();
        let low = cam.world_to_screen(Point::new(0.0, 0.0), viewport());
        let high = cam.world_to_screen(Point::new(0.0, 10.0), viewport());
        assert!(high.y < low.y);
    }

    #[test]
    fn viewport_center_shows_camera_center() {
        let cam = Camera::default();
        let p = cam.world_to_screen(cam.center, viewport());
        assert_eq!(p, viewport().center());
    }

    #[test]
    fn zoom_keeps_point_under_cursor_fixed() {
        let mut cam = Camera::default();
        let cursor = Pos2::new(300.0, 200.0);
        let before = cam.screen_to_world(cursor, viewport());
        cam.zoom_at(cursor, viewport(), 1.7);
        let after = cam.screen_to_world(cursor, viewport());
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
        assert!((cam.zoom - 3.4).abs() < 1e-9);
    }

    #[test]
    fn zoom_is_clamped_and_ignores_bad_factors() {
        let mut cam = Camera::default();
        cam.zoom_at(Pos2::ZERO, viewport(), 1e12);
        assert_eq!(cam.zoom, MAX_ZOOM);
        cam.zoom_at(Pos2::ZERO, viewport(), 1e-12);
        assert_eq!(cam.zoom, MIN_ZOOM);
        let snapshot = cam;
        cam.zoom_at(Pos2::ZERO, viewport(), f64::NAN);
        cam.zoom_at(Pos2::ZERO, viewport(), -2.0);
        assert_eq!(cam, snapshot);
    }

    #[test]
    fn panning_moves_content_with_the_pointer() {
        let mut cam = Camera::default();
        let p = Point::new(10.0, 10.0);
        let before = cam.world_to_screen(p, viewport());
        cam.pan_by_pixels(Vec2::new(25.0, -10.0));
        let after = cam.world_to_screen(p, viewport());
        assert!((after.x - before.x - 25.0).abs() < 1e-3);
        assert!((after.y - before.y + 10.0).abs() < 1e-3);
    }

    #[test]
    fn fit_frames_bounds_inside_the_viewport() {
        let mut cam = Camera::default();
        let b = Bounds::from_points(Point::new(0.0, 0.0), Point::new(1000.0, 500.0));
        cam.fit(Some(b), viewport());
        let min = cam.world_to_screen(b.min, viewport());
        let max = cam.world_to_screen(b.max, viewport());
        assert!(min.x >= viewport().left() && max.x <= viewport().right());
        assert!(min.y <= viewport().bottom() && max.y >= viewport().top());
        assert_eq!(cam.center, Point::new(500.0, 250.0));
    }

    #[test]
    fn fit_handles_a_single_point_and_empty_drawing() {
        let mut cam = Camera {
            center: Point::new(9.0, 9.0),
            zoom: 77.0,
        };
        let pt = Bounds::from_points(Point::new(5.0, 5.0), Point::new(5.0, 5.0));
        cam.fit(Some(pt), viewport());
        assert_eq!(cam.center, Point::new(5.0, 5.0));
        assert!(cam.zoom.is_finite());
        cam.fit(None, viewport());
        assert_eq!(cam, Camera::default());
    }
}
