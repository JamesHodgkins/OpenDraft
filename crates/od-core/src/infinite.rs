//! Construction lines (XLINE) and rays (RAY): lines with no far end. An
//! `Xline` runs both ways from its base point, a `Ray` one way. Both store the
//! base point and a unit direction; this module is the pure geometry.
//!
//! They have no finite box. [`bounds`] gives the box of everything they can
//! reach (infinite in the directions they run), which the spatial index keeps
//! aside and tests like any other box, and which the drawing's extents skip.
//! Where a finite stand-in is needed (intersections, selection edges) a long
//! [`segment`] serves.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Bounds, Entity, Grip, GripKind, Point};

/// How far along the line [`segment`] reaches from the base point.
pub const REACH: f64 = 1e7;

/// How far from the base point the direction grips sit, in screen pixels.
const ARM_PIXELS: f64 = 80.0;

/// The size of a screen pixel in drawing units, as the bits of an `f64`; zero
/// until a view sets it.
static PIXEL: AtomicU64 = AtomicU64::new(0);

/// Tell the grips how big a screen pixel is (drawing units) in the view being
/// edited, so a direction grip can sit a fixed number of pixels from its base
/// point. The canvas sets it every frame; a grip, picked or dragged on any
/// thread, is then where it was drawn.
pub fn set_pixel_size(units_per_pixel: f64) {
    if units_per_pixel.is_finite() && units_per_pixel > 0.0 {
        PIXEL.store(units_per_pixel.to_bits(), Ordering::Relaxed);
    }
}

fn arm() -> f64 {
    let pixel = f64::from_bits(PIXEL.load(Ordering::Relaxed));
    ARM_PIXELS * if pixel > 0.0 { pixel } else { 1.0 }
}

/// A construction line or ray (as `Entity::infinite_line` gives it) cut to
/// `window`, a little widened, as a line that starts on a whole pattern period
/// from the base point, so a linetype lies where it would on the whole line.
/// `None` when it misses the window. `pattern` is the elements and scale.
pub(crate) fn stand_in(
    line: (Point, Point, bool),
    pattern: Option<(&[crate::StrokeElement], f64)>,
    window: &Bounds,
) -> Option<Entity> {
    let (origin, direction, ray) = line;
    let margin = (window.max.x - window.min.x).max(window.max.y - window.min.y) * 0.05;
    let wide = Bounds::from_points(
        Point::new(window.min.x - margin, window.min.y - margin),
        Point::new(window.max.x + margin, window.max.y + margin),
    );
    let (a, b) = clipped(origin, direction, ray, &wide)?;
    let period = pattern.map_or(0.0, |(elements, scale)| {
        elements.iter().map(|e| e.length()).sum::<f64>() * scale
    });
    let start = if period.is_finite() && period > 0.0 {
        let along = (a.x - origin.x) * direction.x + (a.y - origin.y) * direction.y;
        let t = (along / period).floor() * period;
        Point::new(origin.x + direction.x * t, origin.y + direction.y * t)
    } else {
        a
    };
    Some(Entity::Line { start, end: b })
}

/// The unit vector along `direction`, or `None` when it is not a finite
/// vector of some length.
pub fn unit(direction: Point) -> Option<Point> {
    let length = direction.x.hypot(direction.y);
    (length.is_finite() && length > 0.0 && direction.x.is_finite() && direction.y.is_finite())
        .then(|| Point::new(direction.x / length, direction.y / length))
}

/// The direction of angle `radians`.
pub fn from_angle(radians: f64) -> Point {
    Point::new(radians.cos(), radians.sin())
}

/// The box of everything the line can reach.
pub fn bounds(origin: Point, direction: Point, ray: bool) -> Bounds {
    let span = |from: f64, along: f64| {
        if along == 0.0 {
            (from, from)
        } else if !ray {
            (f64::NEG_INFINITY, f64::INFINITY)
        } else if along > 0.0 {
            (from, f64::INFINITY)
        } else {
            (f64::NEG_INFINITY, from)
        }
    };
    let (x0, x1) = span(origin.x, direction.x);
    let (y0, y1) = span(origin.y, direction.y);
    Bounds {
        min: Point::new(x0, y0),
        max: Point::new(x1, y1),
    }
}

/// The part of the line a long way either side of its base point, as two
/// ends: from the base point of a ray, or [`REACH`] behind it for a
/// construction line.
pub fn segment(origin: Point, direction: Point, ray: bool) -> (Point, Point) {
    let at = |t: f64| Point::new(origin.x + direction.x * t, origin.y + direction.y * t);
    (if ray { origin } else { at(-REACH) }, at(REACH))
}

