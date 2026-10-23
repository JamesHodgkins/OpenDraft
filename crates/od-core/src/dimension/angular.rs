//! Angular dimensions: the angle between two rays out of a vertex, drawn as
//! an arc about the vertex with extension lines along the rays. The points
//! name the rays (the vertex, a point on each, and where the arc passes);
//! which of the angles the rays make is measured depends on where the arc
//! passes, as AutoCAD's.

use std::f64::consts::{PI, TAU};

use super::arc_length::{draw, Layout};
use super::sketch::{dist, sub};
use super::{Arrow, Built, DimKind, Dimension};
use crate::Point;

/// An angle swept counter-clockwise from `start`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Sector {
    pub start: f64,
    pub sweep: f64,
}

fn angle_of(vertex: Point, to: Point) -> Option<f64> {
    let (x, y) = sub(to, vertex);
    (x.hypot(y) > 1e-12).then(|| y.atan2(x))
}

fn contains(sector: Sector, angle: f64) -> bool {
    (angle - sector.start).rem_euclid(TAU) <= sector.sweep + 1e-12
}

/// The angle measured. Between two rays (`lines` false: three points, an
/// arc or a circle) it is the one the arc passes through, which is either
/// of the two the rays make. Between two lines (`lines` true) it is one of
/// the four their crossing makes.
pub(super) fn sector(kind: &DimKind) -> Option<Sector> {
    let DimKind::Angular {
        vertex,
        p1,
        p2,
        arc_point,
        lines,
    } = kind
    else {
        return None;
    };
    let (a, b) = (angle_of(*vertex, *p1)?, angle_of(*vertex, *p2)?);
    let towards = angle_of(*vertex, *arc_point).unwrap_or_else(|| {
        let turn = (b - a).rem_euclid(TAU);
        a + turn / 2.0
    });
    let turn = (b - a).rem_euclid(TAU);
    if turn < 1e-9 || TAU - turn < 1e-9 {
        return None;
    }
    if !*lines {
        let first = Sector {
            start: a,
            sweep: turn,
        };
        return Some(if contains(first, towards) {
            first
        } else {
            Sector {
                start: b,
                sweep: TAU - turn,
            }
        });
    }
    // Order the rays so the first turns to the second by at most a half
    // turn, then the four sectors are those between a ray and the next.
    let (a, b) = if turn <= PI { (a, b) } else { (b, a) };
    let theta = (b - a).rem_euclid(TAU);
    let sectors = [
        Sector {
            start: a,
            sweep: theta,
        },
        Sector {
            start: b,
            sweep: PI - theta,
        },
        Sector {
            start: a + PI,
            sweep: theta,
        },
        Sector {
            start: b + PI,
            sweep: PI - theta,
        },
    ];
    sectors
        .into_iter()
        .find(|s| contains(*s, towards))
        .or(Some(sectors[0]))
}

/// The sweep, in radians.
pub(super) fn measurement(kind: &DimKind) -> Option<f64> {
    sector(kind).map(|s| s.sweep).filter(|s| *s > 1e-9)
}

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::Angular {
        vertex,
        p1,
        p2,
        arc_point,
        ..
    } = &dim.kind
    else {
        return None;
    };
    let Sector { start, sweep } = sector(&dim.kind)?;
    if sweep < 1e-9 {
        return None;
    }
    let (r1, r2) = (dist(*vertex, *p1), dist(*vertex, *p2));
    let (a, b) = (angle_of(*vertex, *p1)?, angle_of(*vertex, *p2)?);
    // The distance of the point that names the ray a boundary lies along
    // (or opposite to).
    let reference = |boundary: f64| {
        let off = |ray: f64| {
            (boundary - ray)
                .rem_euclid(PI)
                .min(PI - (boundary - ray).rem_euclid(PI))
        };
        if off(a) < 1e-6 {
            r1
        } else if off(b) < 1e-6 {
            r2
        } else {
            r1.max(r2)
        }
    };
    let size = if dim.style.arrow == Arrow::None {
        0.0
    } else {
        dim.style.arrow_size * dim.style.scale
    };
    let reach = r1.max(r2);
    let on_arc = dist(*vertex, *arc_point);
    draw(
        dim,
        Layout {
            c: *vertex,
            a1: start,
            sweep,
            on_arc: if on_arc > 1e-12 {
                on_arc
            } else {
                reach + 3.0 * size.max(1.0)
            },
            measured: reach,
            ends: [reference(start), reference(start + sweep)],
            leader: false,
            symbol: false,
            label: dim.label(),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::DimStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn angular(through: Point, lines: bool) -> Dimension {
        Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 10.0),
            through,
            lines,
            &DimStyle::default(),
        )
    }

    #[test]
    fn three_points_measure_the_angle_the_arc_passes_through() {
        let inside = angular(p(7.0, 7.0), false);
        assert!((inside.measurement().unwrap() - FRAC_PI_2).abs() < 1e-9);
        let outside = angular(p(-7.0, -7.0), false);
        assert!((outside.measurement().unwrap() - 3.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn two_lines_make_four_angles_chosen_by_the_pick() {
        let quarter = |at: Point| angular(at, true).measurement().unwrap();
        for at in [p(7.0, 7.0), p(-7.0, -7.0), p(-7.0, 7.0), p(7.0, -7.0)] {
            assert!((quarter(at) - FRAC_PI_2).abs() < 1e-9, "{at:?}");
        }
        let slanted = Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(7.0, 3.0),
            true,
            &DimStyle::default(),
        );
        assert!((slanted.measurement().unwrap() - PI / 4.0).abs() < 1e-9);
        let beside = Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(-3.0, 7.0),
            true,
            &DimStyle::default(),
        );
        assert!((beside.measurement().unwrap() - 3.0 * PI / 4.0).abs() < 1e-9);
    }

    #[test]
    fn the_text_reads_in_degrees() {
        assert_eq!(angular(p(7.0, 7.0), false).label(), "90");
        let precise = Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(7.0, 3.0),
            false,
            &DimStyle {
                angle_precision: 2,
                ..DimStyle::default()
            },
        );
        assert_eq!(precise.label(), "45.00");
    }

    #[test]
    fn the_picture_has_an_arc_arrows_and_text() {
        let d = angular(p(7.0, 7.0), false);
        let pieces = d.picture();
        assert!(pieces
            .iter()
            .any(|e| matches!(e, crate::Entity::Arc { .. })));
        assert!(pieces.iter().any(|e| matches!(e, crate::Entity::MText(_))));
        let centre = d.text_centre().unwrap();
        let radius = dist(p(0.0, 0.0), p(7.0, 7.0));
        assert!(dist(p(0.0, 0.0), centre) > radius - 1.0);
    }

    #[test]
    fn rays_along_one_line_or_a_missing_ray_make_nothing() {
        let same = Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(5.0, 0.0),
            p(3.0, 3.0),
            false,
            &DimStyle::default(),
        );
        assert_eq!(same.measurement(), None);
        assert!(same.picture().is_empty());
        let none = Dimension::angular(
            p(1.0, 1.0),
            p(1.0, 1.0),
            p(5.0, 0.0),
            p(3.0, 3.0),
            false,
            &DimStyle::default(),
        );
        assert_eq!(none.measurement(), None);
    }
}
