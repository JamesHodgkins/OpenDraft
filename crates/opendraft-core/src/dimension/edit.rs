//! Moving, turning, mirroring, scaling and stretching a dimension, and its
//! grips. All of them edit the defining points; the picture follows.

use super::{DimKind, Dimension};
use crate::{Grip, GripKind, Point};

impl Dimension {
    /// The defining points, in the order of the grips (the text grip comes
    /// after them).
    pub fn points(&self) -> Vec<Point> {
        match &self.kind {
            DimKind::Linear {
                p1, p2, line_point, ..
            } => vec![*p1, *p2, *line_point],
            DimKind::Radial {
                center,
                chord_point,
                jog,
                ..
            } => {
                let mut points = vec![*center, *chord_point];
                points.extend(jog.iter().flat_map(|(shown, at)| [*shown, *at]));
                points
            }
            DimKind::ArcLength {
                center,
                start,
                end,
                arc_point,
                ..
            } => vec![*center, *start, *end, *arc_point],
            DimKind::Angular {
                vertex,
                p1,
                p2,
                arc_point,
                ..
            } => vec![*vertex, *p1, *p2, *arc_point],
            DimKind::Ordinate {
                feature,
                leader_end,
                ..
            } => vec![*feature, *leader_end],
        }
    }

    /// The same dimension with `f` applied to every point, the text position
    /// included.
    fn mapped(&self, f: impl Fn(Point) -> Point) -> Dimension {
        let mut out = self.clone();
        match &mut out.kind {
            DimKind::Linear {
                p1, p2, line_point, ..
            } => {
                *p1 = f(*p1);
                *p2 = f(*p2);
                *line_point = f(*line_point);
            }
            DimKind::Radial {
                center,
                chord_point,
                jog,
                ..
            } => {
                *center = f(*center);
                *chord_point = f(*chord_point);
                *jog = jog.map(|(shown, at)| (f(shown), f(at)));
            }
            DimKind::ArcLength {
                center,
                start,
                end,
                arc_point,
                ..
            } => {
                *center = f(*center);
                *start = f(*start);
                *end = f(*end);
                *arc_point = f(*arc_point);
            }
            DimKind::Angular {
                vertex,
                p1,
                p2,
                arc_point,
                ..
            } => {
                *vertex = f(*vertex);
                *p1 = f(*p1);
                *p2 = f(*p2);
                *arc_point = f(*arc_point);
            }
            DimKind::Ordinate {
                feature,
                leader_end,
                ..
            } => {
                *feature = f(*feature);
                *leader_end = f(*leader_end);
            }
        }
        out.text_pos = out.text_pos.map(&f);
        out.jog_line = out.jog_line.map(&f);
        out
    }