/// How far along the line (from the base point, in the direction) the foot of
/// the perpendicular from `p` falls, kept to the line's extent.
pub fn parameter_of(origin: Point, direction: Point, ray: bool, p: Point) -> f64 {
    let t = (p.x - origin.x) * direction.x + (p.y - origin.y) * direction.y;
    if ray {
        t.max(0.0)
    } else {
        t
    }
}

/// The distance from `p` to the line.
pub fn distance_to(origin: Point, direction: Point, ray: bool, p: Point) -> f64 {
    let t = parameter_of(origin, direction, ray, p);
    let nearest = Point::new(origin.x + direction.x * t, origin.y + direction.y * t);
    (p.x - nearest.x).hypot(p.y - nearest.y)
}

/// The part of the line inside `window`, as its two ends in order along the
/// direction, or `None` when it misses the window.
pub fn clipped(
    origin: Point,
    direction: Point,
    ray: bool,
    window: &Bounds,
) -> Option<(Point, Point)> {
    let (mut near, mut far) = (if ray { 0.0 } else { f64::NEG_INFINITY }, f64::INFINITY);
    let axes = [
        (origin.x, direction.x, window.min.x, window.max.x),
        (origin.y, direction.y, window.min.y, window.max.y),
    ];
    for (from, along, lo, hi) in axes {
        if along == 0.0 {
            if from < lo || from > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - from) / along, (hi - from) / along);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    if near.is_nan() || far.is_nan() || near > far || !near.is_finite() || !far.is_finite() {
        return None;
    }
    let at = |t: f64| Point::new(origin.x + direction.x * t, origin.y + direction.y * t);
    Some((at(near), at(far)))
}

/// The line `distance` to the left of its direction (to the right when
/// negative).
pub fn shifted(origin: Point, direction: Point, distance: f64) -> Point {
    Point::new(
        origin.x - direction.y * distance,
        origin.y + direction.x * distance,
    )
}

/// The direction reflected in the line through the origin along `axis` (a
/// unit vector).
pub fn reflected(direction: Point, axis: Point) -> Point {
    let along = direction.x * axis.x + direction.y * axis.y;
    Point::new(
        2.0 * along * axis.x - direction.x,
        2.0 * along * axis.y - direction.y,
    )
}

/// The direction of the line that halves the angle from `first` to `second`
/// (both directions from the vertex), or `None` when they are opposite and
/// the half is not the one wanted.
pub fn bisector(first: Point, second: Point) -> Option<Point> {
    let (a, b) = (unit(first)?, unit(second)?);
    unit(Point::new(a.x + b.x, a.y + b.y))
}

impl Entity {
    /// The base point, unit direction and whether it is a ray, for a
    /// construction line or ray.
    pub fn infinite_line(&self) -> Option<(Point, Point, bool)> {
        match self {
            Entity::Xline { origin, direction } => Some((*origin, *direction, false)),
            Entity::Ray { origin, direction } => Some((*origin, *direction, true)),
            _ => None,
        }
    }

    /// A construction line or ray of the same kind with a new base point and
    /// direction; any other kind is returned as it is.
    pub fn with_infinite_line(&self, origin: Point, direction: Point) -> Entity {
        match self {
            Entity::Xline { .. } => Entity::Xline { origin, direction },
            Entity::Ray { .. } => Entity::Ray { origin, direction },
            other => other.clone(),
        }
    }

    /// The grips of a construction line or ray: the base point (moves it) and
    /// a point on the line a fixed distance from the screen along it (turns
    /// it), with a second one behind the base for a construction line.
    pub(crate) fn line_grips(&self) -> Vec<Grip> {
        let Some((origin, direction, ray)) = self.infinite_line() else {
            return Vec::new();
        };
        let at = |t: f64| Point::new(origin.x + direction.x * t, origin.y + direction.y * t);
        let mut grips = vec![
            Grip {
                at: origin,
                kind: GripKind::Move,
            },
            Grip {
                at: at(arm()),
                kind: GripKind::Stretch,
            },
        ];
        if !ray {
            grips.push(Grip {
                at: at(-arm()),
                kind: GripKind::Stretch,
            });
        }
        grips
    }

    /// The line with grip `index` dragged to `to`: the base moves, or the
    /// line turns about the base to run through `to`.
    pub(crate) fn line_grip_moved(&self, index: usize, to: Point) -> Option<Entity> {
        let (origin, ..) = self.infinite_line()?;
        let toward = Point::new(to.x - origin.x, to.y - origin.y);
        match index {
            0 => Some(self.translated(to.x - origin.x, to.y - origin.y)),
            1 => unit(toward).map(|d| self.with_infinite_line(origin, d)),
            2 => unit(Point::new(-toward.x, -toward.y)).map(|d| self.with_infinite_line(origin, d)),
            _ => None,
        }
    }

