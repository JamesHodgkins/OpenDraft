//! Working out what a hatch draws: the filled area and the pattern strokes.
//! Pure and bounded; nothing here changes the hatch.

use super::clip;
use super::topology;
use super::{HatchEntity, HatchError, HatchFill};
use crate::{Bounds, Point};

/// How much work one evaluation may do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HatchLimits {
    /// Pattern lines over the whole hatch (AutoCAD's `HPMAXLINES`).
    pub max_pattern_lines: u64,
    /// Strokes and dots produced.
    pub max_segments: usize,
    /// Places where pattern lines meet the boundary.
    pub max_crossings: usize,
}

impl Default for HatchLimits {
    fn default() -> Self {
        Self {
            max_pattern_lines: 100_000,
            max_segments: 1_000_000,
            max_crossings: 4_000_000,
        }
    }
}

impl HatchLimits {
    /// A much smaller budget, for a preview that must answer at once.
    pub fn preview() -> Self {
        Self {
            max_pattern_lines: 5_000,
            max_segments: 50_000,
            max_crossings: 200_000,
        }
    }
}

/// What a hatch draws, in drawing coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedHatch {
    /// Polygons whose even-odd union is the filled area. For a pattern it is
    /// the area the lines are cut to.
    pub fill: Vec<Vec<Point>>,
    /// Pattern strokes. Empty for solid and gradient fills.
    pub strokes: Vec<[Point; 2]>,
    /// Pattern dots (zero-length dashes).
    pub dots: Vec<Point>,
    /// The area filled.
    pub area: f64,
    /// The box around the boundary.
    pub bounds: Bounds,
}

impl HatchEntity {
    /// The box around all the loops, or `None` for a hatch with none.
    pub fn bounds(&self) -> Option<Bounds> {
        self.loops
            .iter()
            .filter_map(|l| l.bounds())
            .reduce(Bounds::union)
    }

    /// Every loop as a polygon, curves followed within `tolerance`. This is
    /// the boundary alone: nothing is left out for the island style.
    pub fn contours(&self, tolerance: f64) -> Result<Vec<Vec<Point>>, HatchError> {
        self.validate()?;
        let bounds = self.bounds().ok_or(HatchError::TooLarge("boundaries"))?;
        let size = (bounds.max.x - bounds.min.x).hypot(bounds.max.y - bounds.min.y);
        let close = (size * 1e-7).max(1e-9);
        let tolerance = tolerance.max(size * 1e-6).max(1e-12);
        self.loops
            .iter()
            .enumerate()
            .map(|(i, l)| l.contour(i, tolerance, close))
            .collect()
    }

    /// The filled area and, for a pattern, its strokes. `tolerance` is how
    /// far a flattened curve may stray from the true one.
    pub fn evaluate(
        &self,
        tolerance: f64,
        limits: &HatchLimits,
    ) -> Result<EvaluatedHatch, HatchError> {
        let contours = self.contours(tolerance)?;
        let bounds = self.bounds().ok_or(HatchError::TooLarge("boundaries"))?;
        let selected = topology::select(contours, self.style)?;
        let strokes = match &self.fill {
            HatchFill::Pattern(pattern) => {
                clip::strokes(&selected.contours, &pattern.lines, limits)?
            }
            _ => clip::Strokes::default(),
        };
        Ok(EvaluatedHatch {
            fill: selected.contours,
            strokes: strokes.segments,
            dots: strokes.dots,
            area: selected.area,
            bounds,
        })
    }

    /// The area filled, working on the boundary alone (no pattern strokes).
    pub fn area(&self, tolerance: f64) -> Option<f64> {
        let contours = self.contours(tolerance).ok()?;
        Some(topology::select(contours, self.style).ok()?.area)
    }

    /// Whether `p` is in the filled area of the boundary (by the island
    /// style), working on the boundary alone.
    pub fn contains(&self, p: Point, tolerance: f64) -> bool {
        let Ok(contours) = self.contours(tolerance) else {
            return false;
        };
        let Ok(selected) = topology::select(contours, self.style) else {
            return false;
        };
        selected
            .contours
            .iter()
            .filter(|c| topology::contains(c, p))
            .count()
            % 2
            == 1
    }

    /// The distance from `p` to the nearest boundary edge, or `None` if the
    /// boundary cannot be followed.
    pub fn distance_to_boundary(&self, p: Point, tolerance: f64) -> Option<f64> {
        let contours = self.contours(tolerance).ok()?;
        contours
            .iter()
            .flat_map(|c| (0..c.len()).map(move |i| (c[i], c[(i + 1) % c.len()])))
            .map(|(a, b)| distance_to_segment(p, a, b))
            .reduce(f64::min)
    }
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}