    /// The same dimension with its extension lines slanted to `angle`
    /// radians (`None` stands them square again). Only a linear or aligned
    /// dimension has them.
    pub fn with_oblique(&self, angle: Option<f64>) -> Option<Dimension> {
        let mut out = self.clone();
        match &mut out.kind {
            DimKind::Linear { oblique, .. } => *oblique = angle,
            _ => return None,
        }
        out.measurement()?;
        Some(out)
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Dimension {
        self.mapped(|p| p.translated(dx, dy))
    }

    pub fn rotated(&self, center: Point, angle: f64) -> Dimension {
        let mut out = self.mapped(|p| p.rotated(center, angle));
        out.text_angle = out.text_angle.map(|a| a + angle);
        if let DimKind::Linear {
            rotation, oblique, ..
        } = &mut out.kind
        {
            *rotation = rotation.map(|a| a + angle);
            *oblique = oblique.map(|a| a + angle);
        }
        out
    }

    /// Scaled about `base`. Sizes set by the style (text, arrows) stay as
    /// they are, as in AutoCAD; the measurement follows the points.
    pub fn scaled(&self, base: Point, factor: f64) -> Dimension {
        let mut out = self.mapped(|p| p.scaled(base, factor));
        if let DimKind::Radial { leader_length, .. } = &mut out.kind {
            *leader_length *= factor.abs();
        }
        out
    }

    /// Reflected in the line through `a` and `b`.
    pub fn mirrored(&self, a: Point, b: Point) -> Dimension {
        let mut out = self.mapped(|p| p.mirrored(a, b));
        let axis = (b.y - a.y).atan2(b.x - a.x);
        out.text_angle = out.text_angle.map(|r| 2.0 * axis - r);
        if let DimKind::Linear {
            rotation, oblique, ..
        } = &mut out.kind
        {
            *rotation = rotation.map(|r| 2.0 * axis - r);
            *oblique = oblique.map(|r| 2.0 * axis - r);
        }
        // Mirroring reverses the turn from start to end of an arc.
        if let DimKind::ArcLength { start, end, .. } = &mut out.kind {
            std::mem::swap(start, end);
        }
        out
    }

    /// Moved by `(dx, dy)` where `inside` says a point is in the stretch
    /// window; `None` when no point is.
    pub fn stretched(&self, inside: impl Fn(Point) -> bool, dx: f64, dy: f64) -> Option<Dimension> {
        if !self.points().into_iter().any(&inside) {
            return None;
        }
        let mut out = self.clone();
        let shift = |p: Point| {
            if inside(p) {
                p.translated(dx, dy)
            } else {
                p
            }
        };
        match &mut out.kind {
            DimKind::Linear {
                p1, p2, line_point, ..
            } => {
                *p1 = shift(*p1);
                *p2 = shift(*p2);
                *line_point = shift(*line_point);
            }
            DimKind::Radial {
                center,
                chord_point,
                jog,
                ..
            } => {
                *center = shift(*center);
                *chord_point = shift(*chord_point);
                *jog = jog.map(|(shown, at)| (shift(shown), shift(at)));
            }
            DimKind::ArcLength {
                center,
                start,
                end,
                arc_point,
                ..
            } => {
                *center = shift(*center);
                *start = shift(*start);
                *end = shift(*end);
                *arc_point = shift(*arc_point);
            }
            DimKind::Angular {
                vertex,
                p1,
                p2,
                arc_point,
                ..
            } => {
                *vertex = shift(*vertex);
                *p1 = shift(*p1);
                *p2 = shift(*p2);
                *arc_point = shift(*arc_point);
            }
            DimKind::Ordinate {
                feature,
                leader_end,
                ..
            } => {
                *feature = shift(*feature);
                *leader_end = shift(*leader_end);
            }
        }
        out.text_pos = out.text_pos.map(shift);
        out.jog_line = out.jog_line.map(shift);
        Some(out)
    }

    /// The grips: each defining point, then the text. A linear dimension's
    /// third grip sits in the middle of its dimension line, wherever that
    /// line was first picked.
    pub fn grips(&self) -> Vec<Grip> {
        let mut grips: Vec<Grip> = self
            .points()
            .into_iter()
            .enumerate()
            .map(|(i, at)| Grip {
                at: match (i, super::linear::line_middle(&self.kind)) {
                    (2, Some(middle)) => middle,
                    _ => at,
                },
                kind: GripKind::Stretch,
            })
            .collect();
        if let Some(at) = self.text_centre() {
            grips.push(Grip {
                at,
                kind: GripKind::Move,
            });
        }
        grips
    }

    /// With grip `index` dragged to `to`. Dragging the text fixes its
    /// position; the rest move one point.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<Dimension> {
        if !(to.x.is_finite() && to.y.is_finite()) {
            return None;
        }
        let mut out = self.clone();
        let count = self.points().len();
        if index == count {
            self.text_centre()?;
            out.text_pos = Some(to);
            return Some(out);
        }
        match (&mut out.kind, index) {
            (DimKind::Linear { p1, .. }, 0) => *p1 = to,
            (DimKind::Linear { p2, .. }, 1) => *p2 = to,
            (DimKind::Linear { line_point, .. }, 2) => *line_point = to,
            (DimKind::Radial { center, .. }, 0) => *center = to,
            (DimKind::Radial { chord_point, .. }, 1) => *chord_point = to,
            (DimKind::Radial { jog: Some(j), .. }, 2) => j.0 = to,
            (DimKind::Radial { jog: Some(j), .. }, 3) => j.1 = to,
            (DimKind::ArcLength { center, .. }, 0) => *center = to,
            (DimKind::ArcLength { start, .. }, 1) => *start = to,
            (DimKind::ArcLength { end, .. }, 2) => *end = to,
            (DimKind::ArcLength { arc_point, .. }, 3) => *arc_point = to,
            (DimKind::Angular { vertex, .. }, 0) => *vertex = to,
            (DimKind::Angular { p1, .. }, 1) => *p1 = to,
            (DimKind::Angular { p2, .. }, 2) => *p2 = to,
            (DimKind::Angular { arc_point, .. }, 3) => *arc_point = to,
            (DimKind::Ordinate { feature, .. }, 0) => *feature = to,
            (DimKind::Ordinate { leader_end, .. }, 1) => *leader_end = to,
            _ => return None,
        }
        // A dimension left with nothing to measure is refused.
        out.measurement()?;
        Some(out)
    }

