//! Snapping: pull the cursor onto geometry (endpoints, midpoints, centres,
//! nodes, quadrants, intersections, insertion points, perpendiculars,
//! tangents, extensions, parallels, the nearest point of a curve) or the grid. Pure functions in
//! world space; the screen aperture arrives as a radius in drawing units.
//!
//! One pass gathers every candidate under the pointer ([`snap_all`]), ranks
//! them best first, and the view shows the first (or the one the user
//! cycled to with Tab). Ranking is our policy, not AutoCAD's: a point inside
//! the aperture beats Nearest, which beats the grid; within a class the closer
//! to the pointer wins, and exact ties go to the more specific mode.

use od_core::{Bounds, EntityId, Object, Point};

mod context;
mod cycle;
#[cfg(test)]
mod engine_tests;
mod fixed;
mod otrack;
mod overrides;
#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod tests;
mod track;

pub(crate) use cycle::cycled;
pub(crate) use fixed::object_candidates;
pub use fixed::{candidates, geometric_center};
pub use otrack::{directions, Guide, Origin, TrackInput, Tracking};

pub use overrides::SnapOverride;
pub use track::{resolve, targets, Acquired, Geom, Track};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SnapKind {
    Endpoint,
    Midpoint,
    Center,
    /// A dimension's definition point.
    Node,
    /// The ends of a circle's, arc's or ellipse's axes.
    Quadrant,
    /// Where two curves cross.
    Intersection,
    /// Where two curves cross as drawn (in 2D the same as Intersection).
    ApparentIntersection,
    /// Where the extension of an end the pointer paused on meets another
    /// object. Only as an override.
    ExtendedIntersection,
    /// The centre of the area a closed polyline or spline encloses.
    GeometricCenter,
    /// The insertion point of a block reference or text.
    Insertion,
    /// The foot of the perpendicular from the command's last point.
    Perpendicular,
    /// Where a line from the command's last point touches a curve.
    Tangent,
    /// The continuation of a line or arc past an end the pointer paused on.
    Extension,
    /// The line through the command's last point parallel to a segment the
    /// pointer paused on.
    Parallel,
    /// The point of an object nearest the cursor.
    Nearest,
    Grid,
    /// Aligned with a path through a point the pointer kept.
    Tracking,
    /// Aligned with a polar path from the command's last point.
    Polar,
}

impl SnapKind {
    /// Every kind that can be switched on or off (all but the grid).
    pub const OBJECT: [SnapKind; 15] = [
        SnapKind::Endpoint,
        SnapKind::Midpoint,
        SnapKind::Center,
        SnapKind::Node,
        SnapKind::Quadrant,
        SnapKind::Intersection,
        SnapKind::ApparentIntersection,
        SnapKind::ExtendedIntersection,
        SnapKind::GeometricCenter,
        SnapKind::Insertion,
        SnapKind::Perpendicular,
        SnapKind::Tangent,
        SnapKind::Extension,
        SnapKind::Parallel,
        SnapKind::Nearest,
    ];

