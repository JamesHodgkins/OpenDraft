//! Object snap tracking and polar tracking: alignment paths (infinite lines)
//! through the points the pointer has kept and through the command's last
//! point, and the points on them. Pure geometry; what is kept lives in the
//! view (`view/acquire.rs`).
//!
//! A path gives a snap point where the pointer is within reach of it
//! ([`REACH`] of the snap aperture) and not within the aperture of where it
//! starts (that point's own snap is there). Two paths from different points
//! give their crossing, while the pointer is within reach of both and the
//! crossing is on screen.

use od_core::{Bounds, Point};
use serde::{Deserialize, Serialize};

use super::{Class, Ranked, SnapKind, SnapPoint};

/// How far from a path the pointer still follows it, as a share of the snap
/// aperture. The spec leaves this to the application.
pub const REACH: f64 = 0.75;

/// A snap point the pointer rested on and kept as the start of paths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Origin {
    pub at: Point,
    pub kind: SnapKind,
}

/// A path a snap point was aligned on: through `origin`, along `dir` (a unit
/// vector). `kind` is the snap point the path starts from, or `None` for a
/// polar path from the command's last point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub origin: Point,
    pub dir: Point,
    pub kind: Option<SnapKind>,
}

impl Guide {
    /// How far `p` is from the path.
    fn off(&self, p: Point) -> f64 {
        ((p.x - self.origin.x) * self.dir.y - (p.y - self.origin.y) * self.dir.x).abs()
    }

    /// How far along the path `p` projects, from its origin.
    fn along(&self, p: Point) -> f64 {
        (p.x - self.origin.x) * self.dir.x + (p.y - self.origin.y) * self.dir.y
    }
}

/// The tracking settings: `AUTOSNAP` bits 16 and 8, `POLARANG`,
/// `POLARADDANG`, `POLARMODE` and `TRACKPATH`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tracking {
    /// Object snap tracking (F11).
    pub otrack: bool,
    /// Polar tracking from the command's last point (F10). Not with ortho.
    pub polar: bool,
    /// Degrees between polar paths (`POLARANG`).
    pub increment: f64,
    /// Further absolute polar angles in degrees (`POLARADDANG`).
    pub additional: Vec<f64>,
    /// Use the additional angles (`POLARMODE` bit 4).
    pub use_additional: bool,
    /// Object snap tracking follows the polar angles, not only the
    /// horizontal and vertical (`POLARMODE` bit 2).
    pub all_polar: bool,
    /// Keep points only while Shift is held, instead of unless it is
    /// (`POLARMODE` bit 8).
    pub shift_to_acquire: bool,
    /// Which paths are drawn full-screen (`TRACKPATH` 0 to 3).
    pub path_display: u8,
    /// Name the point and its distance and angle beside tracking and polar
    /// points (`AUTOSNAP` bit 32).
    pub tooltips: bool,
}

impl Default for Tracking {
    fn default() -> Self {
        Self {
            otrack: true,
            polar: false,
            increment: 90.0,
            additional: Vec::new(),
            use_additional: false,
            all_polar: false,
            shift_to_acquire: false,
            path_display: 0,
            tooltips: true,
        }
    }
}

impl Tracking {
    /// The path directions of polar tracking, in degrees from 0 up to (not
    /// including) 180: a path is a whole line, so a direction and its
    /// opposite are one.
    pub fn polar_angles(&self) -> Vec<f64> {
        let step = if self.increment.is_finite() && self.increment > 0.0 {
            self.increment
        } else {
            90.0
        };
        let turns = (360.0 / step).ceil().clamp(1.0, 720.0) as u32;
        let mut angles: Vec<f64> = (0..turns).map(|k| f64::from(k) * step).collect();
        if self.use_additional {
            angles.extend(&self.additional);
        }
        let mut lines: Vec<f64> = Vec::new();
        for a in angles {
            let a = a.rem_euclid(180.0);
            if lines.iter().all(|l| (l - a).abs() > 1e-9) {
                lines.push(a);
            }
        }
        lines
    }

    /// The directions object snap tracking follows from kept points.
    pub fn track_angles(&self) -> Vec<f64> {
        if self.all_polar {
            self.polar_angles()
        } else {
            vec![0.0, 90.0]
        }
    }

    /// Whether the full path (not just from its start to the pointer) is
    /// drawn, for a polar path or an object snap tracking path.
    pub fn full_path(&self, polar: bool) -> bool {
        matches!((polar, self.path_display), (true, 0 | 1) | (false, 0 | 2))
    }

    /// Whether the path is drawn at all (`TRACKPATH` 2 and 3 hide polar ones).
    pub fn shows(&self, polar: bool) -> bool {
        !(polar && self.path_display >= 2)
    }
}

