//! Shared by the hatch tests.

pub(super) use super::*;
pub(super) use crate::{Entity, Point};

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(super) fn square(x: f64, y: f64, side: f64) -> HatchLoop {
    HatchLoop {
        flags: HatchLoopFlags(HatchLoopFlags::EXTERNAL | HatchLoopFlags::POLYLINE),
        boundary: HatchBoundary::Polyline(HatchPolyline {
            vertices: vec![
                p(x, y),
                p(x + side, y),
                p(x + side, y + side),
                p(x, y + side),
            ],
            bulges: Vec::new(),
            closed: true,
        }),
        sources: Vec::new(),
    }
}

pub(super) fn solid(loops: Vec<HatchLoop>, style: HatchStyle) -> HatchEntity {
    HatchEntity {
        style,
        ..HatchEntity::new(loops, HatchFill::Solid)
    }
}

pub(super) fn catalog_pattern(name: &str, scale: f64, angle: f64, origin: Point) -> HatchPattern {
    let def = builtin().find(name).unwrap();
    HatchPattern::from_definition(def, PatternType::Predefined, scale, angle, origin).unwrap()
}

pub(super) fn patterned(loops: Vec<HatchLoop>, pattern: HatchPattern) -> HatchEntity {
    HatchEntity::new(loops, HatchFill::Pattern(pattern))
}

pub(super) const TOL: f64 = 1e-3;

pub(super) fn eval(h: &HatchEntity) -> EvaluatedHatch {
    h.evaluate(TOL, &HatchLimits::default()).unwrap()
}

pub(super) fn length(strokes: &[[Point; 2]]) -> f64 {
    strokes
        .iter()
        .map(|s| (s[0].x - s[1].x).hypot(s[0].y - s[1].y))
        .sum()
}

/// Squares nested to `depth` levels, each 2 smaller on a side than the last.
pub(super) fn nested(depth: usize) -> Vec<HatchLoop> {
    (0..depth)
        .map(|i| square(i as f64, i as f64, 20.0 - 2.0 * i as f64))
        .collect()
}
