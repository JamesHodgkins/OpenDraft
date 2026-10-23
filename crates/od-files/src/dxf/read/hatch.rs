//! HATCH records, in ENTITIES.
//!
//! The groups of a HATCH repeat codes with different meanings (`10` is the
//! elevation, then a vertex, then a seed point; `97` is a fit-point count
//! inside a spline edge and a source count after a loop), so the record is
//! read in order with a cursor, not by code. Every count is checked against
//! the groups that remain, and against the hatch limits, before anything is
//! gathered.
//!
//! Angles in the file are degrees, except a gradient's rotation. A clockwise
//! arc or ellipse edge (`73` is 0) carries its angles measured clockwise, as
//! AutoCAD's own curve classes do, so they are mirrored on the way in
//! (`360 - angle`) and out.
//!
//! The loop's source objects (`330`) are kept as unresolved handles. A hatch
//! must lie flat in the drawing plane (normal along +Z); one facing down is
//! skipped by name.

use std::f64::consts::TAU;

use od_core::hatch::{
    builtin, BoundarySource, HatchBoundary, HatchEdge, HatchEntity, HatchFill, HatchGradient,
    HatchLoop, HatchLoopFlags, HatchPattern, HatchPolyline, HatchSpline, HatchStyle, PatternLine,
    PatternType, MAX_DASHES, MAX_EDGES, MAX_LOOPS, MAX_PATTERN_LINES, MAX_SEEDS, MAX_STOPS,
    MAX_VERTICES,
};
use od_core::{Entity, Point};

use super::{Group, Importer, Record};

/// Most groups of boundary data one file's hatches may use together.
pub(super) const MAX_FILE_HATCH_GROUPS: usize = 8_000_000;
/// Most sources one loop may name.
const MAX_SOURCES: usize = 4096;

enum Fail {
    Damaged,
    NotFlat,
    TooLarge,
}

struct Cursor<'a> {
    groups: &'a [Group],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<i32> {
        self.groups.get(self.at).map(|g| g.code)
    }

    fn left(&self) -> usize {
        self.groups.len() - self.at
    }

    fn next_if(&mut self, code: i32) -> Option<&'a str> {
        let g = self.groups.get(self.at).filter(|g| g.code == code)?;
        self.at += 1;
        Some(g.value.as_str())
    }

    fn num(&mut self, code: i32) -> Result<f64, Fail> {
        self.next_if(code)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or(Fail::Damaged)
    }

    fn int(&mut self, code: i32) -> Result<i64, Fail> {
        self.next_if(code)
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or(Fail::Damaged)
    }

    /// A count, checked against `limit` and against the groups left, each
    /// item needing at least `groups` of them.
    fn count(&mut self, code: i32, limit: usize, groups: usize) -> Result<usize, Fail> {
        let n = usize::try_from(self.int(code)?).map_err(|_| Fail::Damaged)?;
        if n > limit {
            return Err(Fail::TooLarge);
        }
        if n.saturating_mul(groups) > self.left() {
            return Err(Fail::Damaged);
        }
        Ok(n)
    }

    /// A point whose y follows x by 10 (`10`/`20`, `11`/`21`).
    fn point(&mut self, x: i32) -> Result<Point, Fail> {
        self.point_at(x, x + 10)
    }

    fn point_at(&mut self, x: i32, y: i32) -> Result<Point, Fail> {
        Ok(Point::new(self.num(x)?, self.num(y)?))
    }
}

fn mirrored(degrees: f64, clockwise: bool) -> f64 {
    if clockwise {
        TAU - degrees.to_radians()
    } else {
        degrees.to_radians()
    }
}

fn edge(c: &mut Cursor) -> Result<HatchEdge, Fail> {
    match c.int(72)? {
        1 => Ok(HatchEdge::Line {
            start: c.point(10)?,
            end: c.point(11)?,
        }),
        2 => {
            let center = c.point(10)?;
            let radius = c.num(40)?;
            let (start, end) = (c.num(50)?, c.num(51)?);
            let ccw = c.int(73)? != 0;
            Ok(HatchEdge::CircularArc {
                center,
                radius,
                start_angle: mirrored(start, !ccw),
                end_angle: mirrored(end, !ccw),
                counter_clockwise: ccw,
            })
        }
        3 => {
            let center = c.point(10)?;
            let major_axis = c.point(11)?;
            let ratio = c.num(40)?;
            let (start, end) = (c.num(50)?, c.num(51)?);
            let ccw = c.int(73)? != 0;
            Ok(HatchEdge::EllipticArc {
                center,
                major_axis,
                minor_major_ratio: ratio,
                start_param: mirrored(start, !ccw),
                end_param: mirrored(end, !ccw),
                counter_clockwise: ccw,
            })
        }
        4 => spline_edge(c),
        _ => Err(Fail::Damaged),
    }
}

