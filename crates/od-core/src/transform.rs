//! Moving, rotating, mirroring and scaling entities. Pure geometry over [`Entity`].

use crate::{arc, bulge, Bounds, Entity, Point};

impl Point {
    /// This point turned `angle` radians counter-clockwise about `center`.
    pub fn rotated(self, center: Point, angle: f64) -> Point {
        let (sin, cos) = angle.sin_cos();
        let (dx, dy) = (self.x - center.x, self.y - center.y);
        Point::new(
            center.x + dx * cos - dy * sin,
            center.y + dx * sin + dy * cos,
        )
    }

    pub fn translated(self, dx: f64, dy: f64) -> Point {
        Point::new(self.x + dx, self.y + dy)
    }

    /// This point moved to `factor` times its distance from `base`.
    pub fn scaled(self, base: Point, factor: f64) -> Point {
        Point::new(
            base.x + (self.x - base.x) * factor,
            base.y + (self.y - base.y) * factor,
        )
    }

    /// The reflection of this point in the line through `a` and `b`. A
    /// zero-length line leaves the point where it is.
    pub fn mirrored(self, a: Point, b: Point) -> Point {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        if len_sq == 0.0 {
            return self;
        }
        let t = ((self.x - a.x) * dx + (self.y - a.y) * dy) / len_sq;
        let foot = Point::new(a.x + t * dx, a.y + t * dy);
        Point::new(2.0 * foot.x - self.x, 2.0 * foot.y - self.y)
    }
}

/// Whether `angle` is a whole number of quarter turns (within rounding).
fn is_quarter_turn(angle: f64) -> bool {
    let turns = angle / std::f64::consts::FRAC_PI_2;
    (turns - turns.round()).abs() < 1e-9
}

/// Whether the line through `a` and `b` runs along an axis.
fn is_axis_aligned(a: Point, b: Point) -> bool {
    let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
    let scale = dx.max(dy);
    scale > 0.0 && (dx / scale < 1e-9 || dy / scale < 1e-9)
}

