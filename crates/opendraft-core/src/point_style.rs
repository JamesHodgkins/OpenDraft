//! How a point object is shown (`PDMODE` and `PDSIZE`). Pure geometry: the
//! canvas works out the size in drawing units when `PDSIZE` is relative to the
//! screen.

use crate::{Entity, Point};

/// The drawing-wide look of every point object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointStyle {
    /// `PDMODE`: 0 a dot, 1 nothing, 2 a plus, 3 an X, 4 a tick up; add 32 for
    /// a circle round it and 64 for a square.
    pub mode: i32,
    /// `PDSIZE`: above zero a size in drawing units, zero 5% of the screen
    /// height, below zero that percentage of it.
    pub size: f64,
}

impl Default for PointStyle {
    fn default() -> Self {
        Self { mode: 0, size: 0.0 }
    }
}

/// What a point looks like at a size.
#[derive(Clone, Debug, PartialEq)]
pub struct PointMarks {
    /// Whether to paint a dot at the point itself (mode 0).
    pub dot: bool,
    /// The lines, circle and square round it.
    pub shapes: Vec<Entity>,
}

impl PointStyle {
    /// Whether `mode` is one `PDMODE` allows: a base 0 to 4 with at most the
    /// circle and square bits.
    pub fn valid_mode(mode: i32) -> bool {
        (0..=4).contains(&(mode & 0x1f)) && mode & !0x7f == 0
    }

    /// The size in drawing units, given the height of the screen in drawing
    /// units. `PDSIZE` of zero is 5% of it, a negative one that percentage.
    pub fn size_in_units(&self, screen_height: f64) -> f64 {
        if self.size > 0.0 {
            self.size
        } else if self.size == 0.0 {
            screen_height * 0.05
        } else {
            screen_height * -self.size / 100.0
        }
    }

    /// The marks drawn for a point at `at` when the shapes are `size` across.
    pub fn marks(&self, at: Point, size: f64) -> PointMarks {
        let half = size / 2.0;
        let line = |dx0: f64, dy0: f64, dx1: f64, dy1: f64| Entity::Line {
            start: Point::new(at.x + dx0, at.y + dy0),
            end: Point::new(at.x + dx1, at.y + dy1),
        };
        let mut shapes = Vec::new();
        match self.mode & 0x1f {
            2 => {
                shapes.push(line(-half, 0.0, half, 0.0));
                shapes.push(line(0.0, -half, 0.0, half));
            }
            3 => {
                shapes.push(line(-half, -half, half, half));
                shapes.push(line(-half, half, half, -half));
            }
            4 => shapes.push(line(0.0, 0.0, 0.0, half)),
            _ => {}
        }
        if self.mode & 32 != 0 {
            shapes.push(Entity::Circle {
                center: at,
                radius: half,
            });
        }
        if self.mode & 64 != 0 {
            shapes.push(Entity::Rectangle {
                min: Point::new(at.x - half, at.y - half),
                max: Point::new(at.x + half, at.y + half),
            });
        }
        PointMarks {
            dot: self.mode & 0x1f == 0,
            shapes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(mode: i32) -> PointStyle {
        PointStyle { mode, size: 1.0 }
    }

    #[test]
    fn mode_zero_is_a_dot_and_one_is_nothing() {
        let dot = style(0).marks(Point::new(1.0, 2.0), 4.0);
        assert!(dot.dot && dot.shapes.is_empty());
        let none = style(1).marks(Point::new(1.0, 2.0), 4.0);
        assert!(!none.dot && none.shapes.is_empty());
    }

    #[test]
    fn plus_x_and_tick_are_lines_across_the_size() {
        let at = Point::new(10.0, 10.0);
        assert_eq!(style(2).marks(at, 4.0).shapes.len(), 2);
        assert_eq!(style(3).marks(at, 4.0).shapes.len(), 2);
        let tick = style(4).marks(at, 4.0).shapes;
        assert_eq!(
            tick,
            [Entity::Line {
                start: at,
                end: Point::new(10.0, 12.0)
            }]
        );
    }

    #[test]
    fn circle_and_square_bits_add_to_the_base() {
        let at = Point::new(0.0, 0.0);
        let both = style(2 + 32 + 64).marks(at, 2.0);
        assert!(!both.dot);
        assert_eq!(both.shapes.len(), 4);
        assert!(both
            .shapes
            .iter()
            .any(|e| matches!(e, Entity::Circle { radius, .. } if *radius == 1.0)));
        let only_circle = style(33).marks(at, 2.0);
        assert_eq!(only_circle.shapes.len(), 1);
    }

    #[test]
    fn valid_modes_are_the_documented_ones() {
        for mode in [0, 1, 2, 3, 4, 32, 36, 64, 67, 96, 100] {
            assert!(PointStyle::valid_mode(mode), "{mode}");
        }
        for mode in [-1, 5, 31, 128, 200] {
            assert!(!PointStyle::valid_mode(mode), "{mode}");
        }
    }

    #[test]
    fn size_is_absolute_or_a_share_of_the_screen() {
        let mut s = PointStyle::default();
        assert_eq!(s.size_in_units(200.0), 10.0);
        s.size = -10.0;
        assert_eq!(s.size_in_units(200.0), 20.0);
        s.size = 3.0;
        assert_eq!(s.size_in_units(200.0), 3.0);
    }
}
