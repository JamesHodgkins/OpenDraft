//! What makes a polyline's four lists possible, and the limits we put on
//! them. The limits are OpenDraft's own safety policy, not AutoCAD's.

use std::fmt;

use crate::Point;

/// The most vertices one polyline may have.
pub const MAX_VERTICES: usize = 1_000_000;
/// The largest bulge magnitude (a bulge of 1e12 is a hairline-thin arc).
pub const MAX_BULGE: f64 = 1.0e12;
/// The widest a segment may be, in drawing units.
pub const MAX_WIDTH: f64 = 1.0e12;

/// Why a polyline could not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolylineError {
    TooFewVertices,
    TooManyVertices(usize),
    NonFinitePoint(usize),
    /// `bulges` is neither empty nor one per point.
    BadBulgeCount,
    BadBulge(usize),
    /// `widths` is neither empty nor one per point.
    BadWidthCount,
    BadWidth(usize),
}

impl fmt::Display for PolylineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewVertices => write!(f, "it has fewer than two vertices"),
            Self::TooManyVertices(n) => {
                write!(f, "it has {n} vertices (the limit is {MAX_VERTICES})")
            }
            Self::NonFinitePoint(i) => write!(f, "vertex {i} is not a finite point"),
            Self::BadBulgeCount => write!(f, "its bulges are not one per vertex"),
            Self::BadBulge(i) => write!(f, "the bulge at vertex {i} is not usable"),
            Self::BadWidthCount => write!(f, "its widths are not one per vertex"),
            Self::BadWidth(i) => write!(f, "the width at vertex {i} is not usable"),
        }
    }
}

impl std::error::Error for PolylineError {}

/// Whether these lists make a polyline.
pub fn check(points: &[Point], bulges: &[f64], widths: &[(f64, f64)]) -> Result<(), PolylineError> {
    if points.len() < 2 {
        return Err(PolylineError::TooFewVertices);
    }
    if points.len() > MAX_VERTICES {
        return Err(PolylineError::TooManyVertices(points.len()));
    }
    if let Some(i) = points
        .iter()
        .position(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(PolylineError::NonFinitePoint(i));
    }
    if !bulges.is_empty() && bulges.len() != points.len() {
        return Err(PolylineError::BadBulgeCount);
    }
    if let Some(i) = bulges
        .iter()
        .position(|b| !b.is_finite() || b.abs() > MAX_BULGE)
    {
        return Err(PolylineError::BadBulge(i));
    }
    if !widths.is_empty() && widths.len() != points.len() {
        return Err(PolylineError::BadWidthCount);
    }
    let usable = |w: f64| w.is_finite() && (0.0..=MAX_WIDTH).contains(&w);
    if let Some(i) = widths.iter().position(|&(a, b)| !usable(a) || !usable(b)) {
        return Err(PolylineError::BadWidth(i));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(n: usize) -> Vec<Point> {
        (0..n).map(|i| Point::new(i as f64, 0.0)).collect()
    }

    #[test]
    fn a_plain_polyline_is_fine() {
        assert_eq!(check(&pts(2), &[], &[]), Ok(()));
        assert_eq!(check(&pts(3), &[0.0, 0.5, 0.0], &[(1.0, 2.0); 3]), Ok(()));
    }

    #[test]
    fn each_way_it_can_be_wrong_is_found() {
        assert_eq!(check(&pts(1), &[], &[]), Err(PolylineError::TooFewVertices));
        let mut bad = pts(3);
        bad[1].y = f64::NAN;
        assert_eq!(check(&bad, &[], &[]), Err(PolylineError::NonFinitePoint(1)));
        assert_eq!(
            check(&pts(3), &[0.0], &[]),
            Err(PolylineError::BadBulgeCount)
        );
        assert_eq!(
            check(&pts(2), &[0.0, f64::INFINITY], &[]),
            Err(PolylineError::BadBulge(1))
        );
        assert_eq!(
            check(&pts(2), &[0.0, 1.0e13], &[]),
            Err(PolylineError::BadBulge(1))
        );
        assert_eq!(
            check(&pts(3), &[], &[(0.0, 0.0)]),
            Err(PolylineError::BadWidthCount)
        );
        assert_eq!(
            check(&pts(2), &[], &[(0.0, 0.0), (-1.0, 0.0)]),
            Err(PolylineError::BadWidth(1))
        );
    }

    #[test]
    fn too_many_vertices_are_refused() {
        assert_eq!(
            check(&pts(MAX_VERTICES + 1), &[], &[]),
            Err(PolylineError::TooManyVertices(MAX_VERTICES + 1))
        );
    }
}
