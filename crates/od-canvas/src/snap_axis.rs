//! Ortho applied to the pointer: after snapping, the point is held to the
//! horizontal or vertical through the prompt's base point.

use od_core::ortho::{axis_of, on_axis};
use od_core::{Document, Point};

use crate::hit::probe;
use crate::snap::{snap_all, Probe, SnapId, SnapKind, SnapModes, SnapPoint};

/// Whether points are held to the horizontal or vertical (`ORTHOMODE`), and
/// how firmly (`ORTHOLOCK`). Standard lets an object snap win, as in AutoCAD;
/// strict finds the object snap and then holds its point to the axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Ortho {
    pub enabled: bool,
    pub strict: bool,
}

/// What the pointer snaps to: the drawing, the active snaps and the sizes
/// that apply, and the base point ortho holds points to (none: no ortho).
pub(crate) struct Aim<'a> {
    pub document: &'a Document,
    pub modes: SnapModes,
    /// Grid spacing to snap to, and the snap radius, in drawing units.
    pub step: f64,
    pub radius: f64,
    pub ortho: Ortho,
    pub base: Option<Point>,
    /// The prompt's base point, the last point Perpendicular and Tangent
    /// work from (ortho or not).
    pub from: Option<Point>,
    /// The snap point the user cycled to, if it is still under the pointer.
    pub chosen: Option<SnapId>,
    /// A one-shot override is waiting: a point with no snap is not placed.
    pub insist: bool,
    /// What the pointer has paused over and kept, as paths.
    pub tracks: Vec<crate::snap::Track>,
    /// The paths of object snap tracking and polar tracking.
    pub track: Option<crate::snap::TrackInput>,
    /// Stand-in objects for what a command has drawn but not yet added.
    pub extra: std::sync::Arc<[od_core::Object]>,
}

impl Aim<'_> {
    /// Every snap point under the pointer at `at`, best first.
    pub fn found(&self, at: Point) -> Vec<SnapPoint> {
        let near = self.document.visible_here_in_rect(probe(at, self.radius));
        let probe = Probe {
            cursor: at,
            tolerance: self.radius,
            grid_step: Some(self.step),
            modes: &self.modes,
            from: self.from,
            tracks: &self.tracks,
            track: self.track.as_ref(),
        };
        snap_all(near.into_iter().chain(self.extra.iter()), &probe)
    }

    /// Where the pointer at `at` ends up, and the snap behind it.
    pub fn resolve(&self, at: Point) -> (Point, Option<SnapPoint>) {
        self.settle(at, &self.found(at))
    }

    /// Where a click at `at` places a point; none when an override insists
    /// on a snap and there is none.
    pub fn place(&self, at: Point) -> Option<(Point, Option<SnapPoint>)> {
        let (point, snapped) = self.resolve(at);
        (snapped.is_some() || !self.insist).then_some((point, snapped))
    }

    /// [`resolve`](Self::resolve) from the points [`found`](Self::found)
    /// there: the one the user cycled to, or else the best.
    pub fn settle(&self, at: Point, found: &[SnapPoint]) -> (Point, Option<SnapPoint>) {
        let snapped = self
            .chosen
            .and_then(|id| found.iter().find(|f| f.id() == id))
            .or_else(|| found.first())
            .copied();
        match self.base {
            Some(base) => hold(self.ortho, base, at, snapped),
            None => (snapped.map_or(at, |s| s.point), snapped),
        }
    }
}

