//! Moving, turning, scaling and mirroring an ellipse, all exact, and the
//! check that stored values are canonical.

use std::f64::consts::TAU;

use super::{vector, Ellipse, EllipseError, PARAM_EPS};
use crate::Point;

impl Ellipse {
    pub fn translated(&self, dx: f64, dy: f64) -> Ellipse {
        Ellipse {
            center: self.center.translated(dx, dy),
            ..*self
        }
    }

    /// Turned `angle` radians counter-clockwise about `about`; the parameters
    /// stay as they are.
    pub fn rotated(&self, about: Point, angle: f64) -> Ellipse {
        Ellipse {
            center: self.center.rotated(about, angle),
            major_axis: self.major_axis.rotated(Point::default(), angle),
            ..*self
        }
    }

    /// Enlarged by `factor` about `base`. A negative factor turns it half
    /// round as well, which the parameters follow unchanged.
    pub fn scaled(&self, base: Point, factor: f64) -> Ellipse {
        Ellipse {
            center: self.center.scaled(base, factor),
            major_axis: vector(self.major_axis.x * factor, self.major_axis.y * factor),
            ..*self
        }
    }

    /// The reflection in the line through `a` and `b`. Reflecting reverses
    /// the direction of travel: the point at `t` goes to the reflected
    /// ellipse's `-t`, so the arc from `start` to `end` becomes the one from
    /// `-end` to `-start`.
    pub fn mirrored(&self, a: Point, b: Point) -> Ellipse {
        let center = self.center.mirrored(a, b);
        let tip = Point::new(
            self.center.x + self.major_axis.x,
            self.center.y + self.major_axis.y,
        )
        .mirrored(a, b);
        let (start, end) = if self.is_full() {
            (0.0, TAU)
        } else {
            let s = (-self.end).rem_euclid(TAU);
            let s = if s >= TAU { 0.0 } else { s };
            (s, s + self.sweep())
        };
        Ellipse {
            center,
            major_axis: vector(tip.x - center.x, tip.y - center.y),
            radius_ratio: self.radius_ratio,
            start,
            end,
        }
    }

    /// Whether the stored values are those [`new`](Self::new) would give,
    /// and everything worked out from them stays finite.
    pub fn validate(&self) -> Result<(), EllipseError> {
        let again = Self::new(
            self.center,
            self.major_axis,
            self.radius_ratio,
            self.start,
            self.end,
        )?;
        let same = (again.start - self.start).abs() <= PARAM_EPS
            && (again.end - self.end).abs() <= PARAM_EPS
            && again.radius_ratio == self.radius_ratio;
        if !same || !(0.0..TAU).contains(&self.start) {
            return Err(EllipseError::NotCanonical);
        }
        let b = self.bounds();
        if ![b.min.x, b.min.y, b.max.x, b.max.y]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(EllipseError::NonFinite);
        }
        Ok(())
    }
}