    /// With the text turned to `angle` radians (`None` for the style's own
    /// angle).
    pub fn with_text_angle(&self, angle: Option<f64>) -> Dimension {
        Dimension {
            text_angle: angle,
            ..self.clone()
        }
    }

    /// With the text at the left (1), right (2) or centre (0) of the
    /// dimension line, as DIMTEDIT's options. Only a linear, aligned or arc
    /// length dimension has a line to justify along.
    pub fn with_text_justified(&self, just: u8) -> Option<Dimension> {
        if matches!(self.kind, DimKind::Radial { .. }) {
            return None;
        }
        let mut out = self.clone();
        out.style.text_just = just.min(2);
        out.text_pos = None;
        Some(out)
    }

    /// Back to the style's own text position.
    pub fn reset_text(&mut self) {
        self.text_pos = None;
        self.text_angle = None;
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::DimStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn linear() -> Dimension {
        Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &DimStyle::default())
    }

    #[test]
    fn text_angle_turns_with_the_dimension_and_resets_with_the_text() {
        let d = linear().with_text_angle(Some(0.5));
        let turned = d.rotated(p(0.0, 0.0), FRAC_PI_2);
        assert_eq!(turned.text_angle, Some(0.5 + FRAC_PI_2));
        let mirrored = d.mirrored(p(0.0, 0.0), p(1.0, 0.0));
        assert_eq!(mirrored.text_angle, Some(-0.5));
        let mut again = d;
        again.reset_text();
        assert_eq!(again.text_angle, None);
    }

    #[test]
    fn text_angle_turns_the_drawn_text() {
        let angles = |d: &Dimension| -> Vec<f64> {
            d.picture()
                .into_iter()
                .filter_map(|e| match e {
                    crate::Entity::MText(t) => Some(t.direction.y.atan2(t.direction.x)),
                    _ => None,
                })
                .collect()
        };
        let d = linear();
        assert!(angles(&d)[0].abs() < 1e-9);
        let turned = d.with_text_angle(Some(FRAC_PI_2));
        assert!((angles(&turned)[0] - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn justifying_text_clears_its_position_and_skips_radial() {
        let mut d = linear();
        d.text_pos = Some(p(4.0, 9.0));
        let left = d.with_text_justified(1).unwrap();
        assert_eq!((left.text_pos, left.style.text_just), (None, 1));
        let r = Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &DimStyle::default());
        assert!(r.with_text_justified(1).is_none());
    }

    #[test]
    fn oblique_slants_only_linear_dimensions() {
        let slanted = linear().with_oblique(Some(1.0)).unwrap();
        assert!(matches!(
            slanted.kind,
            DimKind::Linear {
                oblique: Some(a), ..
            } if a == 1.0
        ));
        let r = Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &DimStyle::default());
        assert!(r.with_oblique(Some(1.0)).is_none());
    }

    #[test]
    fn the_dimension_line_grip_sits_in_the_middle_of_the_line() {
        let d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(7.0, 5.0), &DimStyle::default());
        assert_eq!(d.grips()[2].at, p(5.0, 5.0));
        let moved = d.with_grip_moved(2, p(5.0, 8.0)).unwrap();
        assert_eq!(moved.grips()[2].at, p(5.0, 8.0));
    }

