//! Object coordinate systems for planar entities (Open Design Specification
//! §20.4 and the DXF "arbitrary axis algorithm").
//!
//! Circles, arcs and polylines are stored in their own coordinate system (the
//! OCS), whose Z axis is the entity's extrusion vector. Only the two
//! orientations that keep drawings flat are accepted: the extrusion along +Z,
//! where the OCS is the world system, and along -Z, where the arbitrary axis
//! algorithm gives X = (-1, 0, 0) and Y = (0, 1, 0), so the OCS is the world
//! system mirrored in X. Any other extrusion is a tilted plane, which is not
//! flattened.

use std::f64::consts::PI;

/// How much an extrusion may differ from the Z axis and still count as along it.
const AXIS_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Extrusion (0, 0, 1): OCS and WCS agree.
    Upright,
    /// Extrusion (0, 0, -1): OCS X points the other way.
    Mirrored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotFlat {
    /// The extrusion is zero or not finite.
    Invalid,
    /// The plane is tilted off the drawing plane.
    Tilted,
}

impl Orientation {
    /// The orientation for an extrusion vector, which need not be unit length.
    pub fn from_extrusion(v: [f64; 3]) -> Result<Orientation, NotFlat> {
        let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if !length.is_finite() || length == 0.0 {
            return Err(NotFlat::Invalid);
        }
        let (x, y, z) = (v[0] / length, v[1] / length, v[2] / length);
        if x.abs() > AXIS_TOLERANCE || y.abs() > AXIS_TOLERANCE {
            Err(NotFlat::Tilted)
        } else if z > 0.0 {
            Ok(Orientation::Upright)
        } else {
            Ok(Orientation::Mirrored)
        }
    }

    /// An OCS point's position in the drawing plane.
    pub fn point(self, p: [f64; 2]) -> [f64; 2] {
        match self {
            Orientation::Upright => p,
            Orientation::Mirrored => [-p[0], p[1]],
        }
    }

    /// The counter-clockwise arc from `start` to `end` (OCS radians) as a
    /// counter-clockwise arc in the drawing plane. Mirroring in X turns an
    /// angle `a` into `pi - a` and reverses the direction, so the ends swap.
    pub fn arc(self, start: f64, end: f64) -> (f64, f64) {
        match self {
            Orientation::Upright => (start, end),
            Orientation::Mirrored => (PI - end, PI - start),
        }
    }

    /// A bulge for a segment, which turns the other way when mirrored.
    pub fn bulge(self, bulge: f64) -> f64 {
        match self {
            Orientation::Upright => bulge,
            Orientation::Mirrored => -bulge,
        }
    }
}

/// Whether a constant elevation is far enough from zero to matter, judged
/// against the size of the coordinates it sits among.
pub fn elevation_matters(elevation: f64, scale: f64) -> bool {
    elevation.abs() > 1e-9 * scale.abs().max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrusions_along_z_are_flat() {
        assert_eq!(
            Orientation::from_extrusion([0.0, 0.0, 1.0]),
            Ok(Orientation::Upright)
        );
        assert_eq!(
            Orientation::from_extrusion([0.0, 0.0, 5.0]),
            Ok(Orientation::Upright)
        );
        assert_eq!(
            Orientation::from_extrusion([0.0, 0.0, -1.0]),
            Ok(Orientation::Mirrored)
        );
        assert_eq!(
            Orientation::from_extrusion([1e-12, 0.0, -2.0]),
            Ok(Orientation::Mirrored)
        );
    }

    #[test]
    fn other_extrusions_are_not_flattened() {
        assert_eq!(
            Orientation::from_extrusion([1.0, 0.0, 0.0]),
            Err(NotFlat::Tilted)
        );
        assert_eq!(
            Orientation::from_extrusion([0.0, 0.6, 0.8]),
            Err(NotFlat::Tilted)
        );
        assert_eq!(
            Orientation::from_extrusion([0.0, 1e-5, 1.0]),
            Err(NotFlat::Tilted)
        );
        assert_eq!(Orientation::from_extrusion([0.0; 3]), Err(NotFlat::Invalid));
        assert_eq!(
            Orientation::from_extrusion([f64::NAN, 0.0, 1.0]),
            Err(NotFlat::Invalid)
        );
        assert_eq!(
            Orientation::from_extrusion([0.0, 0.0, f64::INFINITY]),
            Err(NotFlat::Invalid)
        );
    }

    #[test]
    fn a_mirrored_point_changes_the_sign_of_x() {
        assert_eq!(Orientation::Mirrored.point([3.0, 4.0]), [-3.0, 4.0]);
        assert_eq!(Orientation::Upright.point([3.0, 4.0]), [3.0, 4.0]);
    }

    #[test]
    fn a_mirrored_arc_is_the_reflection_with_its_ends_swapped() {
        // A quarter arc from 0 to 90 degrees in the mirrored system covers the
        // second quadrant in the drawing: 90 to 180 degrees.
        let (s, e) = Orientation::Mirrored.arc(0.0, PI / 2.0);
        assert!((s - PI / 2.0).abs() < 1e-12 && (e - PI).abs() < 1e-12);
        // The midpoint maps consistently: OCS angle 45 degrees -> WCS 135.
        let mid = Orientation::Mirrored.point([(PI / 4.0).cos(), (PI / 4.0).sin()]);
        assert!((mid[1].atan2(mid[0]) - 3.0 * PI / 4.0).abs() < 1e-12);
        assert_eq!(Orientation::Upright.arc(1.0, 2.0), (1.0, 2.0));
    }

    #[test]
    fn mirroring_reverses_the_sense_of_a_bulge() {
        assert_eq!(Orientation::Mirrored.bulge(0.5), -0.5);
        assert_eq!(Orientation::Upright.bulge(0.5), 0.5);
        assert_eq!(Orientation::Mirrored.bulge(0.0), 0.0);
    }

    #[test]
    fn elevation_is_judged_against_the_coordinate_scale() {
        assert!(!elevation_matters(0.0, 100.0));
        assert!(!elevation_matters(1e-12, 1.0));
        assert!(elevation_matters(0.5, 100.0));
        assert!(
            !elevation_matters(1e-4, 1e9),
            "rounding of large coordinates"
        );
        assert!(elevation_matters(1e3, 1e9));
    }
}
