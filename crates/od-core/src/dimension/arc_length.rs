//! Arc-length dimensions: a dimension arc, with extension lines running
//! radially out from the two ends of the measured arc.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::fit::fit;
use super::font;
use super::sketch::{along, direction, dist, readable, sub, Sketch};
use super::{Arrow, Built, DimKind, Dimension};
use crate::Point;

/// The arc-length symbol is not in the bundled font, so it is drawn: a
/// shallow arc this wide and `SYMBOL_RISE` high, in text heights, followed by
/// `SYMBOL_GAP` when it leads the number.
const SYMBOL_WIDTH: f64 = 0.7;
const SYMBOL_RISE: f64 = 0.3;
const SYMBOL_GAP: f64 = 0.3;

/// The symbol with the middle of its base at `mid`, upright for text running
/// along `angle`.
fn arc_symbol(out: &mut Sketch, mid: Point, angle: f64, h: f64) {
    let (half, rise) = (SYMBOL_WIDTH * h / 2.0, SYMBOL_RISE * h);
    let radius = (half * half + rise * rise) / (2.0 * rise);
    let up = direction(angle + FRAC_PI_2);
    let centre = along(mid, up, rise - radius);
    let spread = (half / radius).asin();
    out.arc(
        centre,
        radius,
        angle + FRAC_PI_2 - spread,
        angle + FRAC_PI_2 + spread,
    );
}

/// The arc's radius, and its angles and sweep (counter-clockwise from the
/// start to the end; equal angles are a full circle).
struct Geometry {
    radius: f64,
    start: f64,
    sweep: f64,
}

fn geometry(kind: &DimKind) -> Option<Geometry> {
    let DimKind::ArcLength {
        center, start, end, ..
    } = kind
    else {
        return None;
    };
    let (a, b) = (sub(*start, *center), sub(*end, *center));
    let radius = (a.0.hypot(a.1) + b.0.hypot(b.1)) / 2.0;
    if !(radius.is_finite() && radius > 1e-12) {
        return None;
    }
    let (a1, a2) = (a.1.atan2(a.0), b.1.atan2(b.0));
    let mut sweep = (a2 - a1).rem_euclid(TAU);
    if sweep < 1e-12 {
        sweep = TAU;
    }
    Some(Geometry {
        radius,
        start: a1,
        sweep,
    })
}

/// Radius times the counter-clockwise sweep.
pub(super) fn measurement(kind: &DimKind) -> Option<f64> {
    geometry(kind).map(|g| g.radius * g.sweep)
}

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::ArcLength {
        center,
        start,
        end,
        arc_point,
        leader,
    } = &dim.kind
    else {
        return None;
    };
    let Geometry {
        radius: measured,
        start: a1,
        sweep,
    } = geometry(&dim.kind)?;
    draw(
        dim,
        Layout {
            c: *center,
            a1,
            sweep,
            on_arc: dist(*center, *arc_point),
            measured,
            ends: [dist(*center, *start), dist(*center, *end)],
            leader: *leader,
            symbol: true,
            label: dim.label(),
            contents: dim.contents(),
        },
    )
}

/// What a dimension arc is drawn from: its centre, where it starts and how
/// far it turns, how far out the user put it, and the radii the extension
/// lines start at (an angular dimension shares this with arc length).
pub(super) struct Layout {
    pub c: Point,
    pub a1: f64,
    pub sweep: f64,
    /// The radius of the point the user placed the arc through (0 for none).
    pub on_arc: f64,
    /// The measured radius, where a leader points and a missing `on_arc`
    /// falls back from.
    pub measured: f64,
    pub ends: [f64; 2],
    pub leader: bool,
    /// Whether the arc-length symbol goes with the text.
    pub symbol: bool,
    pub label: String,
    /// The label as multiline text contents.
    pub contents: String,
}

