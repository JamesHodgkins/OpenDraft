//! OVERKILL: remove duplicate objects and merge lines and arcs that overlap
//! or meet end to end on the same line or circle. Pure; the command only
//! asks which objects, groups them by the properties that must match, and
//! applies the outcome.

use std::f64::consts::TAU;

use crate::{arc, bulge, Entity, Point};

/// What OVERKILL may do beyond removing exact duplicates and objects lying
/// wholly on top of another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// How far apart two numbers may be and still count as equal.
    pub tolerance: f64,
    /// Merge objects that overlap only in part into one that spans both.
    pub overlapping: bool,
    /// Merge objects that touch end to end into one.
    pub end_to_end: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tolerance: 1e-6,
            overlapping: true,
            end_to_end: true,
        }
    }
}

/// What to do to the objects given, by position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// Objects that become something else (a longer line, a bigger arc).
    pub changed: Vec<(usize, Entity)>,
    /// Objects to delete, lowest first.
    pub removed: Vec<usize>,
}

/// Tidy `items`: each is an object and a group number; only objects of one
/// group are compared (the caller groups by layer, colour and so on).
pub fn overkill(items: &[(Entity, usize)], settings: &Settings) -> Outcome {
    let mut now: Vec<Entity> = items.iter().map(|(e, _)| e.clone()).collect();
    let mut gone = vec![false; items.len()];
    for i in 0..items.len() {
        if gone[i] {
            continue;
        }
        for j in i + 1..items.len() {
            if !gone[j] && items[i].1 == items[j].1 && same(&now[i], &now[j], settings.tolerance) {
                gone[j] = true;
            }
        }
    }
    for i in 0..items.len() {
        let mut j = i + 1;
        while j < items.len() {
            if !gone[i] && !gone[j] && items[i].1 == items[j].1 {
                if let Some(merged) = combine(&now[i], &now[j], settings) {
                    now[i] = merged;
                    gone[j] = true;
                    // The longer object may now reach ones passed over.
                    j = i + 1;
                    continue;
                }
            }
            j += 1;
        }
    }
    let changed = (0..items.len())
        .filter(|&i| !gone[i] && now[i] != items[i].0)
        .map(|i| (i, now[i].clone()))
        .collect();
    let removed = (0..items.len()).filter(|&i| gone[i]).collect();
    Outcome { changed, removed }
}

fn near(a: Point, b: Point, tol: f64) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= tol
}

/// Whether two objects are the same shape in the same place.
fn same(a: &Entity, b: &Entity, tol: f64) -> bool {
    match (a, b) {
        (Entity::Line { start: s1, end: e1 }, Entity::Line { start: s2, end: e2 }) => {
            (near(*s1, *s2, tol) && near(*e1, *e2, tol))
                || (near(*s1, *e2, tol) && near(*e1, *s2, tol))
        }
        (
            Entity::Circle {
                center: c1,
                radius: r1,
            },
            Entity::Circle {
                center: c2,
                radius: r2,
            },
        ) => near(*c1, *c2, tol) && (r1 - r2).abs() <= tol,
        (
            Entity::Arc {
                center: c1,
                radius: r1,
                start: s1,
                end: e1,
            },
            Entity::Arc {
                center: c2,
                radius: r2,
                start: s2,
                end: e2,
            },
        ) => {
            near(*c1, *c2, tol)
                && (r1 - r2).abs() <= tol
                && near(
                    arc::point_at(*c1, *r1, *s1),
                    arc::point_at(*c2, *r2, *s2),
                    tol,
                )
                && near(
                    arc::point_at(*c1, *r1, *e1),
                    arc::point_at(*c2, *r2, *e2),
                    tol,
                )
        }
        (Entity::Rectangle { min: a1, max: b1 }, Entity::Rectangle { min: a2, max: b2 }) => {
            near(*a1, *a2, tol) && near(*b1, *b2, tol)
        }
        (Entity::Point { at: p }, Entity::Point { at: q }) => near(*p, *q, tol),
        (Entity::Polyline { .. }, Entity::Polyline { .. }) => same_polyline(a, b, tol),
        _ => a == b,
    }
}

