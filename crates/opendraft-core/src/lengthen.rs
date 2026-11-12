//! Lengthening or shortening an entity by an amount, the base for LENGTHEN.
//! (Extending to a boundary is `extend`.) A line keeps its direction, an arc
//! its circle, and an open polyline changes its first or last segment the same
//! way. Ellipses and splines are not handled yet.

use std::f64::consts::TAU;

use crate::extend::End;
use crate::{arc, bulge, Entity, Point};

/// Shortest length or angle left when shortening: anything less is gone.
const MIN: f64 = 1e-9;

/// How much to grow an end by.
#[derive(Clone, Copy)]
enum Growth {
    Length(f64),
    /// Radians, for an arc.
    Angle(f64),
}

impl Entity {
    /// The length of a line, an arc or an open polyline; `None` for any other
    /// object.
    pub fn curve_length(&self) -> Option<f64> {
        match self {
            Entity::Line { start, end } => Some((end.x - start.x).hypot(end.y - start.y)),
            Entity::Arc {
                radius, start, end, ..
            } => Some(radius.abs() * arc::sweep(*start, *end)),
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                ..
            } if points.len() >= 2 => Some(bulge::path_length(points, bulges, false)),
            _ => None,
        }
    }

    /// The angle an arc turns through, in radians.
    pub fn included_angle(&self) -> Option<f64> {
        match self {
            Entity::Arc { start, end, .. } => Some(arc::sweep(*start, *end)),
            _ => None,
        }
    }

    /// The end of a line, arc or open polyline nearer `at`.
    pub fn nearer_end(&self, at: Point) -> Option<End> {
        let (first, last) = match self {
            Entity::Line { start, end } => (*start, *end),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => (
                arc::point_at(*center, *radius, *start),
                arc::point_at(*center, *radius, *end),
            ),
            Entity::Polyline {
                points,
                closed: false,
                ..
            } if points.len() >= 2 => (points[0], points[points.len() - 1]),
            _ => return None,
        };
        let d = |p: Point| (p.x - at.x).hypot(p.y - at.y);
        Some(if d(first) <= d(last) {
            End::Start
        } else {
            End::End
        })
    }

    /// Which end of the object grip `index` (see [`Entity::grips`]) is, for
    /// the grips that sit on an end of a line, an arc or an open polyline.
    pub fn end_grip(&self, index: usize) -> Option<End> {
        let last = match self {
            Entity::Line { .. } | Entity::Arc { .. } => 1,
            Entity::Polyline {
                points,
                closed: false,
                ..
            } if points.len() >= 2 => points.len() - 1,
            _ => return None,
        };
        match index {
            0 => Some(End::Start),
            i if i == last => Some(End::End),
            _ => None,
        }
    }

    /// With `end` moved along the object by `delta` (negative shortens). `None`
    /// for a kind that cannot be lengthened, or when too little would be left.
    pub fn lengthened(&self, end: End, delta: f64) -> Option<Entity> {
        self.grown(end, Growth::Length(delta))
    }

    /// An arc with `end` turned on by `delta` radians (negative shortens).
    pub fn lengthened_by_angle(&self, end: End, delta: f64) -> Option<Entity> {
        self.grown(end, Growth::Angle(delta))
    }

    /// With `end` moved to where the cursor `p` is along the object: its
    /// projection on the line, or its direction from the centre of the arc.
    pub fn lengthened_to(&self, end: End, p: Point) -> Option<Entity> {
        match self {
            Entity::Line { start, end: stop } => {
                let (base, tip) = match end {
                    End::Start => (*stop, *start),
                    End::End => (*start, *stop),
                };
                let len = (tip.x - base.x).hypot(tip.y - base.y);
                if len <= MIN {
                    return None;
                }
                let along =
                    ((p.x - base.x) * (tip.x - base.x) + (p.y - base.y) * (tip.y - base.y)) / len;
                self.lengthened(end, along - len)
            }
            Entity::Arc {
                center,
                start,
                end: stop,
                ..
            } => {
                let at = arc::angle_of(*center, p);
                let new_sweep = match end {
                    End::End => (at - start).rem_euclid(TAU),
                    End::Start => (stop - at).rem_euclid(TAU),
                };
                self.lengthened_by_angle(end, new_sweep - arc::sweep(*start, *stop))
            }
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                widths,
            } if points.len() >= 2 => end_segment(points, bulges, widths, end, |segment, moves| {
                segment.lengthened_to(moves, p)
            }),
            _ => None,
        }
    }

    fn grown(&self, end: End, growth: Growth) -> Option<Entity> {
        match self {
            Entity::Line { start, end: stop } => {
                let Growth::Length(delta) = growth else {
                    return None;
                };
                let len = (stop.x - start.x).hypot(stop.y - start.y);
                let new = len + delta;
                if len <= MIN || new.is_nan() || new <= MIN {
                    return None;
                }
                let (ux, uy) = ((stop.x - start.x) / len, (stop.y - start.y) / len);
                Some(match end {
                    End::End => Entity::Line {
                        start: *start,
                        end: Point::new(start.x + ux * new, start.y + uy * new),
                    },
                    End::Start => Entity::Line {
                        start: Point::new(stop.x - ux * new, stop.y - uy * new),
                        end: *stop,
                    },
                })
            }
            Entity::Arc {
                center,
                radius,
                start,
                end: stop,
            } => {
                let turn = match growth {
                    Growth::Length(delta) => delta / radius.abs(),
                    Growth::Angle(angle) => angle,
                };
                let new = arc::sweep(*start, *stop) + turn;
                if radius.abs() <= MIN || new.is_nan() || new <= MIN || new >= TAU - MIN {
                    return None;
                }
                let (start, end) = match end {
                    End::End => (*start, arc::normalize(stop + turn)),
                    End::Start => (arc::normalize(start - turn), *stop),
                };
                Some(Entity::Arc {
                    center: *center,
                    radius: *radius,
                    start,
                    end,
                })
            }
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                widths,
            } if points.len() >= 2 => end_segment(points, bulges, widths, end, |segment, moves| {
                segment.grown(moves, growth)
            }),
            _ => None,
        }
    }
}