    /// The tooltip text, as AutoCAD words it.
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "Endpoint",
            SnapKind::Midpoint => "Midpoint",
            SnapKind::Center => "Center",
            SnapKind::Node => "Node",
            SnapKind::Quadrant => "Quadrant",
            SnapKind::Intersection => "Intersection",
            SnapKind::ApparentIntersection => "Apparent Intersection",
            SnapKind::ExtendedIntersection => "Extended Intersection",
            SnapKind::GeometricCenter => "Geometric Center",
            SnapKind::Insertion => "Insertion",
            SnapKind::Perpendicular => "Perpendicular",
            SnapKind::Tangent => "Tangent",
            SnapKind::Extension => "Extension",
            SnapKind::Parallel => "Parallel",
            SnapKind::Nearest => "Nearest",
            SnapKind::Grid => "Grid",
            SnapKind::Tracking => "Tracking",
            SnapKind::Polar => "Polar",
        }
    }

    /// Which snap wins when two are as close to the pointer as each other
    /// (lower first).
    fn priority(self) -> u8 {
        match self {
            SnapKind::Endpoint => 0,
            SnapKind::Insertion => 1,
            SnapKind::Node => 2,
            SnapKind::Intersection => 3,
            SnapKind::ApparentIntersection => 4,
            SnapKind::ExtendedIntersection => 5,
            SnapKind::Center => 6,
            SnapKind::GeometricCenter => 7,
            SnapKind::Quadrant => 8,
            SnapKind::Midpoint => 9,
            SnapKind::Extension => 10,
            SnapKind::Perpendicular => 11,
            SnapKind::Tangent => 12,
            SnapKind::Parallel => 13,
            SnapKind::Nearest => 14,
            SnapKind::Grid => 15,
            SnapKind::Tracking => 16,
            SnapKind::Polar => 17,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapPoint {
    pub point: Point,
    pub kind: SnapKind,
    /// The objects it was found on (two for an intersection).
    pub source: [Option<EntityId>; 2],
    /// Perpendicular or Tangent with no last point yet: the point is only
    /// where the object is, and the command works out the real one from the
    /// next point it is given.
    pub deferred: bool,
    /// The paths a tracking or polar point was aligned on.
    pub guides: [Option<Guide>; 2],
}

/// What identifies a snap point from one frame to the next, so a point the
/// user cycled to stays chosen while the pointer drifts: its kind, its
/// objects and where it is (to a millionth of a unit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SnapId {
    kind: SnapKind,
    source: [Option<EntityId>; 2],
    at: (i64, i64),
}

impl SnapId {
    pub fn kind(&self) -> SnapKind {
        self.kind
    }

    /// The objects the snap point was found on.
    pub fn sources(&self) -> [Option<EntityId>; 2] {
        self.source
    }
}

impl SnapPoint {
    pub fn new(point: Point, kind: SnapKind) -> Self {
        Self {
            point,
            kind,
            source: [None, None],
            deferred: false,
            guides: [None, None],
        }
    }

    /// The tooltip: the kind's name, with "Deferred" before it when deferred.
    pub fn label(&self) -> String {
        if let [Some(first), second] = self.guides {
            let part = |g: &Guide| {
                let name = g.kind.map_or("Polar", SnapKind::label);
                let (dx, dy) = (self.point.x - g.origin.x, self.point.y - g.origin.y);
                let angle = dy.atan2(dx).to_degrees().rem_euclid(360.0);
                format!("{name}: {:.4} < {angle:.0}\u{b0}", dx.hypot(dy))
            };
            return match second {
                Some(second) => format!("{} | {}", part(&first), part(&second)),
                None => part(&first),
            };
        }
        match self.deferred {
            true => format!("Deferred {}", self.kind.label()),
            false => self.kind.label().to_owned(),
        }
    }

    /// The same point, found on `object`.
    pub(crate) fn from(mut self, object: EntityId) -> Self {
        self.source[0] = Some(object);
        self
    }

    pub fn id(&self) -> SnapId {
        let micro = |v: f64| (v * 1e6).round() as i64;
        SnapId {
            kind: self.kind,
            source: self.source,
            at: (micro(self.point.x), micro(self.point.y)),
        }
    }
}

/// Which snaps are active. `enabled` is the master switch (`OSMODE` bit
/// 16384). The first block of fields are the running object snaps, set as
/// in AutoCAD's `OSMODE`; `marker`, `tooltip` and `magnet` are `AUTOSNAP`
/// bits 1, 2 and 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SnapModes {
    pub enabled: bool,
    pub endpoint: bool,
    pub midpoint: bool,
    pub center: bool,
    pub node: bool,
    pub quadrant: bool,
    pub intersection: bool,
    pub apparent_intersection: bool,
    /// Only ever set by a one-shot override (`SnapOverride`), never running.
    pub extended_intersection: bool,
    pub geometric_center: bool,
    pub insertion: bool,
    pub perpendicular: bool,
    pub tangent: bool,
    pub extension: bool,
    pub parallel: bool,
    pub nearest: bool,
    pub grid: bool,
    /// Draw the marker of the snap in force.
    pub marker: bool,
    /// Name the snap in force beside it.
    pub tooltip: bool,
    /// Draw the crosshair on the snap point, not under the pointer.
    pub magnet: bool,
    /// Draw the aperture box around the pointer (`APBOX`).
    pub aperture_box: bool,
}

impl Default for SnapModes {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: true,
            midpoint: true,
            center: true,
            node: false,
            quadrant: false,
            intersection: true,
            apparent_intersection: false,
            extended_intersection: false,
            geometric_center: false,
            insertion: false,
            perpendicular: false,
            tangent: false,
            extension: true,
            parallel: false,
            nearest: false,
            grid: true,
            marker: true,
            tooltip: true,
            magnet: true,
            aperture_box: false,
        }
    }
}

