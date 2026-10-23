//! A 2D affine map: what an insert does to its block's coordinates.

use crate::Point;

/// `x' = xx·x + xy·y + tx`, `y' = yx·x + yy·y + ty`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub tx: f64,
    pub ty: f64,
}

/// An affine map that keeps shapes: a turn, one size factor, an optional
/// reflection and a move. `apply(p) = R(rotation)·scale·F·p + (tx, ty)`,
/// where `F` reflects in the x axis when `mirrored`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Similarity {
    pub rotation: f64,
    pub scale: f64,
    pub mirrored: bool,
    pub tx: f64,
    pub ty: f64,
}

/// Relative tolerance for deciding that a map keeps shapes.
const SHAPE_TOLERANCE: f64 = 1e-9;

impl Affine {
    pub const IDENTITY: Affine = Affine {
        xx: 1.0,
        xy: 0.0,
        yx: 0.0,
        yy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn translation(dx: f64, dy: f64) -> Self {
        Self {
            tx: dx,
            ty: dy,
            ..Self::IDENTITY
        }
    }

    pub fn rotation(angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self {
            xx: cos,
            xy: -sin,
            yx: sin,
            yy: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn scaling(sx: f64, sy: f64) -> Self {
        Self {
            xx: sx,
            yy: sy,
            ..Self::IDENTITY
        }
    }

    /// The reflection in the line through `a` and `b` (the identity for a
    /// zero-length line, as `Point::mirrored`).
    pub fn reflection(a: Point, b: Point) -> Self {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        if len_sq == 0.0 {
            return Self::IDENTITY;
        }
        let (c, s) = ((dx * dx - dy * dy) / len_sq, 2.0 * dx * dy / len_sq);
        let flip = Self {
            xx: c,
            xy: s,
            yx: s,
            yy: -c,
            tx: 0.0,
            ty: 0.0,
        };
        Self::translation(-a.x, -a.y)
            .then(&flip)
            .then(&Self::translation(a.x, a.y))
    }

    /// This map followed by `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        Affine {
            xx: next.xx * self.xx + next.xy * self.yx,
            xy: next.xx * self.xy + next.xy * self.yy,
            yx: next.yx * self.xx + next.yy * self.yx,
            yy: next.yx * self.xy + next.yy * self.yy,
            tx: next.xx * self.tx + next.xy * self.ty + next.tx,
            ty: next.yx * self.tx + next.yy * self.ty + next.ty,
        }
    }

    pub fn apply(&self, p: Point) -> Point {
        Point::new(
            self.xx * p.x + self.xy * p.y + self.tx,
            self.yx * p.x + self.yy * p.y + self.ty,
        )
    }

    pub fn determinant(&self) -> f64 {
        self.xx * self.yy - self.xy * self.yx
    }

    /// The inverse map, or `None` when this one squashes the plane flat.
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.determinant();
        let size = self.xx.abs() + self.xy.abs() + self.yx.abs() + self.yy.abs();
        if !det.is_finite() || det.abs() <= 1e-12 * size * size {
            return None;
        }
        let (xx, xy, yx, yy) = (self.yy / det, -self.xy / det, -self.yx / det, self.xx / det);
        Some(Affine {
            xx,
            xy,
            yx,
            yy,
            tx: -(xx * self.tx + xy * self.ty),
            ty: -(yx * self.tx + yy * self.ty),
        })
    }

    /// This map as a turn, size and reflection, or `None` when it stretches
    /// one direction more than another (a circle would become an ellipse).
    pub fn similarity(&self) -> Option<Similarity> {
        let (c1, c2) = ((self.xx, self.yx), (self.xy, self.yy));
        let (l1, l2) = (c1.0.hypot(c1.1), c2.0.hypot(c2.1));
        let size = l1.max(l2);
        if !(size.is_finite() && size > 0.0) {
            return None;
        }
        let dot = c1.0 * c2.0 + c1.1 * c2.1;
        if (l1 - l2).abs() > SHAPE_TOLERANCE * size || dot.abs() > SHAPE_TOLERANCE * size * size {
            return None;
        }
        Some(Similarity {
            rotation: c1.1.atan2(c1.0),
            scale: l1,
            mirrored: self.determinant() < 0.0,
            tx: self.tx,
            ty: self.ty,
        })
    }

    /// How much the map enlarges lengths on average: the square root of the
    /// area factor.
    pub fn mean_scale(&self) -> f64 {
        self.determinant().abs().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn composing_applies_in_order() {
        let m = Affine::translation(1.0, 0.0).then(&Affine::rotation(FRAC_PI_2));
        assert!(near(m.apply(Point::new(1.0, 0.0)), Point::new(0.0, 2.0)));
    }

    #[test]
    fn the_inverse_undoes_the_map() {
        let m = Affine::scaling(2.0, -3.0)
            .then(&Affine::rotation(0.7))
            .then(&Affine::translation(5.0, -1.0));
        let inv = m.inverse().unwrap();
        let p = Point::new(3.5, -2.25);
        assert!(near(inv.apply(m.apply(p)), p));
        assert_eq!(Affine::scaling(0.0, 1.0).inverse(), None);
    }

    #[test]
    fn a_reflection_matches_point_mirroring() {
        let (a, b) = (Point::new(-3.0, 1.0), Point::new(4.0, 5.0));
        let m = Affine::reflection(a, b);
        let p = Point::new(2.0, -7.0);
        assert!(near(m.apply(p), p.mirrored(a, b)));
        let s = m.similarity().unwrap();
        assert!(s.mirrored && (s.scale - 1.0).abs() < 1e-12);
    }

    #[test]
    fn only_shape_keeping_maps_are_similarities() {
        let m = Affine::scaling(2.0, 2.0).then(&Affine::rotation(1.0));
        let s = m.similarity().unwrap();
        assert!((s.rotation - 1.0).abs() < 1e-12 && (s.scale - 2.0).abs() < 1e-12);
        assert!(!s.mirrored);
        assert_eq!(Affine::scaling(2.0, 1.0).similarity(), None);
        let flipped = Affine::scaling(-2.0, 2.0).similarity().unwrap();
        assert!(flipped.mirrored);
    }
}