/// Where the pointer at `cursor`, snapped to `found`, ends up when ortho
/// holds it to the axis from `base` that the cursor is nearer to. An object
/// snap wins in the standard lock; in the strict lock its point is projected
/// onto the axis, and the snap is still reported where it was found. A grid
/// snap is always held to the axis, and reported there.
pub(crate) fn hold(
    ortho: Ortho,
    base: Point,
    cursor: Point,
    found: Option<SnapPoint>,
) -> (Point, Option<SnapPoint>) {
    let axis = axis_of(base, cursor);
    match found {
        Some(s) if s.kind != SnapKind::Grid && !ortho.strict => (s.point, found),
        Some(s) if s.kind != SnapKind::Grid => (on_axis(base, s.point, axis), found),
        Some(s) => {
            let point = on_axis(base, s.point, axis);
            (point, Some(SnapPoint { point, ..s }))
        }
        None => (on_axis(base, cursor, axis), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn at(point: Point, kind: SnapKind) -> Option<SnapPoint> {
        Some(SnapPoint::new(point, kind))
    }

    const STANDARD: Ortho = Ortho {
        enabled: true,
        strict: false,
    };
    const STRICT: Ortho = Ortho {
        enabled: true,
        strict: true,
    };

    #[test]
    fn standard_lets_an_object_snap_leave_the_axis() {
        let endpoint = at(p(50.0, 8.0), SnapKind::Endpoint);
        let held = hold(STANDARD, p(0.0, 0.0), p(49.0, 7.0), endpoint);
        assert_eq!(held, (p(50.0, 8.0), endpoint));
    }

    #[test]
    fn strict_projects_an_object_snap_onto_the_axis_and_keeps_its_marker() {
        let endpoint = at(p(50.0, 8.0), SnapKind::Endpoint);
        let held = hold(STRICT, p(0.0, 0.0), p(49.0, 7.0), endpoint);
        assert_eq!(held, (p(50.0, 0.0), endpoint));
        let upright = hold(STRICT, p(0.0, 0.0), p(3.0, 40.0), endpoint);
        assert_eq!(upright.0, p(0.0, 8.0), "the cursor chooses the axis");
    }

    #[test]
    fn a_grid_snap_is_held_to_the_axis_in_both_locks() {
        let grid = at(p(50.0, 10.0), SnapKind::Grid);
        let moved = at(p(50.0, 3.0), SnapKind::Grid);
        for ortho in [STANDARD, STRICT] {
            let held = hold(ortho, p(0.0, 3.0), p(49.0, 9.0), grid);
            assert_eq!(held, (p(50.0, 3.0), moved));
        }
    }

    #[test]
    fn with_no_snap_the_cursor_is_held_to_the_axis() {
        for ortho in [STANDARD, STRICT] {
            let held = hold(ortho, p(0.0, 3.0), p(49.0, 9.0), None);
            assert_eq!(held, (p(49.0, 3.0), None));
        }
    }

    fn aimed(document: &Document, over: Option<crate::SnapOverride>) -> Aim<'_> {
        let running = SnapModes::default();
        Aim {
            document,
            modes: over.as_ref().map_or(running, |o| o.applied(&running)),
            step: 10.0,
            radius: 2.0,
            ortho: Ortho::default(),
            base: None,
            from: None,
            chosen: None,
            insist: over.as_ref().is_some_and(crate::SnapOverride::insists),
            tracks: Vec::new(),
            track: None,
            extra: Default::default(),
        }
    }

    fn placed(document: &Document, over: Option<crate::SnapOverride>, at: Point) -> Option<Point> {
        aimed(document, over).place(at).map(|(p, _)| p)
    }

    #[test]
    fn extra_geometry_is_snapped_to_like_the_drawing() {
        let doc = Document::default();
        let mut aim = aimed(&doc, None);
        let none = aim.place(p(5.0, 0.3)).map(|(at, _)| at);
        assert_ne!(none, Some(p(5.0, 0.0)), "nothing there yet but the grid");
        let id = od_core::EntityId(u64::MAX);
        let attrs = od_core::Attributes::on_layer(od_core::LayerId::DEFAULT);
        let line = od_core::Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        };
        aim.extra = vec![od_core::Object::new(id, line, attrs, None)].into();
        let found = aim.found(p(5.0, 0.3));
        assert!(found
            .iter()
            .any(|s| s.kind == SnapKind::Midpoint && s.point == p(5.0, 0.0)));
    }

    #[test]
    fn an_override_that_finds_nothing_places_no_point() {
        let doc = Document::from_entities([od_core::Entity::Line {
            start: p(0.0, 0.0),
            end: p(100.0, 0.0),
        }]);
        let centre = Some(crate::SnapOverride::Only(vec![SnapKind::Center]));
        // Running snaps would give the endpoint here, and the grid elsewhere.
        assert_eq!(placed(&doc, None, p(0.5, 0.5)), Some(p(0.0, 0.0)));
        assert_eq!(placed(&doc, centre.clone(), p(0.5, 0.5)), None);
        assert_eq!(placed(&doc, centre, p(53.0, 27.0)), None);
        // `NON` places the point, on the grid, with no object snap.
        let none = Some(crate::SnapOverride::None);
        assert_eq!(placed(&doc, none, p(0.5, 0.5)), Some(p(0.0, 0.0)));
        // An override that finds its snap places it, though running snaps are off.
        let end = Some(crate::SnapOverride::Only(vec![SnapKind::Endpoint]));
        assert_eq!(placed(&doc, end, p(99.5, 0.5)), Some(p(100.0, 0.0)));
    }
}