    /// A construction line or ray as the long line that stands for it; any
    /// other kind is returned as it is. Work done on the stand-in (a cut, a
    /// corner) comes back through [`Entity::resumed`].
    pub fn long_line(&self) -> Entity {
        match self.infinite_line() {
            Some((origin, direction, ray)) => {
                let (start, end) = segment(origin, direction, ray);
                Entity::Line { start, end }
            }
            None => self.clone(),
        }
    }

    /// What `self` (a construction line or ray) is once its stand-in has
    /// become `result`: a piece with one far end is a ray from the other end,
    /// a piece with both ends near is a line, one with both far is the
    /// original; any other result is returned as it is.
    pub fn resumed(&self, result: Entity) -> Entity {
        let Some((origin, ..)) = self.infinite_line() else {
            return result;
        };
        let Entity::Line { start, end } = result else {
            return result;
        };
        let far = |p: Point| (p.x - origin.x).hypot(p.y - origin.y) > REACH / 2.0;
        let from = |near: Point, toward: Point| match unit(Point::new(
            toward.x - near.x,
            toward.y - near.y,
        )) {
            Some(direction) => Entity::Ray {
                origin: near,
                direction,
            },
            None => self.clone(),
        };
        match (far(start), far(end)) {
            (true, true) => self.clone(),
            (true, false) => from(end, start),
            (false, true) => from(start, end),
            (false, false) => Entity::Line { start, end },
        }
    }

