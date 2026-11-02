//! Flattening an object into outlines: linetype dashes, points, dimension
//! pictures and plain curves.

use super::PlotError;
use crate::linetype::dash::{dash_path, Piece};
use crate::linetype::paths::{curve_outlines, is_dashable};
use crate::plot_hatch;
use crate::{Bounds, Document, Entity, Point};

/// An object's outlines, cut into dashes and dots by its linetype. The pattern
/// is laid in the units the outlines are in, so a viewport's scale applies to
/// the dashes as it does to the drawing.
pub(super) fn dashed_outlines(
    document: &Document,
    object: &crate::Object,
    tolerance: f64,
    window: &Bounds,
) -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
    if let Entity::Point { at } = &object.entity {
        // What a point sized as a share of the screen is a share of.
        return point_outlines(document, *at, tolerance, window.max.y - window.min.y);
    }
    // A construction line or ray is cut to the window (a little wider, so its
    // end is not seen) and the pattern laid from its base point.
    let infinite = object
        .entity
        .infinite_line()
        .map(|line| crate::infinite::stand_in(line, document.dash_pattern(object), window));
    let entity = match &infinite {
        Some(None) => return Ok(Vec::new()),
        Some(Some(line)) => line,
        None => &object.entity,
    };
    let Some((elements, scale)) = document
        .dash_pattern(object)
        .filter(|_| is_dashable(entity))
    else {
        return match &infinite {
            Some(_) => Ok(curve_outlines(entity, tolerance, false)
                .map_err(|_| PlotError::TooDetailed)?
                .unwrap_or_default()),
            None => object_outlines(object, tolerance),
        };
    };
    let outlines = curve_outlines(entity, tolerance, !object.attrs.continuous_generation)
        .map_err(|_| PlotError::TooDetailed)?
        .unwrap_or_default();
    // A dot is a hair of line; the round cap makes it a dot.
    let dot = tolerance * 0.1;
    let mut out = Vec::new();
    for (points, closed) in outlines {
        match dash_path(&points, closed, elements, scale) {
            None => out.push((points, closed)),
            Some(pieces) => out.extend(pieces.into_iter().map(|piece| match piece {
                Piece::Dash(run) => (run, false),
                Piece::Dot { at, along } => (
                    vec![at, Point::new(at.x + along.x * dot, at.y + along.y * dot)],
                    false,
                ),
            })),
        }
    }
    Ok(out)
}

/// A point as `PDMODE` and `PDSIZE` draw it: the dot as a hair of line, whose
/// round cap makes it a dot, and the other marks as outlines.
pub(super) fn point_outlines(
    document: &Document,
    at: Point,
    tolerance: f64,
    view_height: f64,
) -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
    let style = document.point_style();
    let marks = style.marks(at, style.size_in_units(view_height));
    let mut all = Vec::new();
    if marks.dot {
        all.push((vec![at, Point::new(at.x + tolerance * 0.1, at.y)], false));
    }
    for shape in &marks.shapes {
        all.extend(outlines(shape, tolerance)?);
    }
    Ok(all)
}

/// A stored object as flattened outlines: a dimension from the picture the
/// object remembers.
pub(super) fn object_outlines(
    object: &crate::Object,
    tolerance: f64,
) -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
    match object.dimension_picture() {
        Some(picture) => dimension_outlines(&picture, tolerance),
        None => outlines(&object.entity, tolerance),
    }
}

/// What a dimension draws (`picture`) as outlines.
pub(super) fn dimension_outlines(
    picture: &[Entity],
    tolerance: f64,
) -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
    let mut all = Vec::new();
    for piece in picture {
        match piece {
            // An arrowhead is a filled triangle on screen and its outline here.
            Entity::Polyline { points, widths, .. }
                if points.len() == 2 && widths.first().is_some_and(|w| w.0 > 0.0) =>
            {
                all.push((arrow_outline(points[0], points[1], widths[0].0), true));
            }
            other => all.extend(outlines(other, tolerance)?),
        }
    }
    Ok(all)
}

/// An entity as flattened outlines (points, closed), curves within `tolerance`.
pub(super) fn outlines(
    entity: &Entity,
    tolerance: f64,
) -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
    let pieces = |parts: Vec<Entity>| -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
        let mut all = Vec::new();
        for piece in &parts {
            all.extend(outlines(piece, tolerance)?);
        }
        Ok(all)
    };
    Ok(match entity {
        // Text is plotted as filled glyphs (`fills`), not strokes.
        Entity::MText(_) | Entity::Text(_) => Vec::new(),
        // A pattern is its strokes; a solid fill is in `fills`.
        Entity::Hatch(h) => plot_hatch::strokes(h, tolerance)?,
        // Callers expand blocks first (`Document::drawn`), so their pieces
        // get their own colours; this is the fallback.
        Entity::Insert(i) => pieces(i.picture())?,
        Entity::Dimension(d) => dimension_outlines(&d.picture(), tolerance)?,
        Entity::MLeader(m) => dimension_outlines(&m.picture(), tolerance)?,
        Entity::Leader(l) => dimension_outlines(&l.picture(), tolerance)?,
        // The curves are shared with the linetype code.
        curve => curve_outlines(curve, tolerance, false)
            .map_err(|_| PlotError::TooDetailed)?
            .unwrap_or_default(),
    })
}

/// The triangle of an arrowhead from its `tail` (of width `base`) to its `tip`.
pub(super) fn arrow_outline(tail: Point, tip: Point, base: f64) -> Vec<Point> {
    let (dx, dy) = (tip.x - tail.x, tip.y - tail.y);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return vec![tail, tip];
    }
    let (nx, ny) = (-dy / len * base / 2.0, dx / len * base / 2.0);
    vec![
        Point::new(tail.x + nx, tail.y + ny),
        tip,
        Point::new(tail.x - nx, tail.y - ny),
    ]
}
