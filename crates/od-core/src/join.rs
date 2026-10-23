//! Joining objects that meet end to end: lines, arcs and open polylines, and
//! elliptical arcs of one ellipse among themselves. The base for JOIN.
//!
//! Pieces whose ends touch are chained into one polyline (a segment of an
//! arc keeps its curve as a bulge). Two special cases give back a simpler
//! object: lines that all lie on one line become a single line, and arcs of
//! one circle become a single arc (or a circle, when they close the loop).

use std::f64::consts::TAU;

use crate::{arc, bulge, Entity, Point};

/// One object made from several.
#[derive(Clone, Debug, PartialEq)]
pub struct Merged {
    /// Which of the given objects were used, lowest first. The first one is
    /// the one to replace; the others go.
    pub used: Vec<usize>,
    pub entity: Entity,
}

/// A run of segments: `points`, and one bulge per point (the last unused).
#[derive(Clone)]
struct Chain {
    points: Vec<Point>,
    bulges: Vec<f64>,
}

impl Chain {
    /// The object as a chain, if it is a line, an arc or an open polyline.
    fn of(entity: &Entity) -> Option<Chain> {
        match entity {
            Entity::Line { start, end } if distance(*start, *end) > 0.0 => Some(Chain {
                points: vec![*start, *end],
                bulges: vec![0.0, 0.0],
            }),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let sweep = arc::sweep(*start, *end);
                (sweep < TAU - 1e-9 && *radius > 0.0).then(|| Chain {
                    points: vec![
                        arc::point_at(*center, *radius, *start),
                        arc::point_at(*center, *radius, *end),
                    ],
                    bulges: vec![bulge::from_sweep(sweep, true), 0.0],
                })
            }
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                ..
            } if points.len() >= 2 => {
                let mut bulges: Vec<f64> =
                    (0..points.len()).map(|i| bulge::at(bulges, i)).collect();
                if let Some(last) = bulges.last_mut() {
                    *last = 0.0;
                }
                Some(Chain {
                    points: points.clone(),
                    bulges,
                })
            }
            _ => None,
        }
    }

    fn first(&self) -> Point {
        self.points[0]
    }

    fn last(&self) -> Point {
        self.points[self.points.len() - 1]
    }

    /// The same run the other way round: each curve turns the other way.
    fn reversed(&self) -> Chain {
        let n = self.points.len();
        let mut bulges: Vec<f64> = (0..n - 1).rev().map(|i| -self.bulges[i]).collect();
        bulges.push(0.0);
        Chain {
            points: self.points.iter().rev().copied().collect(),
            bulges,
        }
    }

    /// `other` added on, starting where this one ends.
    fn append(&mut self, other: &Chain) {
        let n = self.points.len();
        self.bulges[n - 1] = other.bulges[0];
        self.points.extend_from_slice(&other.points[1..]);
        self.bulges.extend_from_slice(&other.bulges[1..]);
    }
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Join whatever of `items` meets end to end (ends within `tolerance`).
/// Objects that meet nothing are left out, as are those that cannot join
/// (circles, rectangles, closed polylines). A single open polyline whose ends
/// meet is closed.
pub fn join(items: &[Entity], tolerance: f64) -> Vec<Merged> {
    let chains: Vec<Option<Chain>> = items.iter().map(Chain::of).collect();
    let mut taken = vec![false; items.len()];
    let mut made = Vec::new();
    for seed in 0..items.len() {
        let Some(start) = chains[seed].clone().filter(|_| !taken[seed]) else {
            continue;
        };
        taken[seed] = true;
        let mut chain = start;
        let mut used = vec![seed];
        while let Some(next) = grow(&mut chain, &chains, &mut taken, tolerance) {
            used.push(next);
        }
        let closes = chain.points.len() >= 3 && distance(chain.first(), chain.last()) <= tolerance;
        if used.len() < 2 && !closes {
            taken[seed] = false;
            continue;
        }
        used.sort_unstable();
        let entity = result(chain, closes, tolerance);
        made.push(Merged { used, entity });
    }
    made.extend(crate::ellipse::join::join_elliptical_arcs(items, tolerance));
    made
}