impl Entity {
    /// Why this object cannot be mirrored, if it cannot. Mirror commands ask
    /// first and leave the drawing alone when any object refuses.
    pub fn mirror_refusal(&self) -> Option<&'static str> {
        match self {
            Entity::MText(m) => m.mirror_refusal(),
            Entity::MLeader(m) => m.mirror_refusal(),
            _ => None,
        }
    }

    /// Why this object cannot be scaled, if it cannot.
    pub fn scale_refusal(&self) -> Option<&'static str> {
        match self {
            Entity::MText(m) => m.scale_refusal(),
            Entity::MLeader(m) => m.scale_refusal(),
            _ => None,
        }
    }

    /// The entity shifted by `(dx, dy)`.
    pub fn translated(&self, dx: f64, dy: f64) -> Entity {
        match self {
            Entity::Dimension(d) => Entity::Dimension(Box::new(d.translated(dx, dy))),
            Entity::MText(m) => Entity::MText(Box::new(m.translated(dx, dy))),
            Entity::MLeader(m) => Entity::MLeader(Box::new(m.translated(dx, dy))),
            Entity::Leader(l) => Entity::Leader(Box::new(l.translated(dx, dy))),
            Entity::Insert(i) => Entity::Insert(Box::new(i.translated(dx, dy))),
            Entity::Point { at } => Entity::Point {
                at: at.translated(dx, dy),
            },
            Entity::Xline { .. } | Entity::Ray { .. } => {
                self.remapped_line(|o, d| (o.translated(dx, dy), d))
            }
            Entity::Hatch(h) => Entity::Hatch(Box::new(h.translated(dx, dy))),
            Entity::Ellipse(e) => Entity::Ellipse(e.translated(dx, dy)),
            Entity::Spline(s) => Entity::Spline(Box::new(s.translated(dx, dy))),
            Entity::Line { start, end } => Entity::Line {
                start: start.translated(dx, dy),
                end: end.translated(dx, dy),
            },
            Entity::Circle { center, radius } => Entity::Circle {
                center: center.translated(dx, dy),
                radius: *radius,
            },
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => Entity::Arc {
                center: center.translated(dx, dy),
                radius: *radius,
                start: *start,
                end: *end,
            },
            Entity::Rectangle { min, max } => Entity::Rectangle {
                min: min.translated(dx, dy),
                max: max.translated(dx, dy),
            },
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => Entity::Polyline {
                points: points.iter().map(|p| p.translated(dx, dy)).collect(),
                closed: *closed,
                bulges: bulges.clone(),
                widths: widths.clone(),
            },
        }
    }

    /// The entity turned `angle` radians counter-clockwise about `center`.
    /// A rectangle stays a rectangle for whole quarter turns and becomes a
    /// closed polyline for any other angle.
    pub fn rotated(&self, center: Point, angle: f64) -> Entity {
        match self {
            Entity::Dimension(d) => Entity::Dimension(Box::new(d.rotated(center, angle))),
            Entity::MText(m) => Entity::MText(Box::new(m.rotated(center, angle))),
            Entity::MLeader(m) => Entity::MLeader(Box::new(m.rotated(center, angle))),
            Entity::Leader(l) => Entity::Leader(Box::new(l.rotated(center, angle))),
            Entity::Insert(i) => Entity::Insert(Box::new(i.rotated(center, angle))),
            Entity::Point { at } => Entity::Point {
                at: at.rotated(center, angle),
            },
            Entity::Xline { .. } | Entity::Ray { .. } => self.remapped_line(|o, d| {
                (o.rotated(center, angle), d.rotated(Point::default(), angle))
            }),
            Entity::Hatch(h) => Entity::Hatch(Box::new(h.rotated(center, angle))),
            Entity::Ellipse(e) => Entity::Ellipse(e.rotated(center, angle)),
            Entity::Spline(s) => Entity::Spline(Box::new(s.rotated(center, angle))),
            Entity::Line { start, end } => Entity::Line {
                start: start.rotated(center, angle),
                end: end.rotated(center, angle),
            },
            Entity::Circle { center: c, radius } => Entity::Circle {
                center: c.rotated(center, angle),
                radius: *radius,
            },
            Entity::Arc {
                center: c,
                radius,
                start,
                end,
            } => Entity::Arc {
                center: c.rotated(center, angle),
                radius: *radius,
                start: arc::normalize(start + angle),
                end: arc::normalize(end + angle),
            },
            Entity::Rectangle { min, max } => {
                let corners = [
                    *min,
                    Point::new(max.x, min.y),
                    *max,
                    Point::new(min.x, max.y),
                ]
                .map(|p| p.rotated(center, angle));
                if is_quarter_turn(angle) {
                    let b = Bounds::from_points(corners[0], corners[2]);
                    Entity::Rectangle {
                        min: b.min,
                        max: b.max,
                    }
                } else {
                    Entity::Polyline {
                        points: corners.to_vec(),
                        closed: true,
                        bulges: Vec::new(),
                        widths: Vec::new(),
                    }
                }
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => Entity::Polyline {
                points: points.iter().map(|p| p.rotated(center, angle)).collect(),
                closed: *closed,
                bulges: bulges.clone(),
                widths: widths.clone(),
            },
        }
    }
}

