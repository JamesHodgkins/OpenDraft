//! Hatches on a plot: a pattern as its strokes, a solid fill as filled
//! polygons (outer loops counter-clockwise and islands clockwise, which the
//! non-zero rule of `PlotFill` fills the same as even-odd).

use crate::hatch::{HatchEntity, HatchError, HatchFill, HatchLimits};
use crate::plot::PlotError;
use crate::Point;

fn failed(why: HatchError) -> PlotError {
    match why {
        HatchError::TooDense { .. } => PlotError::HatchTooDense,
        // A hatch that cannot be followed is left off the sheet.
        _ => PlotError::HatchTooDense,
    }
}

/// The pattern's strokes as open paths, dots as very short ones.
pub(crate) fn strokes(
    h: &HatchEntity,
    tolerance: f64,
) -> Result<Vec<(Vec<Point>, bool)>, PlotError> {
    if !matches!(h.fill, HatchFill::Pattern(_)) {
        return Ok(Vec::new());
    }
    let done = match h.evaluate(tolerance, &HatchLimits::default()) {
        Ok(done) => done,
        // A hatch whose boundary is broken is skipped, as on screen.
        Err(HatchError::TooDense { .. }) => return Err(PlotError::HatchTooDense),
        Err(_) => return Ok(Vec::new()),
    };
    let mut paths: Vec<(Vec<Point>, bool)> = done
        .strokes
        .iter()
        .map(|s| (vec![s[0], s[1]], false))
        .collect();
    paths.extend(
        done.dots
            .iter()
            .map(|d| (vec![*d, Point::new(d.x + tolerance, d.y)], false)),
    );
    Ok(paths)
}

/// The polygons of a solid or gradient fill (nothing for a pattern).
pub(crate) fn fill(h: &HatchEntity, tolerance: f64) -> Result<Option<Vec<Vec<Point>>>, PlotError> {
    if matches!(h.fill, HatchFill::Pattern(_)) {
        return Ok(None);
    }
    match h.evaluate(tolerance, &HatchLimits::default()) {
        Ok(done) => Ok(Some(done.fill)),
        Err(HatchError::TooDense { .. }) => Err(failed(HatchError::TooDense {
            estimated: 0,
            limit: 0,
        })),
        Err(_) => Ok(None),
    }
}
