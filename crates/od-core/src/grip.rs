//! Grips: the handles an object shows when selected, and what dragging one
//! does to it. Pure geometry over [`Entity`]; the canvas draws and picks
//! them and the Stretch edit applies them.
//!
//! Each kind of object offers grips at its defining points: the ends of a
//! line and the middle to move the whole line, a circle's centre to move it
//! and its quadrants to resize it, and so on. A grip is named by its index,
//! which stays the same while the object is edited so a drag can refer to it.

use std::f64::consts::FRAC_PI_2;

use crate::intersect::corners;
use crate::{arc, bulge, Bounds, Entity, Point};

const EPS: f64 = 1e-9;

/// What dragging a grip does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GripKind {
    /// Moves one defining point; the rest stays put.
    Stretch,
    /// Moves the whole object, or a whole segment of it.
    Move,
    /// Changes the size (a radius).
    Resize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    pub at: Point,
    pub kind: GripKind,
}

fn grip(at: Point, kind: GripKind) -> Grip {
    Grip { at, kind }
}

fn mid(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

impl Entity {
    /// The grips the object shows, in a fixed order per kind:
    ///
    /// - line: start, end, middle;
    /// - circle: centre, then the east, north, west and south points;
    /// - arc: start, end, middle (the arc then passes through where it is
    ///   dragged), centre;
    /// - rectangle: the four corners from the minimum corner round, then the
    ///   four edge middles (bottom, right, top, left);
    /// - polyline: the vertices, then the middle of each segment;
    /// - dimension: its defining points, then the text;
    /// - ellipse: centre, the major then the minor axis ends, then on an arc
    ///   its start and end (see [`crate::Ellipse::grips`]);
    /// - spline: its fit points, or its control points (see
    ///   [`crate::Spline::grips`]);
    /// - block reference: its insertion point, which moves it;
    /// - hatch: one grip at the middle of its extent, which moves it;
    /// - construction line or ray: its base point, then a point on the line
    ///   (two for a construction line) that turns it ([`crate::infinite`]).
    pub fn grips(&self) -> Vec<Grip> {
        use GripKind::{Move, Resize, Stretch};
        match self {
            Entity::Insert(i) => vec![grip(i.insertion, Move)],
            Entity::AttributeDefinition(d) => vec![grip(d.text.position, Move)],
            Entity::Point { at } => vec![grip(*at, Move)],
            Entity::Xline { .. } | Entity::Ray { .. } => self.line_grips(),
            Entity::Hatch(_) => {
                let b = self.bounds();
                vec![grip(mid(b.min, b.max), Move)]
            }
            Entity::Ellipse(e) => e.grips(),
            Entity::Spline(s) => s.grips(),
            Entity::Dimension(d) => d.grips(),
            Entity::MText(m) => m.grips(),
            Entity::MLeader(m) => m.grips(),
            Entity::Leader(l) => l.grips(),
            Entity::Line { start, end } => vec![
                grip(*start, Stretch),
                grip(*end, Stretch),
                grip(mid(*start, *end), Move),
            ],
            Entity::Circle { center, radius } => {
                let r = radius.abs();
                let mut grips = vec![grip(*center, Move)];
                for quarter in 0..4 {
                    let angle = f64::from(quarter) * FRAC_PI_2;
                    grips.push(grip(arc::point_at(*center, r, angle), Resize));
                }
                grips
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => vec![
                grip(arc::point_at(*center, *radius, *start), Stretch),
                grip(arc::point_at(*center, *radius, *end), Stretch),
                grip(arc::midpoint(*center, *radius, *start, *end), Resize),
                grip(*center, Move),
            ],
            Entity::Rectangle { min, max } => {
                let c = corners(*min, *max);
                let mut grips: Vec<Grip> = c.iter().map(|p| grip(*p, Stretch)).collect();
                grips.extend((0..4).map(|i| grip(mid(c[i], c[(i + 1) % 4]), Stretch)));
                grips
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            } => {
                let mut grips: Vec<Grip> = points.iter().map(|p| grip(*p, Stretch)).collect();
                grips.extend(segments(points, *closed).enumerate().map(|(i, (a, b))| {
                    let bulge = bulge::at(bulges, i);
                    // The middle of a straight segment moves it; the middle of an
                    // arc reshapes it.
                    let kind = if bulge.abs() > bulge::STRAIGHT {
                        Resize
                    } else {
                        Move
                    };
                    grip(bulge::midpoint(points[a], points[b], bulge), kind)
                }));
                grips
            }
        }
    }

    /// The object with grip `index` dragged to `to`, or `None` when that
    /// leaves nothing sensible (a circle of no radius, an arc through three
    /// points in a line) or there is no such grip.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<Entity> {
        let grips = self.grips();
        let from = grips.get(index)?.at;
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        if !(dx.is_finite() && dy.is_finite()) {
            return None;
        }
        match self {
            Entity::Dimension(d) => d
                .with_grip_moved(index, to)
                .map(|d| Entity::Dimension(Box::new(d))),
            Entity::MText(m) => m
                .with_grip_moved(index, to)
                .map(|m| Entity::MText(Box::new(m))),
            Entity::MLeader(m) => m
                .with_grip_moved(index, to)
                .map(|m| Entity::MLeader(Box::new(m))),
            Entity::Leader(l) => l
                .with_grip_moved(index, to)
                .map(|l| Entity::Leader(Box::new(l))),
            Entity::Xline { .. } | Entity::Ray { .. } => self.line_grip_moved(index, to),
            Entity::Insert(_)
            | Entity::AttributeDefinition(_)
            | Entity::Hatch(_)
            | Entity::Point { .. } => Some(self.translated(dx, dy)),
            Entity::Ellipse(e) => e.with_grip_moved(index, to).map(Entity::Ellipse),
            Entity::Spline(s) => s
                .with_grip_moved(index, to)
                .map(|s| Entity::Spline(Box::new(s))),
            Entity::Line { start, end } => Some(match index {
                0 => Entity::Line {
                    start: to,
                    end: *end,
                },
                1 => Entity::Line {
                    start: *start,
                    end: to,
                },
                _ => self.translated(dx, dy),
            }),
            Entity::Circle { center, .. } => {
                if index == 0 {
                    return Some(self.translated(dx, dy));
                }
                let radius = (to.x - center.x).hypot(to.y - center.y);
                (radius > EPS).then_some(Entity::Circle {
                    center: *center,
                    radius,
                })
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                if index == 3 {
                    return Some(self.translated(dx, dy));
                }
                let s = arc::point_at(*center, *radius, *start);
                let e = arc::point_at(*center, *radius, *end);
                let m = arc::midpoint(*center, *radius, *start, *end);
                // Drag one of the three defining points: the arc is the one
                // through the three as they now are.
                let (a, b, c) = match index {
                    0 => (to, m, e),
                    1 => (s, m, to),
                    _ => (s, to, e),
                };
                let (center, radius, start, end) = arc::through_points(a, b, c)?;
                Some(Entity::Arc {
                    center,
                    radius,
                    start,
                    end,
                })
            }
            Entity::Rectangle { min, max } => {
                let b = Bounds::from_points(*min, *max);
                let (lo, hi) = (b.min, b.max);
                let (new_min, new_max) = match index {
                    // A corner moves; the opposite one stays.
                    0 => ((to.x, to.y), (hi.x, hi.y)),
                    1 => ((lo.x, to.y), (to.x, hi.y)),
                    2 => ((lo.x, lo.y), (to.x, to.y)),
                    3 => ((to.x, lo.y), (hi.x, to.y)),
                    // An edge moves sideways, by the amount across it.
                    4 => ((lo.x, to.y), (hi.x, hi.y)),
                    5 => ((lo.x, lo.y), (to.x, hi.y)),
                    6 => ((lo.x, lo.y), (hi.x, to.y)),
                    _ => ((to.x, lo.y), (hi.x, hi.y)),
                };
                let made = Bounds::from_points(
                    Point::new(new_min.0, new_min.1),
                    Point::new(new_max.0, new_max.1),
                );
                // A rectangle dragged flat is not a rectangle.
                (made.max.x - made.min.x > EPS && made.max.y - made.min.y > EPS).then_some(
                    Entity::Rectangle {
                        min: made.min,
                        max: made.max,
                    },
                )
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => {
                let n = points.len();
                let mut moved = points.clone();
                let mut curves = bulges.clone();
                if index < n {
                    moved[index] = to;
                } else {
                    let segment = index - n;
                    let (a, b) = segments(points, *closed).nth(segment)?;
                    if bulge::at(bulges, segment).abs() > bulge::STRAIGHT {
                        // An arc's middle: the arc passes through where it is dragged.
                        curves.resize(n, 0.0);
                        curves[segment] = bulge::through(points[a], to, points[b])?;
                    } else {
                        // A straight segment's middle moves both its ends.
                        moved[a] = moved[a].translated(dx, dy);
                        moved[b] = moved[b].translated(dx, dy);
                    }
                }
                Some(Entity::Polyline {
                    points: moved,
                    closed: *closed,
                    bulges: curves,
                    widths: widths.clone(),
                })
            }
        }
    }
}

/// The vertex index pairs of a polyline's segments, including the closing
/// one when closed.
fn segments(points: &[Point], closed: bool) -> impl Iterator<Item = (usize, usize)> {
    let n = points.len();
    let count = if n < 2 {
        0
    } else if closed && n > 2 {
        n
    } else {
        n - 1
    };
    (0..count).map(move |i| (i, (i + 1) % n))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    fn line() -> Entity {
        Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        }
    }

    #[test]
    fn a_line_has_two_ends_and_a_middle() {
        let grips = line().grips();
        assert_eq!(grips.len(), 3);
        assert!(near(grips[0].at, p(0.0, 0.0)) && grips[0].kind == GripKind::Stretch);
        assert!(near(grips[2].at, p(5.0, 0.0)) && grips[2].kind == GripKind::Move);
        // An end follows the pointer; the other end stays.
        assert_eq!(
            line().with_grip_moved(1, p(7.0, 3.0)),
            Some(Entity::Line {
                start: p(0.0, 0.0),
                end: p(7.0, 3.0)
            })
        );
        // The middle carries the whole line: the middle lands where dragged.
        assert_eq!(
            line().with_grip_moved(2, p(5.0, 4.0)),
            Some(Entity::Line {
                start: p(0.0, 4.0),
                end: p(10.0, 4.0)
            })
        );
        assert_eq!(line().with_grip_moved(3, p(0.0, 0.0)), None, "no such grip");
    }

    #[test]
    fn a_circle_moves_by_its_centre_and_resizes_by_its_quadrants() {
        let c = Entity::Circle {
            center: p(2.0, 3.0),
            radius: 4.0,
        };
        let grips = c.grips();
        assert_eq!(grips.len(), 5);
        assert!(near(grips[1].at, p(6.0, 3.0)) && near(grips[2].at, p(2.0, 7.0)));
        assert!(near(grips[3].at, p(-2.0, 3.0)) && near(grips[4].at, p(2.0, -1.0)));
        assert!(grips[1..].iter().all(|g| g.kind == GripKind::Resize));
        assert_eq!(
            c.with_grip_moved(0, p(5.0, 5.0)),
            Some(Entity::Circle {
                center: p(5.0, 5.0),
                radius: 4.0
            })
        );
        // Any quadrant sets the radius to the distance from the centre.
        assert_eq!(
            c.with_grip_moved(3, p(2.0, 13.0)),
            Some(Entity::Circle {
                center: p(2.0, 3.0),
                radius: 10.0
            })
        );
        assert_eq!(c.with_grip_moved(1, p(2.0, 3.0)), None, "no radius left");
    }

    #[test]
    fn an_arc_passes_through_the_dragged_defining_point() {
        // The upper half of the unit circle.
        let a = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 1.0,
            start: 0.0,
            end: PI,
        };
        let grips = a.grips();
        assert_eq!(grips.len(), 4);
        assert!(near(grips[0].at, p(1.0, 0.0)) && near(grips[1].at, p(-1.0, 0.0)));
        assert!(near(grips[2].at, p(0.0, 1.0)) && near(grips[3].at, p(0.0, 0.0)));
        // Dragging the middle up makes a taller arc through the same ends.
        let taller = a.with_grip_moved(2, p(0.0, 3.0)).unwrap();
        assert!(taller.distance_to(p(1.0, 0.0)) < 1e-9);
        assert!(taller.distance_to(p(-1.0, 0.0)) < 1e-9);
        assert!(taller.distance_to(p(0.0, 3.0)) < 1e-9);
        // Dragging an end: the arc passes through the new end and the old middle.
        let moved = a.with_grip_moved(0, p(0.0, -1.0)).unwrap();
        assert!(moved.distance_to(p(0.0, -1.0)) < 1e-9);
        assert!(moved.distance_to(p(0.0, 1.0)) < 1e-9);
        assert!(moved.distance_to(p(-1.0, 0.0)) < 1e-9);
        // The centre moves everything.
        let shifted = a.with_grip_moved(3, p(5.0, 5.0)).unwrap();
        assert!(shifted.distance_to(p(5.0, 6.0)) < 1e-9);
        // Dragging the middle onto the ends' line leaves no arc.
        assert_eq!(a.with_grip_moved(2, p(0.0, 0.0)), None);
    }

    #[test]
    fn a_rectangle_corner_keeps_the_opposite_corner_and_an_edge_moves_sideways() {
        let r = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(10.0, 6.0),
        };
        let grips = r.grips();
        assert_eq!(grips.len(), 8);
        assert!(near(grips[2].at, p(10.0, 6.0)) && near(grips[4].at, p(5.0, 0.0)));
        assert_eq!(
            r.with_grip_moved(2, p(14.0, 9.0)),
            Some(Entity::Rectangle {
                min: p(0.0, 0.0),
                max: p(14.0, 9.0)
            })
        );
        // Dragged past the opposite corner it turns the other way round.
        assert_eq!(
            r.with_grip_moved(0, p(12.0, 8.0)),
            Some(Entity::Rectangle {
                min: p(10.0, 6.0),
                max: p(12.0, 8.0)
            })
        );
        // The bottom edge moves up or down only; the sideways part is ignored.
        assert_eq!(
            r.with_grip_moved(4, p(99.0, -2.0)),
            Some(Entity::Rectangle {
                min: p(0.0, -2.0),
                max: p(10.0, 6.0)
            })
        );
        assert_eq!(
            r.with_grip_moved(5, p(13.0, 99.0)),
            Some(Entity::Rectangle {
                min: p(0.0, 0.0),
                max: p(13.0, 6.0)
            })
        );
        assert_eq!(r.with_grip_moved(6, p(0.0, 0.0)), None, "dragged flat");
    }

    #[test]
    fn a_polyline_has_a_grip_for_each_vertex_and_segment() {
        let open = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let grips = open.grips();
        assert_eq!(grips.len(), 3 + 2);
        assert!(near(grips[3].at, p(5.0, 0.0)) && near(grips[4].at, p(10.0, 5.0)));
        let closed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert_eq!(
            closed.grips().len(),
            3 + 3,
            "the closing segment has one too"
        );
        // A vertex moves alone.
        let Some(Entity::Polyline { points, .. }) = open.with_grip_moved(1, p(12.0, 2.0)) else {
            panic!()
        };
        assert_eq!(points, vec![p(0.0, 0.0), p(12.0, 2.0), p(10.0, 10.0)]);
        // A segment's middle carries both of its vertices.
        let Some(Entity::Polyline { points, .. }) = open.with_grip_moved(4, p(11.0, 5.0)) else {
            panic!()
        };
        assert_eq!(points, vec![p(0.0, 0.0), p(11.0, 0.0), p(11.0, 10.0)]);
        // The closing segment runs from the last vertex back to the first, and
        // its middle (5, 5) carries both.
        let Some(Entity::Polyline { points, .. }) = closed.with_grip_moved(5, p(0.0, 5.0)) else {
            panic!()
        };
        assert_eq!(points, vec![p(-5.0, 0.0), p(10.0, 0.0), p(5.0, 10.0)]);
    }

    #[test]
    fn the_middle_grip_of_an_arc_segment_reshapes_the_arc() {
        // A half circle below the chord from (0, 0) to (4, 0).
        let poly = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)],
            closed: false,
            bulges: vec![1.0, 0.0, 0.0],
            widths: Vec::new(),
        };
        let grips = poly.grips();
        assert_eq!(grips.len(), 3 + 2);
        // The arc's middle (2, -2) is a Resize grip; the straight segment's a Move.
        assert!(near(grips[3].at, p(2.0, -2.0)) && grips[3].kind == GripKind::Resize);
        assert_eq!(grips[4].kind, GripKind::Move);
        // Dragging it deeper makes a deeper arc through the same ends.
        let Some(Entity::Polyline { points, bulges, .. }) = poly.with_grip_moved(3, p(2.0, -4.0))
        else {
            panic!()
        };
        assert_eq!(points, vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)]);
        assert!(bulges[0] > 1.0, "deeper than a half circle: {}", bulges[0]);
        let deeper = Entity::Polyline {
            points,
            closed: false,
            bulges,
            widths: Vec::new(),
        };
        assert!(deeper.distance_to(p(2.0, -4.0)) < 1e-9);
        // Dragging it onto the chord leaves no arc.
        assert_eq!(poly.with_grip_moved(3, p(2.0, 0.0)), None);
        // A vertex next to an arc moves; the arc follows its two ends.
        let Some(Entity::Polyline { points, bulges, .. }) = poly.with_grip_moved(1, p(6.0, 0.0))
        else {
            panic!()
        };
        assert_eq!(points[1], p(6.0, 0.0));
        assert_eq!(
            bulges[0], 1.0,
            "the bulge stays; the arc grows with the chord"
        );
    }

    #[test]
    fn dragging_a_grip_to_where_it_is_changes_nothing_and_bad_numbers_are_refused() {
        for e in [
            line(),
            Entity::Circle {
                center: p(0.0, 0.0),
                radius: 2.0,
            },
            Entity::Rectangle {
                min: p(0.0, 0.0),
                max: p(3.0, 4.0),
            },
        ] {
            for (i, g) in e.grips().iter().enumerate() {
                let same = e.with_grip_moved(i, g.at).unwrap();
                for (a, b) in e.grips().iter().zip(same.grips()) {
                    assert!(near(a.at, b.at), "{e:?} grip {i}");
                }
            }
            assert_eq!(e.with_grip_moved(0, p(f64::NAN, 0.0)), None);
        }
    }
}