pub(super) fn draw(dim: &Dimension, layout: Layout) -> Option<Built> {
    let Layout {
        c,
        a1,
        sweep,
        on_arc,
        measured,
        ends,
        leader,
        symbol,
        label,
        contents,
    } = layout;
    let s = dim.style.resolved();
    let size = if s.arrow == Arrow::None {
        0.0
    } else {
        s.arrow_size
    };
    let ra = if on_arc > 1e-12 {
        on_arc
    } else {
        measured + 3.0 * size.max(1.0)
    };
    let a2 = a1 + sweep;
    let mid = a1 + sweep / 2.0;

    let symbol_mode = if symbol { s.arc_symbol } else { 2 };
    let (h, g) = (s.text_height, s.text_gap);
    // The symbol in front of the number takes room of its own; `width` is
    // the whole run, symbol and number.
    let lead = if !label.is_empty() && symbol_mode == 0 {
        SYMBOL_WIDTH + SYMBOL_GAP
    } else {
        0.0
    };
    let width = font::contents_width(&contents, h) + lead * h;
    let clearance = if label.is_empty() {
        0.0
    } else {
        width + 2.0 * g
    };
    let length = ra * sweep;
    let (arrows_in, text_in) = fit(length, size, clearance, s.fit, s.text_inside);

    let radial = direction(mid);
    let horizontal = if text_in {
        s.inside_horizontal
    } else {
        s.outside_horizontal
    };
    let text_angle = if horizontal {
        0.0
    } else {
        readable(mid + FRAC_PI_2)
    };
    let text_angle = dim.text_angle.unwrap_or(text_angle);
    // Text runs straight along the tangent while the arc falls away from it,
    // so it is raised by how far the arc sags under the text's ends.
    let half = (width / 2.0).min(ra);
    let sag = ra - (ra * ra - half * half).sqrt();
    let lift = g + h / 2.0 + if horizontal { 0.0 } else { sag };
    let centred = s.text_above == 0 && text_in && !horizontal;
    let default_centre = {
        let m = along(c, radial, ra);
        let offset = match s.text_above {
            4 if !text_in || horizontal => -lift,
            4 => -lift,
            0 if centred => 0.0,
            _ => lift,
        };
        along(m, radial, offset)
    };
    let centre = dim.text_pos.unwrap_or(default_centre);

    let mut out = Sketch::default();
    // Extension lines run along the radius through each end.
    for (i, (r, angle)) in [(ends[0], a1), (ends[1], a2)].into_iter().enumerate() {
        let sign = if ra >= r { 1.0 } else { -1.0 };
        if s.suppress_ext[i] || (ra - r).abs() <= s.ext_offset {
            continue;
        }
        let u = direction(angle);
        out.line(
            along(c, u, r + sign * s.ext_offset),
            along(c, u, ra + sign * s.ext_extend),
        );
    }
    // The dimension arc, broken around centred text.
    let half_gap = if centred && !label.is_empty() && dim.text_pos.is_none() {
        (width / 2.0 + g) / ra
    } else {
        0.0
    };
    let tangent_start = (-a1.sin(), a1.cos());
    let tangent_end = (-a2.sin(), a2.cos());
    let second = s.arrow2.unwrap_or(s.arrow);
    let tip1 = along(c, direction(a1), ra);
    let tip2 = along(c, direction(a2), ra);
    if arrows_in {
        out.arrow(s.arrow, tip1, tangent_start, size);
        out.arrow(second, tip2, (-tangent_end.0, -tangent_end.1), size);
    } else {
        out.arrow(s.arrow, tip1, (-tangent_start.0, -tangent_start.1), size);
        out.arrow(second, tip2, tangent_end, size);
        let stub = 2.0 * size / ra;
        out.arc(c, ra, a1 - stub, a1);
        out.arc(c, ra, a2, a2 + stub);
    }
    if arrows_in || text_in || s.force_line {
        if half_gap > 0.0 && half_gap < sweep / 2.0 {
            out.arc(c, ra, a1, mid - half_gap);
            out.arc(c, ra, mid + half_gap, a2);
        } else if half_gap == 0.0 {
            out.arc(c, ra, a1, a2);
        }
    }
    if !label.is_empty() {
        let (run, up) = (direction(text_angle), direction(text_angle + FRAC_PI_2));
        match symbol_mode {
            // In front of the number, its top level with the capitals.
            0 => {
                let front = along(centre, run, (SYMBOL_WIDTH * h - width) / 2.0);
                arc_symbol(&mut out, along(front, up, 0.2 * h), text_angle, h);
            }
            // Over the number.
            1 => arc_symbol(&mut out, along(centre, up, 0.6 * h), text_angle, h),
            _ => {}
        }
        let text_at = along(centre, run, lead * h / 2.0);
        out.label(&s, &contents, width, centre, text_at, text_angle);
    }
    // The leader runs radially, a quarter of the way along, so it clears
    // the text at the middle.
    if leader && sweep > FRAC_PI_2 && (ra - measured).abs() > size {
        let at = a1 + sweep / 4.0;
        let sign = if ra >= measured { 1.0 } else { -1.0 };
        let (tip, far) = (
            along(c, direction(at), measured),
            along(c, direction(at), ra),
        );
        out.line(tip, far);
        out.arrow(s.arrow, tip, (sign * at.cos(), sign * at.sin()), size);
    }
    Some(Built {
        pieces: out.pieces,
        text_centre: centre,
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::{DimStyle, Entity, Point};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn quarter(style: &DimStyle) -> Dimension {
        Dimension::arc_length(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 10.0),
            p(12.0, 12.0),
            style,
        )
    }

    #[test]
    fn a_quarter_circle_measures_a_quarter_of_the_circumference() {
        let d = quarter(&DimStyle::default());
        assert!((d.measurement().unwrap() - 10.0 * PI / 2.0).abs() < 1e-9);
    }

    #[test]
    fn an_arc_crossing_zero_keeps_its_sweep() {
        // From 315 degrees round through zero to 45: a quarter turn.
        let (s, e) = (PI * 1.75, PI * 0.25);
        let d = Dimension::arc_length(
            p(0.0, 0.0),
            p(10.0 * s.cos(), 10.0 * s.sin()),
            p(10.0 * e.cos(), 10.0 * e.sin()),
            p(12.0, 0.0),
            &DimStyle::default(),
        );
        assert!((d.measurement().unwrap() - 10.0 * PI / 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_long_way_round_is_a_major_arc() {
        let d = Dimension::arc_length(
            p(0.0, 0.0),
            p(0.0, 10.0),
            p(10.0, 0.0),
            p(12.0, 12.0),
            &DimStyle::default(),
        );
        assert!((d.measurement().unwrap() - 10.0 * PI * 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_leader_is_drawn_only_for_a_long_arc_and_runs_radially() {
        let half = |leader: bool| {
            let mut d = Dimension::arc_length(
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(-10.0, 0.0),
                p(14.0, 0.0),
                &DimStyle::default(),
            );
            if let DimKind::ArcLength { leader: l, .. } = &mut d.kind {
                *l = leader;
            }
            d
        };
        let lines = |d: &Dimension| {
            d.picture()
                .into_iter()
                .filter(|e| matches!(e, crate::Entity::Line { .. }))
                .count()
        };
        assert_eq!(lines(&half(true)), lines(&half(false)) + 1);
        let with_leader = {
            let mut d = quarter(&DimStyle::default());
            if let DimKind::ArcLength { leader, .. } = &mut d.kind {
                *leader = true;
            }
            d
        };
        assert_eq!(lines(&with_leader), lines(&quarter(&DimStyle::default())));
    }

    #[test]
    fn equal_ends_make_a_full_circle() {
        let d = Dimension::arc_length(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 0.0),
            p(12.0, 0.0),
            &DimStyle::default(),
        );
        assert!((d.measurement().unwrap() - 20.0 * PI).abs() < 1e-9);
    }

    #[test]
    fn the_dimension_arc_lies_at_the_arc_point_radius() {
        let d = quarter(&DimStyle::default());
        let radius = (12.0_f64).hypot(12.0);
        let arcs: Vec<f64> = d
            .picture()
            .into_iter()
            .filter_map(|e| match e {
                // The symbol's own small arc is centred elsewhere.
                Entity::Arc { center, radius, .. } if center == p(0.0, 0.0) => Some(radius),
                _ => None,
            })
            .collect();
        assert!(!arcs.is_empty());
        assert!(arcs.iter().all(|r| (r - radius).abs() < 1e-9));
    }

    #[test]
    fn the_symbol_can_be_left_out() {
        let with = quarter(&DimStyle::default());
        let without = quarter(&DimStyle {
            arc_symbol: 2,
            ..DimStyle::default()
        });
        assert!(with.picture().len() > without.picture().len());
    }

    #[test]
    fn text_sits_outside_the_dimension_arc() {
        let d = quarter(&DimStyle::default());
        let c = d.text_centre().unwrap();
        assert!(c.x.hypot(c.y) > (12.0_f64).hypot(12.0));
    }

    #[test]
    fn coincident_centre_and_end_make_nothing() {
        let d = Dimension::arc_length(
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(5.0, 0.0),
            &DimStyle::default(),
        );
        assert!(d.picture().is_empty());
    }
}