/// Add the first untaken piece that meets an end of `chain` and return its
/// index.
fn grow(
    chain: &mut Chain,
    chains: &[Option<Chain>],
    taken: &mut [bool],
    tolerance: f64,
) -> Option<usize> {
    if chain.points.len() >= 3 && distance(chain.first(), chain.last()) <= tolerance {
        return None;
    }
    for (i, other) in chains.iter().enumerate() {
        let Some(other) = other else { continue };
        if taken[i] {
            continue;
        }
        let at_end = [other.clone(), other.reversed()]
            .into_iter()
            .find(|c| distance(chain.last(), c.first()) <= tolerance);
        if let Some(piece) = at_end {
            chain.append(&piece);
            taken[i] = true;
            return Some(i);
        }
        let at_start = [other.clone(), other.reversed()]
            .into_iter()
            .find(|c| distance(c.last(), chain.first()) <= tolerance);
        if let Some(piece) = at_start {
            let mut joined = piece;
            joined.append(chain);
            *chain = joined;
            taken[i] = true;
            return Some(i);
        }
    }
    None
}

/// The object a finished chain becomes.
fn result(mut chain: Chain, closed: bool, tolerance: f64) -> Entity {
    if closed {
        chain.points.pop();
        chain.bulges.pop();
    } else {
        // An open chain of lines on one line is a line.
        if let Some(line) = as_line(&chain, tolerance) {
            return line;
        }
    }
    if let Some(round) = as_arc(&chain, closed, tolerance) {
        return round;
    }
    let has_arcs = bulge::has_arcs(&chain.bulges);
    Entity::Polyline {
        points: chain.points,
        closed,
        bulges: if has_arcs { chain.bulges } else { Vec::new() },
        widths: Vec::new(),
    }
}

fn as_line(chain: &Chain, tolerance: f64) -> Option<Entity> {
    if bulge::has_arcs(&chain.bulges) {
        return None;
    }
    let (a, b) = (chain.first(), chain.last());
    let length = distance(a, b);
    if length <= tolerance {
        return None;
    }
    let (ux, uy) = ((b.x - a.x) / length, (b.y - a.y) / length);
    let straight = chain.points.windows(2).all(|w| {
        let along = (w[1].x - w[0].x) * ux + (w[1].y - w[0].y) * uy;
        along > 0.0
    }) && chain
        .points
        .iter()
        .all(|p| ((p.x - a.x) * uy - (p.y - a.y) * ux).abs() <= tolerance);
    straight.then_some(Entity::Line { start: a, end: b })
}

