//! Moving, turning, mirroring and scaling an insert: the edit combines with
//! the insert's own transform, and the block itself never changes.

use std::f64::consts::PI;

use crate::block::affine::Similarity;
use crate::block::{Affine, Insert};
use crate::{arc, Point};

impl Similarity {
    pub fn apply(&self, p: Point) -> Point {
        let y = if self.mirrored { -p.y } else { p.y };
        let (sin, cos) = self.rotation.sin_cos();
        let (x, y) = (p.x * self.scale, y * self.scale);
        Point::new(x * cos - y * sin + self.tx, x * sin + y * cos + self.ty)
    }
}

impl Insert {
    /// The insert after a shape-keeping map. A reflection is kept as a
    /// negative x scale, as AutoCAD writes a mirrored block.
    pub fn transformed(&self, s: &Similarity) -> Insert {
        let (rotation, scale_x) = if s.mirrored {
            (s.rotation - self.rotation + PI, -self.scale_x)
        } else {
            (s.rotation + self.rotation, self.scale_x)
        };
        Insert {
            block: self.block.clone(),
            insertion: s.apply(self.insertion),
            scale_x: scale_x * s.scale,
            scale_y: self.scale_y * s.scale,
            rotation: arc::normalize(rotation),
            attributes: self.attributes.clone(),
        }
    }

    fn mapped_by(&self, map: Affine) -> Insert {
        match map.similarity() {
            Some(s) => self.transformed(&s),
            None => self.clone(),
        }
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Insert {
        Insert {
            insertion: self.insertion.translated(dx, dy),
            ..self.clone()
        }
    }

    pub fn rotated(&self, center: Point, angle: f64) -> Insert {
        self.mapped_by(
            Affine::translation(-center.x, -center.y)
                .then(&Affine::rotation(angle))
                .then(&Affine::translation(center.x, center.y)),
        )
    }

    pub fn mirrored(&self, a: Point, b: Point) -> Insert {
        self.mapped_by(Affine::reflection(a, b))
    }

    /// Enlarged by `factor` about `base`. A zero factor changes nothing; the
    /// caller rejects it.
    pub fn scaled(&self, base: Point, factor: f64) -> Insert {
        self.mapped_by(
            Affine::translation(-base.x, -base.y)
                .then(&Affine::scaling(factor, factor))
                .then(&Affine::translation(base.x, base.y)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::tests::{near, square_block};
    use crate::Entity;

    /// The edit made to the insert draws exactly what the same edit made to
    /// its picture draws.
    fn same_as_picture(edit: impl Fn(&Entity) -> Entity) {
        let mut insert = Insert::new(square_block(), Point::new(3.0, -2.0));
        insert.rotation = 0.4;
        insert.scale_x = 1.5;
        insert.scale_y = 1.5;
        let moved = edit(&Entity::Insert(Box::new(insert.clone())));
        let Entity::Insert(moved) = moved else {
            panic!("still an insert")
        };
        let want: Vec<Entity> = insert.picture().iter().map(&edit).collect();
        let got = moved.picture();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) {
            let (gb, wb) = (g.bounds(), w.bounds());
            assert!(
                near(gb.min, wb.min) && near(gb.max, wb.max),
                "{g:?} != {w:?}"
            );
        }
    }

    #[test]
    fn moving_turning_scaling_and_mirroring_follow_the_picture() {
        same_as_picture(|e| e.translated(4.0, 1.0));
        same_as_picture(|e| e.rotated(Point::new(1.0, 2.0), 1.1));
        same_as_picture(|e| e.scaled(Point::new(-1.0, 0.5), 2.5));
        same_as_picture(|e| e.mirrored(Point::new(0.0, 1.0), Point::new(2.0, 4.0)));
    }

    #[test]
    fn mirroring_is_kept_as_a_negative_x_scale() {
        let insert = Insert::new(square_block(), Point::new(1.0, 0.0));
        let m = insert.mirrored(Point::new(0.0, 0.0), Point::new(0.0, 1.0));
        assert!(m.scale_x < 0.0 && m.scale_y > 0.0);
        assert!(near(m.insertion, Point::new(-1.0, 0.0)));
        let twice = m.mirrored(Point::new(0.0, 0.0), Point::new(0.0, 1.0));
        assert!(twice.scale_x > 0.0 && near(twice.insertion, insert.insertion));
        assert!(twice.rotation.abs() < 1e-9 || (twice.rotation - 2.0 * PI).abs() < 1e-9);
    }
}