    /// Move a construction line or ray: `map` takes the base point and the
    /// direction (a vector) to their new values.
    pub(crate) fn remapped_line(&self, map: impl FnOnce(Point, Point) -> (Point, Point)) -> Entity {
        match self.infinite_line() {
            Some((origin, direction, _)) => {
                let (origin, direction) = map(origin, direction);
                self.with_infinite_line(origin, unit(direction).unwrap_or(direction))
            }
            None => self.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutting_a_construction_line_gives_rays_and_a_line_between() {
        let line = Entity::Xline {
            origin: p(0.0, 0.0),
            direction: p(1.0, 0.0),
        };
        let one = line.split_at(&[p(3.0, 0.0)]);
        assert_eq!(one.len(), 2);
        assert!(one.iter().all(|e| matches!(e, Entity::Ray { .. })));
        let rays: Vec<f64> = one.iter().map(|e| e.infinite_line().unwrap().1.x).collect();
        assert_eq!(
            rays.iter().filter(|x| **x < 0.0).count(),
            1,
            "one runs back"
        );
        let two = line.split_at(&[p(3.0, 0.0), p(8.0, 0.0)]);
        assert_eq!(two.len(), 3);
        let Entity::Line { start, end } = &two[1] else {
            panic!("a line between the cuts, got {:?}", two[1]);
        };
        assert!((start.x - 3.0).abs() < 1e-6 && (end.x - 8.0).abs() < 1e-6);
    }

    #[test]
    fn breaking_a_ray_removes_the_part_between_the_points() {
        let ray = Entity::Ray {
            origin: p(0.0, 0.0),
            direction: p(1.0, 0.0),
        };
        let pieces = ray.break_between(p(2.0, 1.0), p(5.0, 1.0)).unwrap();
        assert_eq!(pieces.len(), 2);
        assert!(matches!(&pieces[0], Entity::Line { .. }), "{:?}", pieces[0]);
        assert!(matches!(&pieces[1], Entity::Ray { .. }));
    }

    #[test]
    fn grips_move_the_base_or_turn_the_line_about_it() {
        let line = Entity::Xline {
            origin: Point::new(1.0, 1.0),
            direction: Point::new(1.0, 0.0),
        };
        let grips = line.grips();
        assert_eq!(grips.len(), 3, "base and a point either way");
        assert_eq!(grips[0].at, Point::new(1.0, 1.0));
        assert!(grips[1].at.x > 1.0 && grips[2].at.x < 1.0);
        let moved = line.with_grip_moved(0, Point::new(4.0, 5.0)).unwrap();
        assert_eq!(moved.infinite_line().unwrap().0, Point::new(4.0, 5.0));
        let turned = line.with_grip_moved(1, Point::new(1.0, 9.0)).unwrap();
        let (o, d, _) = turned.infinite_line().unwrap();
        assert_eq!(o, Point::new(1.0, 1.0));
        assert!(d.x.abs() < 1e-12 && (d.y - 1.0).abs() < 1e-12);
        // The grip behind the base turns it the other way round.
        let behind = line.with_grip_moved(2, Point::new(1.0, -9.0)).unwrap();
        assert!((behind.infinite_line().unwrap().1.y - 1.0).abs() < 1e-12);
        assert_eq!(line.with_grip_moved(1, Point::new(1.0, 1.0)), None);
    }

    #[test]
    fn a_ray_has_no_grip_behind_its_base() {
        let ray = Entity::Ray {
            origin: Point::new(0.0, 0.0),
            direction: Point::new(0.0, 1.0),
        };
        assert_eq!(ray.grips().len(), 2);
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_direction_is_made_a_unit_or_refused() {
        assert_eq!(unit(p(0.0, 5.0)), Some(p(0.0, 1.0)));
        assert_eq!(unit(p(0.0, 0.0)), None);
        assert_eq!(unit(p(f64::NAN, 1.0)), None);
        assert_eq!(unit(p(f64::INFINITY, 1.0)), None);
    }

    #[test]
    fn the_box_is_open_in_the_directions_the_line_runs() {
        let b = bounds(p(1.0, 2.0), p(1.0, 0.0), false);
        assert_eq!((b.min.x, b.max.x), (f64::NEG_INFINITY, f64::INFINITY));
        assert_eq!((b.min.y, b.max.y), (2.0, 2.0));
        let r = bounds(p(1.0, 2.0), p(-0.6, 0.8), true);
        assert_eq!((r.min.x, r.max.x), (f64::NEG_INFINITY, 1.0));
        assert_eq!((r.min.y, r.max.y), (2.0, f64::INFINITY));
        let window = Bounds::from_points(p(50.0, 50.0), p(60.0, 60.0));
        assert!(!r.intersects(&window), "up and to the left of (1, 2)");
        assert!(bounds(p(0.0, 0.0), p(1.0, 1.0), false).intersects(&window));
    }

    #[test]
    fn distance_is_to_the_line_and_a_ray_stops_at_its_base() {
        let d = p(1.0, 0.0);
        assert_eq!(distance_to(p(0.0, 0.0), d, false, p(-5.0, 3.0)), 3.0);
        assert_eq!(distance_to(p(0.0, 0.0), d, true, p(-4.0, 3.0)), 5.0);
        assert_eq!(distance_to(p(0.0, 0.0), d, true, p(7.0, -2.0)), 2.0);
    }

    #[test]
    fn a_line_is_cut_to_a_window() {
        let w = Bounds::from_points(p(0.0, 0.0), p(10.0, 10.0));
        let (a, b) = clipped(p(5.0, 5.0), p(1.0, 0.0), false, &w).unwrap();
        assert_eq!((a, b), (p(0.0, 5.0), p(10.0, 5.0)));
        let (a, b) = clipped(p(5.0, 5.0), p(1.0, 0.0), true, &w).unwrap();
        assert_eq!((a, b), (p(5.0, 5.0), p(10.0, 5.0)));
        assert_eq!(clipped(p(15.0, 5.0), p(1.0, 0.0), true, &w), None);
        assert_eq!(clipped(p(5.0, 20.0), p(1.0, 0.0), false, &w), None);
        let (a, b) = clipped(p(0.0, 0.0), p(0.6, 0.8), false, &w).unwrap();
        assert!(
            (a.x - 0.0).abs() < 1e-12 && (b.x - 7.5).abs() < 1e-12 && (b.y - 10.0).abs() < 1e-12
        );
    }

    #[test]
    fn the_long_segment_starts_at_the_base_of_a_ray() {
        let (a, b) = segment(p(2.0, 3.0), p(0.0, 1.0), true);
        assert_eq!(a, p(2.0, 3.0));
        assert_eq!(b, p(2.0, 3.0 + REACH));
        let (a, _) = segment(p(2.0, 3.0), p(0.0, 1.0), false);
        assert_eq!(a, p(2.0, 3.0 - REACH));
    }

    #[test]
    fn shifting_goes_left_of_the_direction_and_reflecting_keeps_the_length() {
        assert_eq!(shifted(p(0.0, 0.0), p(1.0, 0.0), 2.0), p(0.0, 2.0));
        assert_eq!(shifted(p(0.0, 0.0), p(1.0, 0.0), -2.0), p(0.0, -2.0));
        let r = reflected(p(0.0, 1.0), p(1.0, 0.0));
        assert!((r.x).abs() < 1e-12 && (r.y + 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_bisector_halves_the_angle() {
        let b = bisector(p(1.0, 0.0), p(0.0, 1.0)).unwrap();
        assert!((b.x - b.y).abs() < 1e-12 && b.x > 0.0);
        assert_eq!(bisector(p(1.0, 0.0), p(-1.0, 0.0)), None);
    }
}