/// A chain whose segments are all arcs of one circle, turning one way, is a
/// single arc (or the whole circle, when it closes).
fn as_arc(chain: &Chain, closed: bool, tolerance: f64) -> Option<Entity> {
    let n = chain.points.len();
    let count = bulge::segment_count(n, closed);
    let arcs: Vec<bulge::SegmentArc> = (0..count)
        .map(|i| bulge::arc_of(chain.points[i], chain.points[(i + 1) % n], chain.bulges[i]))
        .collect::<Option<_>>()?;
    let first = arcs.first()?;
    let same = arcs.iter().all(|a| {
        a.forward == first.forward
            && distance(a.center, first.center) <= tolerance
            && (a.radius - first.radius).abs() <= tolerance
    });
    if !same {
        return None;
    }
    let total: f64 = arcs.iter().map(|a| a.sweep).sum();
    if total >= TAU - 1e-6 {
        return Some(Entity::Circle {
            center: first.center,
            radius: first.radius,
        });
    }
    let last = arcs.last()?;
    let (start, end) = if first.forward {
        (first.start, last.end)
    } else {
        (last.start, first.end)
    };
    Some(Entity::Arc {
        center: first.center,
        radius: first.radius,
        start,
        end,
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn quarter(start: f64) -> Entity {
        Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start,
            end: start + FRAC_PI_2,
        }
    }

    #[test]
    fn lines_end_to_end_make_one_polyline() {
        let items = [
            line(p(0.0, 0.0), p(10.0, 0.0)),
            line(p(10.0, 10.0), p(10.0, 0.0)),
            line(p(0.0, 0.0), p(0.0, -4.0)),
        ];
        let made = join(&items, 1e-6);
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].used, vec![0, 1, 2]);
        assert_eq!(
            made[0].entity,
            Entity::Polyline {
                points: vec![p(0.0, -4.0), p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new(),
            }
        );
    }

    #[test]
    fn lines_on_one_line_make_a_line() {
        let items = [
            line(p(0.0, 0.0), p(4.0, 0.0)),
            line(p(9.0, 0.0), p(4.0, 0.0)),
        ];
        let made = join(&items, 1e-6);
        assert_eq!(made[0].entity, line(p(0.0, 0.0), p(9.0, 0.0)));
    }

    #[test]
    fn a_line_doubling_back_is_not_one_line() {
        let items = [
            line(p(0.0, 0.0), p(4.0, 0.0)),
            line(p(4.0, 0.0), p(1.0, 0.0)),
        ];
        let made = join(&items, 1e-6);
        assert!(matches!(made[0].entity, Entity::Polyline { .. }));
    }

    #[test]
    fn a_line_and_an_arc_keep_the_curve() {
        let items = [quarter(0.0), line(p(0.0, 5.0), p(-5.0, 5.0))];
        let made = join(&items, 1e-6);
        let Entity::Polyline { points, bulges, .. } = &made[0].entity else {
            panic!("expected a polyline");
        };
        assert_eq!(points.len(), 3);
        assert!((bulges[0] - (PI / 8.0).tan()).abs() < 1e-9);
        assert_eq!(bulges[1], 0.0);
    }

    #[test]
    fn an_arc_met_backwards_turns_the_other_way() {
        // The line ends where the arc ends, so the arc is walked in reverse.
        let items = [line(p(5.0, -5.0), p(0.0, 5.0)), quarter(0.0)];
        let made = join(&items, 1e-6);
        let Entity::Polyline { points, bulges, .. } = &made[0].entity else {
            panic!("expected a polyline");
        };
        assert_eq!(points[0], p(5.0, -5.0));
        assert!((points[1].x).abs() < 1e-9 && (points[1].y - 5.0).abs() < 1e-9);
        assert!(bulges[1] < 0.0);
    }

    #[test]
    fn arcs_of_one_circle_make_an_arc() {
        let items = [quarter(0.0), quarter(FRAC_PI_2)];
        let made = join(&items, 1e-6);
        let Entity::Arc {
            start, end, radius, ..
        } = made[0].entity
        else {
            panic!("expected an arc");
        };
        assert!(start.abs() < 1e-9 && (end - PI).abs() < 1e-9 && (radius - 5.0).abs() < 1e-9);
    }

    #[test]
    fn arcs_that_close_make_a_circle() {
        let items = [
            quarter(0.0),
            quarter(FRAC_PI_2),
            quarter(PI),
            quarter(PI + FRAC_PI_2),
        ];
        let made = join(&items, 1e-6);
        assert_eq!(made.len(), 1);
        assert!(
            matches!(made[0].entity, Entity::Circle { radius, .. } if (radius - 5.0).abs() < 1e-9)
        );
    }

    #[test]
    fn a_loop_of_lines_is_closed() {
        let items = [
            line(p(0.0, 0.0), p(4.0, 0.0)),
            line(p(4.0, 0.0), p(4.0, 3.0)),
            line(p(4.0, 3.0), p(0.0, 0.0)),
        ];
        let made = join(&items, 1e-6);
        assert!(matches!(
            &made[0].entity,
            Entity::Polyline { points, closed: true, .. } if points.len() == 3
        ));
    }

    #[test]
    fn a_polyline_whose_ends_meet_is_closed() {
        let open = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0), p(0.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let made = join(&[open], 1e-6);
        assert_eq!(made[0].used, vec![0]);
        assert!(matches!(
            &made[0].entity,
            Entity::Polyline { closed: true, .. }
        ));
    }

    #[test]
    fn things_that_do_not_meet_are_left_alone() {
        let items = [
            line(p(0.0, 0.0), p(1.0, 0.0)),
            line(p(5.0, 0.0), p(6.0, 0.0)),
            Entity::Circle {
                center: p(0.0, 0.0),
                radius: 1.0,
            },
        ];
        assert!(join(&items, 1e-6).is_empty());
    }

    #[test]
    fn separate_groups_stay_separate() {
        let items = [
            line(p(0.0, 0.0), p(1.0, 0.0)),
            line(p(1.0, 0.0), p(1.0, 1.0)),
            line(p(10.0, 0.0), p(11.0, 0.0)),
            line(p(11.0, 0.0), p(11.0, 1.0)),
        ];
        let made = join(&items, 1e-6);
        assert_eq!(made.len(), 2);
        assert_eq!(made[1].used, vec![2, 3]);
    }
}