impl Entity {
    /// The reflection of the entity in the line through `a` and `b`. A
    /// rectangle stays a rectangle when the line runs along an axis and
    /// becomes a closed polyline otherwise.
    pub fn mirrored(&self, a: Point, b: Point) -> Entity {
        match self {
            Entity::Dimension(d) => Entity::Dimension(Box::new(d.mirrored(a, b))),
            Entity::MLeader(m) => Entity::MLeader(Box::new(m.mirrored(a, b))),
            Entity::Leader(l) => Entity::Leader(Box::new(l.mirrored(a, b))),
            Entity::Insert(i) => Entity::Insert(Box::new(i.mirrored(a, b))),
            Entity::Point { at } => Entity::Point {
                at: at.mirrored(a, b),
            },
            Entity::Xline { .. } | Entity::Ray { .. } => self.remapped_line(|o, d| {
                // The direction is reflected in the mirror line's direction.
                let axis = crate::infinite::unit(Point::new(b.x - a.x, b.y - a.y));
                let d = axis.map_or(d, |axis| crate::infinite::reflected(d, axis));
                (o.mirrored(a, b), d)
            }),
            Entity::Hatch(h) => Entity::Hatch(Box::new(h.mirrored(a, b))),
            Entity::Ellipse(e) => Entity::Ellipse(e.mirrored(a, b)),
            Entity::Spline(s) => Entity::Spline(Box::new(s.mirrored(a, b))),
            // Callers check `mirror_refusal` first; if they do not, the text
            // is moved to its reflected place and never itself reversed.
            Entity::MText(m) => {
                let to = m.insertion.mirrored(a, b);
                Entity::MText(Box::new(
                    m.translated(to.x - m.insertion.x, to.y - m.insertion.y),
                ))
            }
            Entity::Line { start, end } => Entity::Line {
                start: start.mirrored(a, b),
                end: end.mirrored(a, b),
            },
            Entity::Circle { center, radius } => Entity::Circle {
                center: center.mirrored(a, b),
                radius: *radius,
            },
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                // Reflection reverses direction: the old end becomes the start.
                let c = center.mirrored(a, b);
                let from = arc::point_at(*center, *radius, *end).mirrored(a, b);
                let to = arc::point_at(*center, *radius, *start).mirrored(a, b);
                Entity::Arc {
                    center: c,
                    radius: *radius,
                    start: arc::angle_of(c, from),
                    end: arc::angle_of(c, to),
                }
            }
            Entity::Rectangle { min, max } => {
                let corners = [
                    *min,
                    Point::new(max.x, min.y),
                    *max,
                    Point::new(min.x, max.y),
                ]
                .map(|p| p.mirrored(a, b));
                if is_axis_aligned(a, b) {
                    let bounds = Bounds::from_points(corners[0], corners[2]);
                    Entity::Rectangle {
                        min: bounds.min,
                        max: bounds.max,
                    }
                } else {
                    Entity::Polyline {
                        points: corners.to_vec(),
                        closed: true,
                        bulges: Vec::new(),
                        widths: Vec::new(),
                    }
                }
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => Entity::Polyline {
                points: points.iter().map(|p| p.mirrored(a, b)).collect(),
                closed: *closed,
                // A reflection turns every arc the other way round.
                bulges: bulge::mirrored(bulges),
                widths: widths.clone(),
            },
        }
    }
}