impl SnapModes {
    pub fn allows(&self, kind: SnapKind) -> bool {
        match kind {
            SnapKind::Endpoint => self.endpoint,
            SnapKind::Midpoint => self.midpoint,
            SnapKind::Center => self.center,
            SnapKind::Node => self.node,
            SnapKind::Quadrant => self.quadrant,
            SnapKind::Intersection => self.intersection,
            SnapKind::ApparentIntersection => self.apparent_intersection,
            SnapKind::ExtendedIntersection => self.extended_intersection,
            SnapKind::GeometricCenter => self.geometric_center,
            SnapKind::Insertion => self.insertion,
            SnapKind::Perpendicular => self.perpendicular,
            SnapKind::Tangent => self.tangent,
            SnapKind::Extension => self.extension,
            SnapKind::Parallel => self.parallel,
            SnapKind::Nearest => self.nearest,
            SnapKind::Grid => self.grid,
            SnapKind::Tracking | SnapKind::Polar => true,
        }
    }

    pub fn set(&mut self, kind: SnapKind, on: bool) {
        let slot = match kind {
            SnapKind::Endpoint => &mut self.endpoint,
            SnapKind::Midpoint => &mut self.midpoint,
            SnapKind::Center => &mut self.center,
            SnapKind::Node => &mut self.node,
            SnapKind::Quadrant => &mut self.quadrant,
            SnapKind::Intersection => &mut self.intersection,
            SnapKind::ApparentIntersection => &mut self.apparent_intersection,
            SnapKind::ExtendedIntersection => &mut self.extended_intersection,
            SnapKind::GeometricCenter => &mut self.geometric_center,
            SnapKind::Insertion => &mut self.insertion,
            SnapKind::Perpendicular => &mut self.perpendicular,
            SnapKind::Tangent => &mut self.tangent,
            SnapKind::Extension => &mut self.extension,
            SnapKind::Parallel => &mut self.parallel,
            SnapKind::Nearest => &mut self.nearest,
            SnapKind::Grid => &mut self.grid,
            // Not running snaps: tracking has its own switches.
            SnapKind::Tracking | SnapKind::Polar => return,
        };
        *slot = on;
    }
}

/// How a candidate ranks against the others (best first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Class {
    /// A point inside the aperture.
    Point,
    /// Where two tracking paths cross.
    TrackCross,
    /// On a path through a point the pointer kept.
    Track,
    Nearest,
    /// On a polar path from the command's last point.
    Polar,
    Grid,
}

/// A candidate with its distance from the pointer and its class.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ranked {
    pub snap: SnapPoint,
    pub dist: f64,
    pub class: Class,
}

/// What to snap: the pointer and aperture, the snaps in force, and the
/// command's last point, which Perpendicular and Tangent need.
pub struct Probe<'a> {
    pub cursor: Point,
    pub tolerance: f64,
    pub grid_step: Option<f64>,
    pub modes: &'a SnapModes,
    pub from: Option<Point>,
    /// What the pointer has paused over and kept (Extension and Parallel).
    pub tracks: &'a [Track],
    /// The tracking and polar paths, when tracking is on.
    pub track: Option<&'a TrackInput>,
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Every snap point under the pointer, best first (see the module notes). A
/// point several modes find is listed once, as the most specific.
pub fn snap_all<'a>(
    objects: impl IntoIterator<Item = &'a Object>,
    probe: &Probe<'_>,
) -> Vec<SnapPoint> {
    let modes = probe.modes;
    if !modes.enabled {
        return Vec::new();
    }
    let (cursor, tolerance) = (probe.cursor, probe.tolerance);
    // Entities whose bounds miss the capture box cannot offer a point in
    // range, so skip them before building their candidate lists.
    let capture = Bounds::from_points(
        Point::new(cursor.x - tolerance, cursor.y - tolerance),
        Point::new(cursor.x + tolerance, cursor.y + tolerance),
    );
    let near: Vec<&Object> = objects
        .into_iter()
        .filter(|o| o.bounds().intersects(&capture))
        .collect();
    let mut found: Vec<Ranked> = near
        .iter()
        .flat_map(|o| object_candidates(o))
        .filter(|c| modes.allows(c.kind))
        .map(|c| Ranked {
            snap: c,
            dist: distance(c.point, cursor),
            class: Class::Point,
        })
        .filter(|r| r.dist <= tolerance)
        .collect();
    if modes.geometric_center {
        found.extend(near.iter().filter_map(|o| {
            let centre = fixed::geometric_center(&o.entity)?;
            let dist = distance(centre, cursor);
            (dist <= tolerance).then(|| Ranked {
                snap: SnapPoint::new(centre, SnapKind::GeometricCenter).from(o.id),
                dist,
                class: Class::Point,
            })
        }));
    }
    let wants_pieces = modes.intersection
        || modes.apparent_intersection
        || modes.extended_intersection
        || modes.perpendicular
        || modes.tangent;
    if wants_pieces {
        let scene = context::Scene::new(&near);
        // With no true crossing in 2D, apparent crossings are the same points.
        let crossing = match (modes.intersection, modes.apparent_intersection) {
            (true, _) => Some(SnapKind::Intersection),
            (false, true) => Some(SnapKind::ApparentIntersection),
            _ => None,
        };
        if let Some(kind) = crossing {
            found.extend(context::intersections(
                &scene, &capture, cursor, tolerance, kind,
            ));
        }
        if modes.extended_intersection {
            found.extend(context::extended(
                &scene,
                &capture,
                probe.tracks,
                cursor,
                tolerance,
            ));
        }
        if modes.perpendicular || modes.tangent {
            let want = (modes.perpendicular, modes.tangent);
            found.extend(context::from_point(
                &scene, &capture, cursor, tolerance, probe.from, want,
            ));
        }
    }
    if modes.extension || modes.parallel {
        found.extend(track::candidates(
            probe.tracks,
            cursor,
            tolerance,
            probe.from,
            modes,
        ));
    }
    if let Some(track) = probe.track {
        found.extend(otrack::candidates(track, cursor, tolerance));
    }
    if modes.nearest {
        for object in &near {
            for point in object_nearest_points(object, cursor) {
                let dist = distance(point, cursor);
                if dist <= tolerance {
                    found.push(Ranked {
                        snap: SnapPoint::new(point, SnapKind::Nearest).from(object.id),
                        dist,
                        class: Class::Nearest,
                    });
                }
            }
        }
    }
    if let Some(step) = probe
        .grid_step
        .filter(|s| modes.grid && s.is_finite() && *s > 0.0)
    {
        let point = Point::new(
            (cursor.x / step).round() * step,
            (cursor.y / step).round() * step,
        );
        found.push(Ranked {
            snap: SnapPoint::new(point, SnapKind::Grid),
            dist: distance(point, cursor),
            class: Class::Grid,
        });
    }
    rank(found, tolerance)
}