fn spline_edge(c: &mut Cursor) -> Result<HatchEdge, Fail> {
    let degree = u8::try_from(c.int(94)?).map_err(|_| Fail::Damaged)?;
    let rational = c.int(73)? != 0;
    let periodic = c.int(74)? != 0;
    let knot_count = c.count(95, MAX_VERTICES, 1)?;
    let control_count = c.count(96, MAX_VERTICES, 2)?;
    let knots = (0..knot_count)
        .map(|_| c.num(40))
        .collect::<Result<Vec<_>, _>>()?;
    let mut control_points = Vec::with_capacity(control_count);
    let mut weights = Vec::new();
    for _ in 0..control_count {
        control_points.push(c.point(10)?);
        if rational {
            weights.push(c.num(42)?);
        }
    }
    // The fit-point count is written after the control points; a file
    // without it goes straight on to the loop's source count.
    let mut fit_points = Vec::new();
    let (mut start_tangent, mut end_tangent) = (None, None);
    let has_fit_count =
        c.peek() == Some(97) && c.groups.get(c.at + 1).is_none_or(|next| next.code != 330);
    if has_fit_count {
        let n = c.count(97, MAX_VERTICES, 2)?;
        for _ in 0..n {
            fit_points.push(c.point(11)?);
        }
        if c.peek() == Some(12) {
            start_tangent = Some(c.point(12)?);
        }
        if c.peek() == Some(13) {
            end_tangent = Some(c.point(13)?);
        }
    }
    Ok(HatchEdge::Spline(HatchSpline {
        degree,
        periodic,
        knots,
        control_points,
        weights,
        fit_points,
        start_tangent,
        end_tangent,
    }))
}

fn hatch_loop(c: &mut Cursor) -> Result<HatchLoop, Fail> {
    let flags = HatchLoopFlags(u32::try_from(c.int(92)?).map_err(|_| Fail::Damaged)?);
    let boundary = if c.peek() == Some(72) {
        let has_bulge = c.int(72)? != 0;
        let closed = c.int(73)? != 0;
        let n = c.count(93, MAX_VERTICES, 2)?;
        let mut vertices = Vec::with_capacity(n);
        let mut bulges = Vec::new();
        for _ in 0..n {
            vertices.push(c.point(10)?);
            if has_bulge {
                bulges.push(c.num(42)?);
            }
        }
        HatchBoundary::Polyline(HatchPolyline {
            vertices,
            bulges,
            closed,
        })
    } else {
        let n = c.count(93, MAX_EDGES, 2)?;
        HatchBoundary::Edges((0..n).map(|_| edge(c)).collect::<Result<_, _>>()?)
    };
    let mut sources = Vec::new();
    if c.peek() == Some(97) {
        let n = c.count(97, MAX_SOURCES, 1)?;
        for _ in 0..n {
            let handle = c.next_if(330).ok_or(Fail::Damaged)?;
            sources.push(BoundarySource::External {
                scheme: "acad-handle".to_owned(),
                value: handle.to_owned(),
            });
        }
    }
    let polyline = matches!(boundary, HatchBoundary::Polyline(_));
    Ok(HatchLoop {
        flags: flags.with(HatchLoopFlags::POLYLINE, polyline),
        boundary,
        sources,
    })
}

fn pattern_lines(c: &mut Cursor) -> Result<Vec<PatternLine>, Fail> {
    let n = c.count(78, MAX_PATTERN_LINES, 6)?;
    let mut lines = Vec::with_capacity(n);
    for _ in 0..n {
        let angle = c.num(53)?.to_radians();
        let base = c.point_at(43, 44)?;
        let offset = c.point_at(45, 46)?;
        let dashes = c.count(79, MAX_DASHES, 1)?;
        lines.push(PatternLine {
            angle,
            base,
            offset: [offset.x, offset.y],
            dashes: (0..dashes).map(|_| c.num(49)).collect::<Result<_, _>>()?,
        });
    }
    Ok(lines)
}

/// Where a named pattern's origin must have been, if it is one we know:
/// AutoCAD bakes it into the line bases.
fn origin_of(
    name: &str,
    kind: PatternType,
    scale: f64,
    angle: f64,
    lines: &[PatternLine],
) -> Point {
    let known = builtin()
        .find(name)
        .filter(|d| kind == PatternType::Predefined && d.lines.len() == lines.len() && scale > 0.0);
    let (Some(def), Some(first)) = (known, lines.first()) else {
        return Point::default();
    };
    let own = def.lines[0].origin;
    let local = Point::new(own.0 * scale, own.1 * scale).rotated(Point::default(), angle);
    Point::new(first.base.x - local.x, first.base.y - local.y)
}

fn color_of(aci: Option<i64>, rgb: Option<i64>) -> [u8; 3] {
    match (rgb, aci) {
        (Some(v), _) => [(v >> 16) as u8, (v >> 8) as u8, v as u8],
        (None, Some(i)) if (1..=255).contains(&i) => {
            let (r, g, b) = od_core::aci::aci_rgb(i as u8);
            [r, g, b]
        }
        _ => [255, 255, 255],
    }
}