impl Entity {
    /// The entity enlarged by `factor` about `base`. A positive factor keeps
    /// every kind as it is; the caller rejects zero and negative factors.
    pub fn scaled(&self, base: Point, factor: f64) -> Entity {
        match self {
            Entity::Dimension(d) => Entity::Dimension(Box::new(d.scaled(base, factor))),
            Entity::MText(m) => Entity::MText(Box::new(m.scaled(base, factor))),
            Entity::MLeader(m) => Entity::MLeader(Box::new(m.scaled(base, factor))),
            Entity::Leader(l) => Entity::Leader(Box::new(l.scaled(base, factor))),
            Entity::Insert(i) => Entity::Insert(Box::new(i.scaled(base, factor))),
            Entity::Point { at } => Entity::Point {
                at: at.scaled(base, factor),
            },
            Entity::Xline { .. } | Entity::Ray { .. } => {
                self.remapped_line(|o, d| (o.scaled(base, factor), d))
            }
            Entity::Hatch(h) => Entity::Hatch(Box::new(h.scaled(base, factor))),
            Entity::Ellipse(e) => Entity::Ellipse(e.scaled(base, factor)),
            Entity::Spline(s) => Entity::Spline(Box::new(s.scaled(base, factor))),
            Entity::Line { start, end } => Entity::Line {
                start: start.scaled(base, factor),
                end: end.scaled(base, factor),
            },
            Entity::Circle { center, radius } => Entity::Circle {
                center: center.scaled(base, factor),
                radius: radius * factor,
            },
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => Entity::Arc {
                center: center.scaled(base, factor),
                radius: radius * factor,
                start: *start,
                end: *end,
            },
            Entity::Rectangle { min, max } => {
                let bounds =
                    Bounds::from_points(min.scaled(base, factor), max.scaled(base, factor));
                Entity::Rectangle {
                    min: bounds.min,
                    max: bounds.max,
                }
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => Entity::Polyline {
                points: points.iter().map(|p| p.scaled(base, factor)).collect(),
                closed: *closed,
                bulges: bulges.clone(),
                widths: widths
                    .iter()
                    .map(|&(a, b)| (a * factor, b * factor))
                    .collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    fn quarter_arc() -> Entity {
        // From east round to north, radius 2 about (1, 0).
        Entity::Arc {
            center: Point::new(1.0, 0.0),
            radius: 2.0,
            start: 0.0,
            end: FRAC_PI_2,
        }
    }

    fn ends(e: &Entity) -> (Point, Point) {
        let Entity::Arc {
            center,
            radius,
            start,
            end,
        } = e
        else {
            panic!("not an arc")
        };
        (
            arc::point_at(*center, *radius, *start),
            arc::point_at(*center, *radius, *end),
        )
    }

    #[test]
    fn an_arc_moves_turns_and_scales_with_its_end_points() {
        let (s, e) = ends(&quarter_arc());
        let (s2, e2) = ends(&quarter_arc().translated(3.0, -1.0));
        assert!(near(s2, s.translated(3.0, -1.0)) && near(e2, e.translated(3.0, -1.0)));
        let pivot = Point::new(-1.0, 2.0);
        let (s3, e3) = ends(&quarter_arc().rotated(pivot, 1.0));
        assert!(near(s3, s.rotated(pivot, 1.0)) && near(e3, e.rotated(pivot, 1.0)));
        let (s4, e4) = ends(&quarter_arc().scaled(pivot, 2.5));
        assert!(near(s4, s.scaled(pivot, 2.5)) && near(e4, e.scaled(pivot, 2.5)));
    }

    #[test]
    fn mirroring_an_arc_swaps_its_ends_so_it_still_runs_counter_clockwise() {
        let (a, b) = (Point::new(-3.0, 1.0), Point::new(4.0, 5.0));
        let (s, e) = ends(&quarter_arc());
        let mirrored = quarter_arc().mirrored(a, b);
        let (ms, me) = ends(&mirrored);
        assert!(near(ms, e.mirrored(a, b)) && near(me, s.mirrored(a, b)));
        // The middle of the arc stays the middle.
        let Entity::Arc {
            center,
            radius,
            start,
            end,
        } = mirrored
        else {
            panic!()
        };
        let mid = arc::midpoint(center, radius, start, end);
        let before = arc::midpoint(Point::new(1.0, 0.0), 2.0, 0.0, FRAC_PI_2);
        assert!(near(mid, before.mirrored(a, b)));
    }

    #[test]
    fn a_point_object_moves_turns_mirrors_and_scales_with_its_position() {
        let point = Entity::Point {
            at: Point::new(2.0, 0.0),
        };
        let at = |e: Entity| match e {
            Entity::Point { at } => at,
            other => panic!("{other:?}"),
        };
        assert!(near(at(point.translated(1.0, 2.0)), Point::new(3.0, 2.0)));
        let turned = point.rotated(Point::new(1.0, 0.0), FRAC_PI_2);
        assert!(near(at(turned), Point::new(1.0, 1.0)));
        let mirrored = point.mirrored(Point::new(0.0, 0.0), Point::new(0.0, 1.0));
        assert!(near(at(mirrored), Point::new(-2.0, 0.0)));
        assert!(near(
            at(point.scaled(Point::new(0.0, 0.0), 3.0)),
            Point::new(6.0, 0.0)
        ));
        assert_eq!(point.grips().len(), 1);
        assert_eq!(point.explode(), None);
        assert_eq!(point.bounds().min, point.bounds().max);
    }

    #[test]
    fn rotating_a_point_a_quarter_turn() {
        let p = Point::new(2.0, 0.0).rotated(Point::new(1.0, 0.0), FRAC_PI_2);
        assert!(near(p, Point::new(1.0, 1.0)));
    }

    #[test]
    fn translate_moves_every_kind() {
        let line = Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 1.0),
        };
        assert_eq!(
            line.translated(2.0, 3.0),
            Entity::Line {
                start: Point::new(2.0, 3.0),
                end: Point::new(3.0, 4.0)
            }
        );
        let circle = Entity::Circle {
            center: Point::new(1.0, 1.0),
            radius: 5.0,
        };
        assert_eq!(
            circle.translated(-1.0, 0.0),
            Entity::Circle {
                center: Point::new(0.0, 1.0),
                radius: 5.0
            }
        );
        let poly = Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(poly.translated(1.0, 1.0).bounds().min, Point::new(1.0, 1.0));
    }

    #[test]
    fn a_polyline_arc_moves_turns_and_scales_with_its_vertices_and_mirrors_the_other_way() {
        // A half circle from (0,0) to (2,0) bulging below: bulge 1.
        let poly = Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
            closed: false,
            bulges: vec![1.0, 0.0],
            widths: Vec::new(),
        };
        let kept = |e: Entity| match e {
            Entity::Polyline { bulges, .. } => bulges,
            other => panic!("{other:?}"),
        };
        assert_eq!(kept(poly.translated(3.0, 3.0)), vec![1.0, 0.0]);
        assert_eq!(
            kept(poly.rotated(Point::new(1.0, 1.0), 1.0)),
            vec![1.0, 0.0]
        );
        assert_eq!(kept(poly.scaled(Point::new(0.0, 0.0), 2.0)), vec![1.0, 0.0]);
        // Mirrored in the x axis the arc bulges above instead: the bulge flips.
        let flipped = poly.mirrored(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
        assert_eq!(kept(flipped.clone()), vec![-1.0, -0.0]);
        // The curve really is the mirror image: the lowest point is now the highest.
        let (before, after) = (poly.bounds(), flipped.bounds());
        assert!((before.min.y + 1.0).abs() < 1e-9 && (after.max.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn circle_rotation_moves_the_center_only() {
        let c = Entity::Circle {
            center: Point::new(2.0, 0.0),
            radius: 3.0,
        };
        let Entity::Circle { center, radius } = c.rotated(Point::new(0.0, 0.0), PI) else {
            panic!("still a circle");
        };
        assert!(near(center, Point::new(-2.0, 0.0)));
        assert_eq!(radius, 3.0);
    }

    #[test]
    fn rectangle_stays_a_rectangle_for_quarter_turns() {
        let r = Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(4.0, 2.0),
        };
        let Entity::Rectangle { min, max } = r.rotated(Point::new(0.0, 0.0), FRAC_PI_2) else {
            panic!("still a rectangle");
        };
        assert!(near(min, Point::new(-2.0, 0.0)));
        assert!(near(max, Point::new(0.0, 4.0)));
    }

    #[test]
    fn rectangle_becomes_a_closed_polyline_at_other_angles() {
        let r = Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(2.0, 2.0),
        };
        let Entity::Polyline { points, closed, .. } = r.rotated(Point::new(1.0, 1.0), PI / 4.0)
        else {
            panic!("expected a polyline");
        };
        assert!(closed);
        assert_eq!(points.len(), 4);
        // A square turned 45 degrees about its center has its corners on
        // the axes through the center.
        assert!(near(points[0], Point::new(1.0, 1.0 - 2f64.sqrt())));
    }

    #[test]
    fn rotating_back_restores_a_line() {
        let line = Entity::Line {
            start: Point::new(1.0, 2.0),
            end: Point::new(5.0, 3.0),
        };
        let c = Point::new(0.5, -1.0);
        let Entity::Line { start, end } = line.rotated(c, 0.7).rotated(c, -0.7) else {
            panic!("still a line");
        };
        assert!(near(start, Point::new(1.0, 2.0)));
        assert!(near(end, Point::new(5.0, 3.0)));
    }

    #[test]
    fn mirroring_a_point_in_the_y_axis() {
        let p = Point::new(3.0, 2.0).mirrored(Point::new(0.0, -5.0), Point::new(0.0, 5.0));
        assert!(near(p, Point::new(-3.0, 2.0)));
    }

    #[test]
    fn mirroring_in_a_diagonal_swaps_the_coordinates() {
        let p = Point::new(3.0, 1.0).mirrored(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        assert!(near(p, Point::new(1.0, 3.0)));
    }

    #[test]
    fn mirroring_twice_restores_the_entity() {
        let line = Entity::Line {
            start: Point::new(1.0, 2.0),
            end: Point::new(5.0, 3.0),
        };
        let (a, b) = (Point::new(0.0, 1.0), Point::new(4.0, 7.0));
        let Entity::Line { start, end } = line.mirrored(a, b).mirrored(a, b) else {
            panic!("still a line");
        };
        assert!(near(start, Point::new(1.0, 2.0)) && near(end, Point::new(5.0, 3.0)));
    }

    #[test]
    fn a_degenerate_mirror_line_changes_nothing() {
        let p = Point::new(1.0, 2.0);
        assert_eq!(p.mirrored(Point::new(3.0, 3.0), Point::new(3.0, 3.0)), p);
    }

    #[test]
    fn rectangles_keep_their_shape_only_for_axis_mirror_lines() {
        let r = Entity::Rectangle {
            min: Point::new(1.0, 1.0),
            max: Point::new(3.0, 2.0),
        };
        let Entity::Rectangle { min, max } = r.mirrored(Point::new(0.0, 0.0), Point::new(0.0, 1.0))
        else {
            panic!("expected a rectangle");
        };
        assert!(near(min, Point::new(-3.0, 1.0)) && near(max, Point::new(-1.0, 2.0)));
        let tilted = r.mirrored(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        assert!(matches!(tilted, Entity::Polyline { closed: true, .. }));
    }

    #[test]
    fn scaling_moves_points_away_from_the_base() {
        let p = Point::new(3.0, 2.0).scaled(Point::new(1.0, 1.0), 2.0);
        assert!(near(p, Point::new(5.0, 3.0)));
    }

    #[test]
    fn scaling_a_circle_scales_center_and_radius() {
        let c = Entity::Circle {
            center: Point::new(2.0, 0.0),
            radius: 3.0,
        };
        let Entity::Circle { center, radius } = c.scaled(Point::new(0.0, 0.0), 0.5) else {
            panic!("still a circle");
        };
        assert!(near(center, Point::new(1.0, 0.0)));
        assert_eq!(radius, 1.5);
    }

    #[test]
    fn scaling_keeps_a_rectangle_a_rectangle() {
        let r = Entity::Rectangle {
            min: Point::new(1.0, 1.0),
            max: Point::new(3.0, 2.0),
        };
        assert_eq!(
            r.scaled(Point::new(0.0, 0.0), 2.0),
            Entity::Rectangle {
                min: Point::new(2.0, 2.0),
                max: Point::new(6.0, 4.0)
            }
        );
    }
}