/// An open polyline with its first or last segment changed by `change`, which
/// is given that segment as a line or an arc and the end of it that moves.
fn end_segment(
    points: &[Point],
    bulges: &[f64],
    widths: &[(f64, f64)],
    end: End,
    change: impl Fn(&Entity, End) -> Option<Entity>,
) -> Option<Entity> {
    let n = points.len();
    let (segment, tip_is_end) = match end {
        End::Start => (0, false),
        End::End => (n - 2, true),
    };
    let (a, b) = (points[segment], points[segment + 1]);
    let curve = bulge::at(bulges, segment);
    let (new_tip, new_curve) = match bulge::arc_of(a, b, curve) {
        None => {
            let line = Entity::Line { start: a, end: b };
            let moves = if tip_is_end { End::End } else { End::Start };
            match change(&line, moves)? {
                Entity::Line { start, end } => (if tip_is_end { end } else { start }, 0.0),
                _ => return None,
            }
        }
        Some(s) => {
            let whole = Entity::Arc {
                center: s.center,
                radius: s.radius,
                start: s.start,
                end: s.end,
            };
            let moves = if tip_is_end == s.forward {
                End::End
            } else {
                End::Start
            };
            match change(&whole, moves)? {
                Entity::Arc {
                    center,
                    radius,
                    start,
                    end,
                } => {
                    let angle = if moves == End::End { end } else { start };
                    (
                        arc::point_at(center, radius, angle),
                        bulge::from_sweep(arc::sweep(start, end), s.forward),
                    )
                }
                _ => return None,
            }
        }
    };
    let mut points = points.to_vec();
    let mut curves = bulges.to_vec();
    points[if tip_is_end { n - 1 } else { 0 }] = new_tip;
    if !curves.is_empty() || new_curve != 0.0 {
        curves.resize(n, 0.0);
        curves[segment] = new_curve;
    }
    Some(Entity::Polyline {
        points,
        closed: false,
        bulges: curves,
        widths: widths.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn quarter() -> Entity {
        Entity::Arc {
            center: p(0.0, 0.0),
            radius: 2.0,
            start: 0.0,
            end: FRAC_PI_2,
        }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_line_grows_or_shrinks_at_the_chosen_end() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            l.lengthened(End::End, 5.0),
            Some(line(p(0.0, 0.0), p(15.0, 0.0)))
        );
        assert_eq!(
            l.lengthened(End::Start, 5.0),
            Some(line(p(-5.0, 0.0), p(10.0, 0.0)))
        );
        assert_eq!(
            l.lengthened(End::End, -4.0),
            Some(line(p(0.0, 0.0), p(6.0, 0.0)))
        );
        assert_eq!(l.lengthened(End::End, -10.0), None, "nothing would be left");
        assert_eq!(l.lengthened(End::End, -11.0), None);
        assert_eq!(
            l.lengthened_by_angle(End::End, 1.0),
            None,
            "angles are for arcs"
        );
    }

    #[test]
    fn an_arc_grows_round_its_circle_by_length_or_angle() {
        let a = quarter();
        let Some(Entity::Arc {
            start, end, radius, ..
        }) = a.lengthened(End::End, PI / 2.0 * 2.0 / 2.0)
        else {
            panic!("an arc")
        };
        assert!(near(radius, 2.0) && near(start, 0.0));
        // A length of pi / 2 on radius 2 is an eighth of a turn more.
        assert!(near(end, FRAC_PI_2 + FRAC_PI_2 / 2.0), "{end}");
        let Some(Entity::Arc { start, end, .. }) = a.lengthened_by_angle(End::Start, 0.5) else {
            panic!("an arc")
        };
        assert!(near(start, -0.5 + TAU) && near(end, FRAC_PI_2));
        assert!(
            a.lengthened_by_angle(End::End, TAU).is_none(),
            "past a full turn"
        );
        assert!(a.lengthened_by_angle(End::End, -FRAC_PI_2).is_none());
        assert!(near(a.curve_length().unwrap(), PI));
        assert!(near(a.included_angle().unwrap(), FRAC_PI_2));
    }

    #[test]
    fn a_polyline_changes_its_end_segment_straight_or_curved() {
        let pl = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let Some(Entity::Polyline { points, .. }) = pl.lengthened(End::End, 2.0) else {
            panic!("a polyline")
        };
        assert_eq!(points, [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)]);
        let Some(Entity::Polyline { points, .. }) = pl.lengthened(End::Start, -1.0) else {
            panic!("a polyline")
        };
        assert_eq!(points[0], p(1.0, 0.0));
        assert!(near(pl.curve_length().unwrap(), 7.0));

        // A half circle of radius 1 from (1,0) to (-1,0); lengthen by a
        // quarter of the circle's length at the end.
        let half = Entity::Polyline {
            points: vec![p(1.0, 0.0), p(-1.0, 0.0)],
            closed: false,
            bulges: vec![1.0, 0.0],
            widths: Vec::new(),
        };
        let Some(Entity::Polyline { points, bulges, .. }) = half.lengthened(End::End, FRAC_PI_2)
        else {
            panic!("a polyline")
        };
        assert!(
            near(points[1].x, 0.0) && near(points[1].y, -1.0),
            "{points:?}"
        );
        assert!(near(bulges[0], (3.0 * FRAC_PI_2 / 4.0).tan()), "{bulges:?}");
        assert!(near(
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                widths: Vec::new()
            }
            .curve_length()
            .unwrap(),
            PI + FRAC_PI_2
        ));
    }

    #[test]
    fn dragging_an_end_puts_it_at_the_cursor() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            l.lengthened_to(End::End, p(14.0, 7.0)),
            Some(line(p(0.0, 0.0), p(14.0, 0.0)))
        );
        assert_eq!(
            l.lengthened_to(End::Start, p(-3.0, 1.0)),
            Some(line(p(-3.0, 0.0), p(10.0, 0.0)))
        );
        let Some(Entity::Arc { end, .. }) = quarter().lengthened_to(End::End, p(-1.0, 1.0)) else {
            panic!("an arc")
        };
        assert!(near(end, 0.75 * PI), "{end}");
    }

    #[test]
    fn end_grips_are_the_ones_on_the_ends() {
        let l = line(p(0.0, 0.0), p(1.0, 0.0));
        assert_eq!(l.end_grip(0), Some(End::Start));
        assert_eq!(l.end_grip(1), Some(End::End));
        assert_eq!(l.end_grip(2), None, "the middle");
        assert_eq!(quarter().end_grip(1), Some(End::End));
        assert_eq!(quarter().end_grip(3), None, "the centre");
        let pl = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(pl.end_grip(0), Some(End::Start));
        assert_eq!(pl.end_grip(1), None, "a bend");
        assert_eq!(pl.end_grip(2), Some(End::End));
        assert_eq!(pl.end_grip(3), None, "a segment middle");
        let closed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(closed.end_grip(0), None);
    }

    #[test]
    fn the_nearer_end_is_found_and_other_kinds_are_refused() {
        let l = line(p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(l.nearer_end(p(2.0, 5.0)), Some(End::Start));
        assert_eq!(l.nearer_end(p(8.0, -5.0)), Some(End::End));
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(circle.nearer_end(p(0.0, 0.0)), None);
        assert_eq!(circle.lengthened(End::End, 1.0), None);
        assert_eq!(circle.curve_length(), None);
    }
}