fn gradient(c: &mut Cursor, name: &str) -> Result<Option<HatchGradient>, Fail> {
    if c.peek() != Some(450) {
        return Ok(None);
    }
    let is_gradient = c.int(450)? == 1;
    c.int(451)?;
    let angle = c.num(460)?;
    let shift = c.num(461)?;
    let one_color = c.int(452)? != 0;
    let tint = c.num(462)?;
    let n = c.count(453, MAX_STOPS, 2)?;
    let mut colors = Vec::with_capacity(n);
    for _ in 0..n {
        let position = c.num(463)?;
        let aci = c.next_if(63).and_then(|v| v.parse::<i64>().ok());
        let rgb = c.next_if(421).and_then(|v| v.parse::<i64>().ok());
        colors.push((position, color_of(aci, rgb)));
    }
    let kind = c.next_if(470).unwrap_or(name).to_owned();
    Ok(is_gradient.then_some(HatchGradient {
        name: kind,
        angle,
        shift,
        one_color,
        tint,
        colors,
    }))
}

fn read(record: &Record, budget: &mut usize) -> Result<HatchEntity, Fail> {
    let groups = &record.groups;
    if groups.len() > *budget {
        return Err(Fail::TooLarge);
    }
    *budget -= groups.len();
    let loops_at = groups
        .iter()
        .position(|g| g.code == 91)
        .ok_or(Fail::Damaged)?;
    let head = |code: i32| groups[..loops_at].iter().find(|g| g.code == code);
    let number = |code: i32, default: f64| match head(code) {
        None => Ok(default),
        Some(g) => g
            .value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or(Fail::Damaged),
    };
    let elevation = number(30, 0.0)?;
    let (nx, ny, nz) = (number(210, 0.0)?, number(220, 0.0)?, number(230, 1.0)?);
    let length = (nx * nx + ny * ny + nz * nz).sqrt();
    if !(length.is_finite() && length > 0.0) {
        return Err(Fail::Damaged);
    }
    if nx.abs() > 1e-9 * length || ny.abs() > 1e-9 * length || nz < 0.0 {
        return Err(Fail::NotFlat);
    }
    let name = head(2).map_or("SOLID", |g| g.value.as_str());
    let solid = head(70).and_then(|g| g.value.parse::<i64>().ok()) == Some(1);
    let associative = head(71).and_then(|g| g.value.parse::<i64>().ok()) == Some(1);

    let mut c = Cursor {
        groups,
        at: loops_at,
    };
    let loop_count = c.count(91, MAX_LOOPS, 3)?;
    let loops = (0..loop_count)
        .map(|_| hatch_loop(&mut c))
        .collect::<Result<Vec<_>, _>>()?;
    let style = HatchStyle::from_code(c.int(75)?).ok_or(Fail::Damaged)?;
    let kind = PatternType::from_code(c.int(76)?).ok_or(Fail::Damaged)?;
    let mut fill = HatchFill::Solid;
    if !solid {
        let angle = if c.peek() == Some(52) {
            c.num(52)?.to_radians()
        } else {
            0.0
        };
        let scale = if c.peek() == Some(41) {
            c.num(41)?
        } else {
            1.0
        };
        let double = c.peek() == Some(77) && c.int(77)? != 0;
        let lines = pattern_lines(&mut c)?;
        fill = HatchFill::Pattern(HatchPattern {
            pattern_type: kind,
            name: name.to_owned(),
            angle,
            scale_or_spacing: scale,
            double,
            origin: origin_of(name, kind, scale, angle, &lines),
            lines,
        });
    }
    let source_pixel_size = if c.peek() == Some(47) {
        Some(c.num(47)?)
    } else {
        None
    };
    let mut seed_points = Vec::new();
    if c.peek() == Some(98) {
        let n = c.count(98, MAX_SEEDS, 2)?;
        for _ in 0..n {
            seed_points.push(c.point(10)?);
        }
    }
    if solid {
        if let Some(g) = gradient(&mut c, name)? {
            fill = HatchFill::Gradient(g);
        }
    }
    let hatch = HatchEntity {
        elevation,
        loops,
        style,
        fill,
        associative,
        seed_points,
        source_pixel_size,
    };
    hatch.validate().map_err(|_| Fail::Damaged)?;
    Ok(hatch)
}

impl Importer {
    pub(super) fn hatch(&mut self, record: &Record) {
        if self.reading_block() {
            self.skip("HATCH (inside a block)");
            return;
        }
        match read(record, &mut self.hatch_budget) {
            Ok(h) => self.add(record, Entity::Hatch(Box::new(h))),
            Err(Fail::NotFlat) => self.skip("HATCH (not facing up in the drawing plane)"),
            Err(Fail::TooLarge) => self.skip("HATCH (too large)"),
            Err(Fail::Damaged) => self.damaged += 1,
        }
    }
}
