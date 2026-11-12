//! Moving, turning, mirroring, scaling and mapping a spline, all exact (a
//! NURBS curve maps with its control points), and its grips.
//!
//! The control points and fit points map like points, end tangents like
//! vectors; knots, weights and the degree stay. Reflection keeps the order
//! of the control points, so the mirrored curve runs the other way round
//! the plane, which is its exact image.

use super::{distance, epsilon, Spline};
use crate::{Affine, Grip, GripKind, Point};

impl Spline {
    /// Each point through `point`, each tangent through `vector`; distances
    /// (the tolerances) grow by `size`.
    fn mapped_with(
        &self,
        point: impl Fn(Point) -> Point,
        vector: impl Fn(Point) -> Point,
        size: f64,
    ) -> Spline {
        let mut out = self.clone();
        for q in &mut out.control_points {
            *q = point(*q);
        }
        out.control_point_tolerance *= size.abs();
        if let Some(fit) = &mut out.fit {
            for q in &mut fit.points {
                *q = point(*q);
            }
            fit.start_tangent = fit.start_tangent.map(&vector);
            fit.end_tangent = fit.end_tangent.map(&vector);
            fit.tolerance *= size.abs();
        }
        out
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Spline {
        self.mapped_with(|q| q.translated(dx, dy), |v| v, 1.0)
    }

    /// Turned `angle` radians counter-clockwise about `about`.
    pub fn rotated(&self, about: Point, angle: f64) -> Spline {
        let origin = Point::default();
        self.mapped_with(
            |q| q.rotated(about, angle),
            |v| v.rotated(origin, angle),
            1.0,
        )
    }

    /// Enlarged by `factor` about `base`.
    pub fn scaled(&self, base: Point, factor: f64) -> Spline {
        self.mapped_with(
            |q| q.scaled(base, factor),
            |v| Point::new(v.x * factor, v.y * factor),
            factor,
        )
    }

    /// The reflection in the line through `a` and `b`.
    pub fn mirrored(&self, a: Point, b: Point) -> Spline {
        let (origin, along) = (Point::default(), Point::new(b.x - a.x, b.y - a.y));
        self.mapped_with(|q| q.mirrored(a, b), |v| v.mirrored(origin, along), 1.0)
    }

    /// The spline under any affine map (a block's uneven scale), exactly.
    pub fn transformed(&self, map: &Affine) -> Spline {
        let linear = Affine {
            tx: 0.0,
            ty: 0.0,
            ..*map
        };
        self.mapped_with(|q| map.apply(q), |v| linear.apply(v), map.mean_scale())
    }

    /// The points grips sit on: the fit points of a spline that can be
    /// refitted, otherwise the control points (on a closed spline, a control
    /// point that repeats an earlier one shows once).
    fn grip_points(&self) -> Vec<Point> {
        if self.is_refittable() {
            return self
                .fit
                .as_ref()
                .map_or_else(Vec::new, |f| f.points.clone());
        }
        let near = epsilon(self.scale());
        let mut shown: Vec<Point> = Vec::new();
        for q in &self.control_points {
            if !(self.closed && shown.iter().any(|s| distance(*s, *q) <= near)) {
                shown.push(*q);
            }
        }
        shown
    }

    /// One grip per fit point or control point (see `grip_points`), each
    /// moving that point.
    pub fn grips(&self) -> Vec<Grip> {
        self.grip_points()
            .into_iter()
            .map(|at| Grip {
                at,
                kind: GripKind::Stretch,
            })
            .collect()
    }

    /// The spline with grip `index` dragged to `to`, or `None` when that
    /// leaves no valid spline. A fit point refits the curve through the
    /// points; a control point moves (with any that coincide with it on a
    /// closed spline) and the fit data, which no longer describes the
    /// curve, is dropped.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<Spline> {
        let from = *self.grip_points().get(index)?;
        self.with_points_moved(
            |q| distance(q, from) <= epsilon(self.scale()),
            to.x - from.x,
            to.y - from.y,
        )
    }

    /// The spline with its defining points for which `inside` is true moved
    /// by `(dx, dy)` (the fit points of one that can be refitted, otherwise
    /// the control points), or `None` when none is or the result is not a
    /// valid spline. Moving them all moves the curve and keeps its fit data.
    pub fn stretched(&self, inside: impl Fn(Point) -> bool, dx: f64, dy: f64) -> Option<Spline> {
        self.with_points_moved(inside, dx, dy)
    }

    fn with_points_moved(
        &self,
        caught: impl Fn(Point) -> bool,
        dx: f64,
        dy: f64,
    ) -> Option<Spline> {
        let shift = |q: Point| if caught(q) { q.translated(dx, dy) } else { q };
        let defining = match (&self.fit, self.is_refittable()) {
            (Some(fit), true) => &fit.points,
            _ => &self.control_points,
        };
        let moved = defining.iter().filter(|q| caught(**q)).count();
        if moved == 0 {
            return None;
        }
        if moved == defining.len() {
            return Some(self.translated(dx, dy));
        }
        let out = if self.is_refittable() {
            self.refitted(defining.iter().map(|q| shift(*q)).collect())
                .ok()?
        } else {
            let mut out = self.clone();
            out.control_points = self.control_points.iter().map(|q| shift(*q)).collect();
            out.fit = None;
            out
        };
        out.validate().ok().map(|()| out)
    }
}