fn same_polyline(a: &Entity, b: &Entity, tol: f64) -> bool {
    let (
        Entity::Polyline {
            points: p1,
            closed: c1,
            bulges: g1,
            widths: w1,
        },
        Entity::Polyline {
            points: p2,
            closed: c2,
            bulges: g2,
            widths: w2,
        },
    ) = (a, b)
    else {
        return false;
    };
    if c1 != c2 || p1.len() != p2.len() || w1 != w2 {
        return false;
    }
    let n = p1.len();
    let forward = (0..n)
        .all(|i| near(p1[i], p2[i], tol) && (bulge::at(g1, i) - bulge::at(g2, i)).abs() <= tol);
    // Open and the other way round: segment i of one is segment n-2-i of the
    // other, turning the opposite way.
    let backward = !c1
        && (0..n).all(|i| near(p1[i], p2[n - 1 - i], tol))
        && (0..n.saturating_sub(1))
            .all(|i| (bulge::at(g1, i) + bulge::at(g2, n - 2 - i)).abs() <= tol);
    forward || backward
}

/// One object spanning both, when they lie on the same line or circle and
/// overlap or meet in a way `settings` allows.
fn combine(a: &Entity, b: &Entity, settings: &Settings) -> Option<Entity> {
    match (a, b) {
        (Entity::Line { start: p, end: q }, Entity::Line { start: r, end: t }) => {
            combine_lines((*p, *q), (*r, *t), settings)
        }
        (
            Entity::Arc {
                center,
                radius,
                start,
                end,
            },
            Entity::Arc {
                center: c2,
                radius: r2,
                start: s2,
                end: e2,
            },
        ) if near(*center, *c2, settings.tolerance)
            && (radius - r2).abs() <= settings.tolerance
            && *radius > settings.tolerance =>
        {
            combine_arcs(*center, *radius, (*start, *end), (*s2, *e2), settings)
        }
        // A circle covers every arc of itself.
        (
            Entity::Circle { center, radius },
            Entity::Arc {
                center: c2,
                radius: r2,
                ..
            },
        )
        | (
            Entity::Arc {
                center: c2,
                radius: r2,
                ..
            },
            Entity::Circle { center, radius },
        ) if near(*center, *c2, settings.tolerance)
            && (radius - r2).abs() <= settings.tolerance =>
        {
            Some(Entity::Circle {
                center: *center,
                radius: *radius,
            })
        }
        _ => None,
    }
}

fn combine_lines(a: (Point, Point), b: (Point, Point), settings: &Settings) -> Option<Entity> {
    let tol = settings.tolerance;
    let len = (a.1.x - a.0.x).hypot(a.1.y - a.0.y);
    let other = (b.1.x - b.0.x).hypot(b.1.y - b.0.y);
    if len <= tol || other <= tol {
        return None;
    }
    let (ux, uy) = ((a.1.x - a.0.x) / len, (a.1.y - a.0.y) / len);
    // Along the first line, and how far off it, for a point.
    let along = |p: Point| (p.x - a.0.x) * ux + (p.y - a.0.y) * uy;
    let off = |p: Point| ((p.x - a.0.x) * uy - (p.y - a.0.y) * ux).abs();
    if off(b.0) > tol || off(b.1) > tol {
        return None;
    }
    let (lo, hi) = {
        let (x, y) = (along(b.0), along(b.1));
        (x.min(y), x.max(y))
    };
    let overlap = len.min(hi) - 0.0f64.max(lo);
    let wholly = (lo >= -tol && hi <= len + tol) || (lo <= tol && hi >= len - tol);
    let allowed = wholly
        || (overlap > tol && settings.overlapping)
        || (overlap >= -tol && overlap <= tol && settings.end_to_end);
    if !allowed {
        return None;
    }
    let (from, to) = (0.0f64.min(lo), len.max(hi));
    let at = |t: f64| Point::new(a.0.x + ux * t, a.0.y + uy * t);
    Some(Entity::Line {
        start: if from == 0.0 { a.0 } else { at(from) },
        end: if to == len { a.1 } else { at(to) },
    })
}