/// Sort best first and list each place once.
fn rank(mut found: Vec<Ranked>, tolerance: f64) -> Vec<SnapPoint> {
    found.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then(a.dist.total_cmp(&b.dist))
            .then(a.snap.kind.priority().cmp(&b.snap.kind.priority()))
            .then(a.snap.source.cmp(&b.snap.source))
            .then(a.snap.point.x.total_cmp(&b.snap.point.x))
            .then(a.snap.point.y.total_cmp(&b.snap.point.y))
    });
    let same = tolerance * 1e-6;
    let mut kept: Vec<SnapPoint> = Vec::with_capacity(found.len());
    for r in found {
        if !kept.iter().any(|k| distance(k.point, r.snap.point) <= same) {
            kept.push(r.snap);
        }
    }
    kept
}

/// Where `cursor` should snap, if anywhere: the first of [`snap_all`] with no
/// last point. Object snaps within `tolerance` win; then, with Nearest on,
/// the point of an object nearest the cursor (AutoCAD lets the other snaps
/// win over it); otherwise the cursor snaps to the nearest grid point when
/// `grid_step` is given.
pub fn snap<'a>(
    objects: impl IntoIterator<Item = &'a Object>,
    cursor: Point,
    tolerance: f64,
    grid_step: Option<f64>,
    modes: &SnapModes,
) -> Option<SnapPoint> {
    let probe = Probe {
        cursor,
        tolerance,
        grid_step,
        modes,
        from: None,
        tracks: &[],
        track: None,
    };
    snap_all(objects, &probe).into_iter().next()
}

/// The nearest point of each piece of a stored object: a dimension uses the
/// picture the object remembers.
fn object_nearest_points(object: &Object, cursor: Point) -> Vec<Point> {
    match object.dimension_picture() {
        Some(picture) => picture
            .iter()
            .flat_map(|e| nearest_points(e, cursor))
            .collect(),
        None => nearest_points(&object.entity, cursor),
    }
}

/// The points of `entity` nearest `cursor`: its outline's, or for a block
/// reference or a dimension, each piece's.
fn nearest_points(entity: &od_core::Entity, cursor: Point) -> Vec<Point> {
    use od_core::Entity;
    let of_pieces = |pieces: Vec<Entity>| -> Vec<Point> {
        pieces
            .iter()
            .flat_map(|e| nearest_points(e, cursor))
            .collect()
    };
    match entity {
        Entity::Insert(i) => of_pieces(i.picture()),
        Entity::Dimension(d) => of_pieces(d.picture()),
        other => other.nearest_point(cursor).into_iter().collect(),
    }
}
