//! Screen-space flipping with a dead band, independent of text widget focus.
use eframe::egui::{Pos2, Rect, Vec2};

pub fn cursor_clearance(cursor: Pos2) -> Rect {
    Rect::from_center_size(cursor, Vec2::splat(56.0))
}

/// Move as little as possible while keeping the entire box inside the canvas.
pub fn avoid(rect: Rect, canvas: Rect, obstacles: &[Rect]) -> Rect {
    let bounds = canvas.shrink(8.0);
    let size = rect.size().min(bounds.size().max(Vec2::splat(1.0)));
    let max = bounds.max - size;
    let preferred = Pos2::new(
        rect.left().clamp(bounds.left(), max.x),
        rect.top().clamp(bounds.top(), max.y),
    );
    let mut xs = vec![preferred.x, bounds.left(), max.x];
    let mut ys = vec![preferred.y, bounds.top(), max.y];
    for obstacle in obstacles {
        xs.extend([obstacle.left() - size.x - 1.0, obstacle.right() + 1.0]);
        ys.extend([obstacle.top() - size.y - 1.0, obstacle.bottom() + 1.0]);
    }
    let mut best = Rect::from_min_size(preferred, size);
    let mut distance = f32::INFINITY;
    for x in xs {
        for &y in &ys {
            let candidate = Rect::from_min_size(Pos2::new(x, y), size);
            if !bounds.contains_rect(candidate)
                || obstacles.iter().any(|o| candidate.intersects(*o))
            {
                continue;
            }
            let displacement = candidate.min.distance_sq(preferred);
            if displacement < distance {
                best = candidate;
                distance = displacement;
            }
        }
    }
    best
}

#[derive(Clone, Default)]
pub struct Placement {
    left: bool,
    up: bool,
}

impl Placement {
    pub fn place(&mut self, anchor: Pos2, size: Vec2, canvas: Rect) -> Rect {
        let bounds = canvas.shrink(8.0);
        let size = size.min(bounds.size().max(Vec2::splat(1.0)));
        if anchor.x + 16.0 + size.x > bounds.right() {
            self.left = true;
        } else if anchor.x + 16.0 + size.x + 24.0 < bounds.right() {
            self.left = false;
        }
        if anchor.y + 18.0 + size.y > bounds.bottom() {
            self.up = true;
        } else if anchor.y + 18.0 + size.y + 24.0 < bounds.bottom() {
            self.up = false;
        }
        let x = if self.left {
            anchor.x - size.x - 16.0
        } else {
            anchor.x + 16.0
        };
        let y = if self.up {
            anchor.y - size.y - 18.0
        } else {
            anchor.y + 18.0
        };
        Rect::from_min_size(
            Pos2::new(
                x.clamp(bounds.left(), bounds.right() - size.x),
                y.clamp(bounds.top(), bounds.bottom() - size.y),
            ),
            size,
        )
    }

    pub fn menu(&self, prompt: Rect, size: Vec2, canvas: Rect) -> Rect {
        let bounds = canvas.shrink(8.0);
        let size = size.min(bounds.size().max(Vec2::splat(1.0)));
        let y = if self.up || prompt.bottom() + size.y > bounds.bottom() {
            prompt.top() - size.y - 2.0
        } else {
            prompt.bottom() + 2.0
        };
        Rect::from_min_size(
            Pos2::new(
                prompt.left().clamp(bounds.left(), bounds.right() - size.x),
                y.clamp(bounds.top(), bounds.bottom() - size.y),
            ),
            size,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_resolution_preserves_clear_placements() {
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 300.0));
        let cursor = cursor_clearance(Pos2::new(250.0, 150.0));
        let preferred = Rect::from_min_size(Pos2::new(300.0, 100.0), Vec2::new(84.0, 24.0));
        assert_eq!(avoid(preferred, canvas, &[cursor]), preferred);
        let colliding = preferred.translate(Vec2::new(-50.0, 30.0));
        let adjusted = avoid(colliding, canvas, &[cursor]);
        assert!(!adjusted.intersects(cursor));
        assert!(canvas.contains_rect(adjusted));
        assert_eq!(avoid(adjusted, canvas, &[cursor]), adjusted);
    }

    #[test]
    fn edge_flip_has_hysteresis_and_stays_inside_canvas() {
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 300.0));
        let size = Vec2::new(100.0, 24.0);
        let mut p = Placement::default();
        let flipped = p.place(Pos2::new(380.0, 270.0), size, canvas);
        assert!(p.left && p.up && canvas.contains_rect(flipped));
        p.place(Pos2::new(375.0, 265.0), size, canvas);
        assert!(p.left && p.up);
        p.place(Pos2::new(340.0, 220.0), size, canvas);
        assert!(!p.left && !p.up);
    }
}