fn combine_arcs(
    center: Point,
    radius: f64,
    a: (f64, f64),
    b: (f64, f64),
    settings: &Settings,
) -> Option<Entity> {
    let tol = settings.tolerance / radius;
    let (w1, w2) = (arc::sweep(a.0, a.1), arc::sweep(b.0, b.1));
    // How far round from the start of one the start of the other lies.
    let from = |x: f64, y: f64| {
        let d = (y - x).rem_euclid(TAU);
        if d > TAU - tol {
            0.0
        } else {
            d
        }
    };
    let (d1, d2) = (from(a.0, b.0), from(b.0, a.0));
    let wholly = d1 + w2 <= w1 + tol || d2 + w1 <= w2 + tol;
    let allowed_for = |d: f64, w: f64| {
        // `d` past the first arc's start, along a first arc of sweep `w`.
        let gap = d - w;
        wholly || (gap < -tol && settings.overlapping) || (gap.abs() <= tol && settings.end_to_end)
    };
    let (start, sweep) = if d1 <= w1 + tol && allowed_for(d1, w1) {
        (a.0, w1.max(d1 + w2))
    } else if d2 <= w2 + tol && allowed_for(d2, w2) {
        (b.0, w2.max(d2 + w1))
    } else {
        return None;
    };
    Some(if sweep >= TAU - tol {
        Entity::Circle { center, radius }
    } else {
        Entity::Arc {
            center,
            radius,
            start,
            end: arc::normalize(start + sweep),
        }
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Entity {
        Entity::Line {
            start: p(x1, y1),
            end: p(x2, y2),
        }
    }

    fn arc_of(start: f64, end: f64) -> Entity {
        Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start,
            end,
        }
    }

    fn run(items: &[Entity], settings: &Settings) -> Outcome {
        let grouped: Vec<_> = items.iter().map(|e| (e.clone(), 0)).collect();
        overkill(&grouped, settings)
    }

    #[test]
    fn a_duplicate_line_goes_even_when_drawn_the_other_way() {
        let out = run(
            &[line(0.0, 0.0, 5.0, 0.0), line(5.0, 0.0, 0.0, 0.0)],
            &Settings::default(),
        );
        assert_eq!(out.removed, vec![1]);
        assert!(out.changed.is_empty());
    }

    #[test]
    fn lines_within_the_tolerance_are_duplicates_and_beyond_it_are_not() {
        let near = [line(0.0, 0.0, 5.0, 0.0), line(0.0, 0.001, 5.0, 0.001)];
        let loose = Settings {
            tolerance: 0.01,
            ..Settings::default()
        };
        assert_eq!(run(&near, &loose).removed, vec![1]);
        assert!(run(&near, &Settings::default()).removed.is_empty());
    }

    #[test]
    fn a_line_inside_another_goes_and_the_first_takes_the_longer_span() {
        let out = run(
            &[line(2.0, 0.0, 3.0, 0.0), line(0.0, 0.0, 10.0, 0.0)],
            &Settings {
                overlapping: false,
                end_to_end: false,
                ..Settings::default()
            },
        );
        assert_eq!(out.removed, vec![1]);
        assert_eq!(out.changed, vec![(0, line(0.0, 0.0, 10.0, 0.0))]);
    }

    #[test]
    fn partly_overlapping_lines_merge_only_when_allowed() {
        let items = [line(0.0, 0.0, 6.0, 0.0), line(4.0, 0.0, 10.0, 0.0)];
        let out = run(&items, &Settings::default());
        assert_eq!(out.changed, vec![(0, line(0.0, 0.0, 10.0, 0.0))]);
        assert_eq!(out.removed, vec![1]);
        let off = Settings {
            overlapping: false,
            ..Settings::default()
        };
        assert_eq!(run(&items, &off), Outcome::default());
    }

    #[test]
    fn lines_meeting_end_to_end_merge_only_when_allowed() {
        let items = [line(0.0, 0.0, 4.0, 0.0), line(4.0, 0.0, 9.0, 0.0)];
        assert_eq!(run(&items, &Settings::default()).removed, vec![1]);
        let off = Settings {
            end_to_end: false,
            ..Settings::default()
        };
        assert_eq!(run(&items, &off), Outcome::default());
    }

    #[test]
    fn lines_that_are_not_collinear_or_leave_a_gap_stay() {
        let out = run(
            &[
                line(0.0, 0.0, 4.0, 0.0),
                line(4.0, 1.0, 9.0, 1.0),
                line(5.0, 0.0, 9.0, 0.0),
            ],
            &Settings::default(),
        );
        assert_eq!(out, Outcome::default());
    }

    #[test]
    fn a_chain_of_overlaps_ends_as_one_line() {
        let out = run(
            &[
                line(0.0, 0.0, 3.0, 0.0),
                line(8.0, 0.0, 12.0, 0.0),
                line(2.0, 0.0, 9.0, 0.0),
            ],
            &Settings::default(),
        );
        assert_eq!(out.removed, vec![1, 2]);
        assert_eq!(out.changed, vec![(0, line(0.0, 0.0, 12.0, 0.0))]);
    }

    #[test]
    fn objects_of_different_groups_are_left_alone() {
        let out = overkill(
            &[(line(0.0, 0.0, 5.0, 0.0), 0), (line(0.0, 0.0, 5.0, 0.0), 1)],
            &Settings::default(),
        );
        assert_eq!(out, Outcome::default());
    }

    #[test]
    fn arcs_of_one_circle_merge_and_two_halves_make_a_circle() {
        let out = run(&[arc_of(0.0, 2.0), arc_of(1.0, 3.0)], &Settings::default());
        assert_eq!(out.removed, vec![1]);
        assert!(matches!(&out.changed[0].1,
            Entity::Arc { start, end, .. } if start.abs() < 1e-9 && (end - 3.0).abs() < 1e-9));
        let full = run(&[arc_of(0.0, PI), arc_of(PI, 0.0)], &Settings::default());
        assert!(matches!(full.changed[0].1, Entity::Circle { .. }));
    }

    #[test]
    fn an_arc_across_the_zero_angle_merges_the_right_way_round() {
        let out = run(&[arc_of(6.0, 0.5), arc_of(0.2, 1.0)], &Settings::default());
        assert!(matches!(&out.changed[0].1,
            Entity::Arc { start, end, .. } if (start - 6.0).abs() < 1e-9 && (end - 1.0).abs() < 1e-9));
    }

    #[test]
    fn arcs_with_a_gap_or_another_radius_stay() {
        let other = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 6.0,
            start: 1.0,
            end: 2.0,
        };
        let out = run(
            &[arc_of(0.0, 1.0), arc_of(1.5, 2.5), other],
            &Settings::default(),
        );
        assert_eq!(out, Outcome::default());
    }

    #[test]
    fn a_circle_swallows_its_arcs_and_a_duplicate_circle() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let out = run(
            &[arc_of(0.0, 1.0), circle.clone(), circle],
            &Settings::default(),
        );
        assert_eq!(out.removed.len(), 2);
        assert_eq!(out.changed.len(), 1);
        assert!(matches!(out.changed[0].1, Entity::Circle { .. }));
    }

    #[test]
    fn duplicate_polylines_match_in_either_direction_with_their_curves() {
        let a = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: vec![0.0, 0.5, 0.0],
            widths: Vec::new(),
        };
        let b = Entity::Polyline {
            points: vec![p(4.0, 3.0), p(4.0, 0.0), p(0.0, 0.0)],
            closed: false,
            bulges: vec![-0.5, 0.0, 0.0],
            widths: Vec::new(),
        };
        assert_eq!(run(&[a.clone(), b], &Settings::default()).removed, vec![1]);
        let c = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert!(run(&[a, c], &Settings::default()).removed.is_empty());
    }
}