/// Unit vectors of lines at these angles in degrees. Horizontal and vertical
/// are exact, so a point on them has exactly the coordinate of its origin.
pub fn directions(angles: &[f64]) -> Vec<Point> {
    angles
        .iter()
        .map(|a| match a.rem_euclid(360.0) {
            r if r == 0.0 || r == 180.0 => Point::new(1.0, 0.0),
            r if r == 90.0 || r == 270.0 => Point::new(0.0, 1.0),
            r => {
                let (s, c) = r.to_radians().sin_cos();
                Point::new(c, s)
            }
        })
        .collect()
}

/// What the paths are made from, for one pointer position.
#[derive(Clone, Debug)]
pub struct TrackInput {
    /// Points kept (object snap tracking).
    pub origins: Vec<Origin>,
    /// The command's last point, with polar tracking on.
    pub base: Option<Point>,
    /// Directions followed from kept points, and from the last point.
    pub track_dirs: Vec<Point>,
    pub polar_dirs: Vec<Point>,
    /// The part of the drawing on screen: a crossing off it is not offered.
    pub view: Bounds,
}

fn contains(b: &Bounds, p: Point) -> bool {
    p.x >= b.min.x && p.x <= b.max.x && p.y >= b.min.y && p.y <= b.max.y
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The snap points on the paths: where the pointer is aligned with one path,
/// and where it is aligned with two.
pub(super) fn candidates(input: &TrackInput, cursor: Point, tolerance: f64) -> Vec<Ranked> {
    let reach = tolerance * REACH;
    let mut near: Vec<Guide> = Vec::new();
    for o in &input.origins {
        for d in &input.track_dirs {
            near.push(Guide {
                origin: o.at,
                dir: *d,
                kind: Some(o.kind),
            });
        }
    }
    if let Some(base) = input.base {
        for d in &input.polar_dirs {
            near.push(Guide {
                origin: base,
                dir: *d,
                kind: None,
            });
        }
    }
    near.retain(|g| g.off(cursor) <= reach);

    let mut found = Vec::new();
    for g in &near {
        let t = g.along(cursor);
        if t.abs() < tolerance {
            continue;
        }
        let point = Point::new(g.origin.x + g.dir.x * t, g.origin.y + g.dir.y * t);
        let (kind, class) = match g.kind {
            Some(_) => (SnapKind::Tracking, Class::Track),
            None => (SnapKind::Polar, Class::Polar),
        };
        let mut snap = SnapPoint::new(point, kind);
        snap.guides = [Some(*g), None];
        found.push(Ranked {
            snap,
            dist: distance(point, cursor),
            class,
        });
    }
    for (i, a) in near.iter().enumerate() {
        for b in &near[i + 1..] {
            if distance(a.origin, b.origin) < 1e-9 {
                continue;
            }
            let cross = a.dir.x * b.dir.y - a.dir.y * b.dir.x;
            if cross.abs() < 1e-3 {
                continue;
            }
            let (dx, dy) = (b.origin.x - a.origin.x, b.origin.y - a.origin.y);
            let s = (dx * b.dir.y - dy * b.dir.x) / cross;
            let point = Point::new(a.origin.x + a.dir.x * s, a.origin.y + a.dir.y * s);
            if !contains(&input.view, point) {
                continue;
            }
            let mut snap = SnapPoint::new(point, SnapKind::Tracking);
            snap.guides = [Some(*a), Some(*b)];
            found.push(Ranked {
                snap,
                dist: distance(point, cursor),
                class: Class::TrackCross,
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn view() -> Bounds {
        Bounds::from_points(p(-1000.0, -1000.0), p(1000.0, 1000.0))
    }

    fn input(origins: &[(f64, f64)], base: Option<Point>, tracking: &Tracking) -> TrackInput {
        TrackInput {
            origins: origins
                .iter()
                .map(|(x, y)| Origin {
                    at: p(*x, *y),
                    kind: SnapKind::Endpoint,
                })
                .collect(),
            base,
            track_dirs: directions(&tracking.track_angles()),
            polar_dirs: directions(&tracking.polar_angles()),
            view: view(),
        }
    }

    #[test]
    fn polar_angles_repeat_round_the_turn_and_are_lines() {
        let t = Tracking::default();
        assert_eq!(t.polar_angles(), [0.0, 90.0]);
        let t = Tracking {
            increment: 45.0,
            ..Tracking::default()
        };
        assert_eq!(t.polar_angles(), [0.0, 45.0, 90.0, 135.0]);
        let t = Tracking {
            increment: 25.0,
            ..Tracking::default()
        };
        // 25, 50, ... 350 as directions are these lines.
        assert_eq!(t.polar_angles().len(), 15);
        assert!(t.polar_angles().contains(&20.0));
    }

    #[test]
    fn extra_angles_need_their_option_and_a_bad_step_falls_back() {
        let mut t = Tracking {
            additional: vec![37.5, 217.5],
            ..Tracking::default()
        };
        assert_eq!(t.polar_angles(), [0.0, 90.0]);
        t.use_additional = true;
        assert_eq!(
            t.polar_angles(),
            [0.0, 90.0, 37.5],
            "an opposite is the same line"
        );
        t.increment = f64::NAN;
        assert_eq!(t.polar_angles(), [0.0, 90.0, 37.5]);
    }

    #[test]
    fn tracking_follows_only_the_axes_unless_told_to_use_the_polar_angles() {
        let t = Tracking {
            increment: 30.0,
            ..Tracking::default()
        };
        assert_eq!(t.track_angles(), [0.0, 90.0]);
        let t = Tracking {
            all_polar: true,
            ..t
        };
        assert_eq!(t.track_angles().len(), 6);
    }

    #[test]
    fn trackpath_decides_which_paths_are_full_and_which_hidden() {
        let at = |n| Tracking {
            path_display: n,
            ..Tracking::default()
        };
        // (polar full, polar shown, otrack full)
        let row = |t: &Tracking| (t.full_path(true), t.shows(true), t.full_path(false));
        assert_eq!(row(&at(0)), (true, true, true));
        assert_eq!(row(&at(1)), (true, true, false));
        assert_eq!(row(&at(2)), (false, false, true));
        assert_eq!(row(&at(3)), (false, false, false));
    }

    #[test]
    fn a_kept_point_gives_its_horizontal_and_vertical() {
        let t = Tracking::default();
        let i = input(&[(10.0, 10.0)], None, &t);
        let found = candidates(&i, p(60.0, 10.5), 4.0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].snap.point, p(60.0, 10.0));
        assert_eq!(found[0].snap.kind, SnapKind::Tracking);
        assert_eq!(found[0].class, Class::Track);
        let up = candidates(&i, p(9.5, -40.0), 4.0);
        assert_eq!(up[0].snap.point, p(10.0, -40.0));
        // Off every path, or on the path but at the point itself: nothing.
        assert!(candidates(&i, p(60.0, 20.0), 4.0).is_empty());
        assert!(candidates(&i, p(12.0, 10.0), 4.0).is_empty());
    }

    #[test]
    fn two_points_give_the_crossing_of_their_paths_once() {
        let t = Tracking::default();
        let i = input(&[(10.0, 50.0), (80.0, 5.0)], None, &t);
        let found = candidates(&i, p(79.0, 51.0), 4.0);
        let crossings: Vec<_> = found
            .iter()
            .filter(|r| r.class == Class::TrackCross)
            .collect();
        assert_eq!(crossings.len(), 1, "{found:?}");
        assert_eq!(crossings[0].snap.point, p(80.0, 50.0));
        assert!(crossings[0].snap.guides.iter().all(Option::is_some));
        // Within reach of one path only: no crossing.
        let one = candidates(&i, p(50.0, 51.0), 4.0);
        assert!(one.iter().all(|r| r.class != Class::TrackCross));
    }

    #[test]
    fn parallel_paths_and_paths_from_one_point_have_no_crossing() {
        let t = Tracking::default();
        // Two horizontals at the same height, from different points.
        let same_row = input(&[(10.0, 50.0), (80.0, 50.0)], None, &t);
        let found = candidates(&same_row, p(45.0, 50.5), 4.0);
        assert!(found.iter().all(|r| r.class != Class::TrackCross));
        // The horizontal and vertical of one point cross at that point.
        let one = input(&[(10.0, 10.0)], None, &t);
        let found = candidates(&one, p(10.5, 10.5), 4.0);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_crossing_off_screen_is_not_offered() {
        let t = Tracking::default();
        let mut i = input(&[(10.0, 50.0), (80.0, 5.0)], None, &t);
        i.view = Bounds::from_points(p(0.0, 0.0), p(70.0, 70.0));
        let found = candidates(&i, p(79.0, 51.0), 4.0);
        assert!(found.iter().all(|r| r.class != Class::TrackCross));
    }

    #[test]
    fn polar_paths_from_the_last_point_rank_below_object_tracking() {
        let t = Tracking {
            polar: true,
            increment: 45.0,
            ..Tracking::default()
        };
        let i = input(&[], Some(p(0.0, 0.0)), &t);
        let found = candidates(&i, p(50.0, 51.0), 4.0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].snap.kind, SnapKind::Polar);
        assert_eq!(found[0].class, Class::Polar);
        assert!((found[0].snap.point.x - 50.5).abs() < 1e-9);
        assert!(found[0].snap.guides[0].unwrap().kind.is_none());
    }

    #[test]
    fn a_polar_path_crosses_the_path_of_a_kept_point() {
        let t = Tracking {
            polar: true,
            increment: 45.0,
            ..Tracking::default()
        };
        // 45 degrees from the origin meets the vertical through (30, 90).
        let i = input(&[(30.0, 90.0)], Some(p(0.0, 0.0)), &t);
        let found = candidates(&i, p(31.0, 29.0), 4.0);
        let cross = found
            .iter()
            .find(|r| r.class == Class::TrackCross)
            .expect("the crossing");
        assert!((cross.snap.point.x - 30.0).abs() < 1e-9);
        assert!((cross.snap.point.y - 30.0).abs() < 1e-9);
    }
}