    #[test]
    fn moving_keeps_the_measurement() {
        let d = linear().translated(3.0, 4.0);
        assert_eq!(d.measurement(), Some(10.0));
        assert_eq!(d.points()[0], p(3.0, 4.0));
    }

    #[test]
    fn turning_a_rotated_dimension_turns_its_direction() {
        let d = Dimension::linear(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 5.0),
            0.0,
            &DimStyle::default(),
        )
        .rotated(p(0.0, 0.0), FRAC_PI_2);
        let DimKind::Linear { rotation, .. } = d.kind else {
            panic!("linear");
        };
        assert!((rotation.unwrap() - FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn scaling_scales_the_measurement() {
        let d = linear().scaled(p(0.0, 0.0), 2.0);
        assert_eq!(d.measurement(), Some(20.0));
        assert_eq!(d.style.text_height, 2.5, "text keeps its size");
    }

    #[test]
    fn mirroring_keeps_the_measurement() {
        let d = Dimension::linear(
            p(1.0, 0.0),
            p(11.0, 3.0),
            p(0.0, 5.0),
            0.0,
            &DimStyle::default(),
        );
        let m = d.mirrored(p(0.0, 0.0), p(0.0, 1.0));
        assert!((m.measurement().unwrap() - 10.0).abs() < 1e-9);
        let DimKind::Linear { rotation, .. } = m.kind else {
            panic!("linear");
        };
        assert!((rotation.unwrap() - 2.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn mirroring_an_arc_dimension_keeps_its_length() {
        let d = Dimension::arc_length(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 10.0),
            p(12.0, 12.0),
            &DimStyle::default(),
        );
        let m = d.mirrored(p(0.0, 0.0), p(1.0, 0.0));
        assert!((m.measurement().unwrap() - d.measurement().unwrap()).abs() < 1e-9);
    }

    #[test]
    fn stretching_moves_only_the_points_in_the_window() {
        let d = linear().stretched(|q| q.x > 5.0, 5.0, 0.0).unwrap();
        assert_eq!(d.measurement(), Some(15.0));
        assert!(linear().stretched(|q| q.x > 50.0, 5.0, 0.0).is_none());
    }

    #[test]
    fn grips_are_the_points_then_the_text() {
        let d = linear();
        let grips = d.grips();
        assert_eq!(grips.len(), 4);
        assert_eq!(grips[0].at, p(0.0, 0.0));
        assert_eq!(grips[3].kind, GripKind::Move);
        assert_eq!(Some(grips[3].at), d.text_centre());
    }

    #[test]
    fn dragging_the_text_grip_fixes_the_text() {
        let d = linear().with_grip_moved(3, p(4.0, 9.0)).unwrap();
        assert_eq!(d.text_pos, Some(p(4.0, 9.0)));
        let mut again = d;
        again.reset_text();
        assert_eq!(again.text_pos, None);
    }

    #[test]
    fn dragging_a_point_grip_changes_the_measurement() {
        let d = linear().with_grip_moved(1, p(20.0, 0.0)).unwrap();
        assert_eq!(d.measurement(), Some(20.0));
    }

    #[test]
    fn dragging_to_nothing_is_refused() {
        let r = Dimension::radius(p(0.0, 0.0), p(5.0, 0.0), &DimStyle::default());
        assert!(r.with_grip_moved(1, p(0.0, 0.0)).is_none());
        assert!(r.with_grip_moved(1, p(f64::NAN, 0.0)).is_none());
        assert!(r.with_grip_moved(9, p(1.0, 0.0)).is_none());
    }
}
