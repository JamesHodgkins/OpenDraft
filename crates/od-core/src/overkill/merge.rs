//! Merging lines, and arcs of one circle, that overlap or meet end to end.

use std::f64::consts::TAU;

use super::equal::near;
use super::Settings;
use crate::{arc, Entity, Point};

/// One object spanning both, when they lie on the same line or circle and
/// overlap or meet in a way `settings` allows.
pub(super) fn combine(a: &Entity, b: &Entity, settings: &Settings) -> Option<Entity> {
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
