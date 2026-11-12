//! Arrays: where the copies made by ARRAYRECT, ARRAYPOLAR and ARRAYPATH
//! go. Each function lists every item of the array as a [`Placement`], the
//! original objects' own (no change) first, so a command adds a copy of the
//! objects for each of the rest.

use std::f64::consts::TAU;

use crate::{arc, bulge, Entity, Point};

/// The most items an array may have, as AutoCAD's limit.
pub const MAX_ITEMS: usize = 100_000;

/// One item of an array: the objects turned `angle` radians about `pivot`,
/// then moved by `offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub offset: Point,
    pub pivot: Point,
    pub angle: f64,
}

impl Placement {
    /// No change: where the original objects are.
    pub const STAY: Placement = Placement {
        offset: Point { x: 0.0, y: 0.0 },
        pivot: Point { x: 0.0, y: 0.0 },
        angle: 0.0,
    };

    fn moved(dx: f64, dy: f64) -> Placement {
        Placement {
            offset: Point::new(dx, dy),
            ..Placement::STAY
        }
    }

    /// `entity` as this item has it.
    pub fn apply(&self, entity: &Entity) -> Entity {
        let turned = if self.angle == 0.0 {
            entity.clone()
        } else {
            entity.rotated(self.pivot, self.angle)
        };
        turned.translated(self.offset.x, self.offset.y)
    }

    /// Where `p` goes in this item.
    pub fn apply_point(&self, p: Point) -> Point {
        p.rotated(self.pivot, self.angle)
            .translated(self.offset.x, self.offset.y)
    }
}

fn count_ok(count: usize) -> bool {
    (1..=MAX_ITEMS).contains(&count)
}

/// A rectangular array: `columns` along x every `column_spacing` and `rows`
/// along y every `row_spacing` (negative spacings go left or down), row by
/// row. `None` when a count is 0 or the whole is past [`MAX_ITEMS`].
pub fn rectangular(
    columns: usize,
    rows: usize,
    column_spacing: f64,
    row_spacing: f64,
) -> Option<Vec<Placement>> {
    let total = columns.checked_mul(rows)?;
    if !count_ok(columns) || !count_ok(rows) || !count_ok(total) {
        return None;
    }
    if !column_spacing.is_finite() || !row_spacing.is_finite() {
        return None;
    }
    let mut items = Vec::with_capacity(total);
    for row in 0..rows {
        for column in 0..columns {
            items.push(Placement::moved(
                column as f64 * column_spacing,
                row as f64 * row_spacing,
            ));
        }
    }
    Some(items)
}

/// Whether `fill` is a whole turn, where the last item would land on the
/// first.
fn whole_turn(fill: f64) -> bool {
    (fill.abs() - TAU).abs() < 1e-9
}

/// The angle between items of a polar array of `items` filling `fill`.
pub fn polar_step(items: usize, fill: f64) -> f64 {
    match items {
        0 | 1 => 0.0,
        n if whole_turn(fill) => fill / n as f64,
        n => fill / (n - 1) as f64,
    }
}

/// A polar array: `items` about `center` over `fill` radians (positive
/// counter-clockwise). With `rotate` each item turns as it goes round;
/// without, it keeps its direction and only its `base` point goes round.
pub fn polar(
    center: Point,
    base: Point,
    items: usize,
    fill: f64,
    rotate: bool,
) -> Option<Vec<Placement>> {
    if !count_ok(items) || !fill.is_finite() || fill.abs() > TAU + 1e-9 {
        return None;
    }
    let step = polar_step(items, fill);
    Some(
        (0..items)
            .map(|k| {
                let angle = step * k as f64;
                if rotate {
                    Placement {
                        pivot: center,
                        angle,
                        ..Placement::STAY
                    }
                } else {
                    let to = base.rotated(center, angle);
                    Placement::moved(to.x - base.x, to.y - base.y)
                }
            })
            .collect(),
    )
}

/// A curve a path array follows, as a fine chain of points measured along
/// its length.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    points: Vec<Point>,
    /// The distance along the path to each point.
    lengths: Vec<f64>,
    closed: bool,
}

impl Path {
    /// The path along `entity`, from its start: lines, arcs, circles,
    /// polylines, rectangles, ellipses and splines. `None` for anything else
    /// or a curve with no length.
    pub fn of(entity: &Entity) -> Option<Path> {
        let size = {
            let b = entity.bounds();
            (b.max.x - b.min.x).hypot(b.max.y - b.min.y)
        };
        let tolerance = (size * 1e-5).max(1e-9);
        let (points, closed) = match entity {
            Entity::Line { start, end } => (vec![*start, *end], false),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => (
                arc::flatten(*center, *radius, *start, *end, tolerance),
                false,
            ),
            Entity::Circle { center, radius } => {
                (arc::flatten(*center, *radius, 0.0, 0.0, tolerance), true)
            }
            Entity::Rectangle { min, max } => (
                vec![
                    *min,
                    Point::new(max.x, min.y),
                    *max,
                    Point::new(min.x, max.y),
                    *min,
                ],
                true,
            ),
            Entity::Polyline {
                points,
                bulges,
                closed,
                ..
            } => {
                let mut flat = bulge::flatten_path(points, bulges, *closed, tolerance);
                // The flattened path leaves the closing point off.
                if let (true, Some(&first)) = (*closed, flat.first()) {
                    flat.push(first);
                }
                (flat, *closed)
            }
            Entity::Ellipse(e) => (e.flatten(tolerance).ok()?, e.is_full()),
            Entity::Spline(s) => {
                let flat = s.flatten(tolerance).ok()?;
                let ends_meet = match (flat.first(), flat.last()) {
                    (Some(a), Some(b)) => (a.x - b.x).hypot(a.y - b.y) <= tolerance,
                    _ => false,
                };
                (flat, ends_meet)
            }
            _ => return None,
        };
        Path::through(points, closed)
    }

    fn through(points: Vec<Point>, closed: bool) -> Option<Path> {
        let mut lengths = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                let q = points[i - 1];
                total += (p.x - q.x).hypot(p.y - q.y);
            }
            lengths.push(total);
        }
        (points.len() >= 2 && total.is_finite() && total > 1e-12).then_some(Path {
            points,
            lengths,
            closed,
        })
    }

    /// The same path run from its other end.
    pub fn reversed(&self) -> Path {
        let mut points = self.points.clone();
        points.reverse();
        Path::through(points, self.closed).unwrap_or_else(|| self.clone())
    }

    pub fn start(&self) -> Point {
        self.points[0]
    }

    pub fn end(&self) -> Point {
        self.points[self.points.len() - 1]
    }

    pub fn length(&self) -> f64 {
        self.lengths[self.lengths.len() - 1]
    }

    /// Whether the path ends where it starts (a circle, a closed polyline).
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The point `s` along the path (held to its ends) and the direction it
    /// runs there, in radians.
    pub fn at(&self, s: f64) -> (Point, f64) {
        let s = s.clamp(0.0, self.length());
        let i = self
            .lengths
            .partition_point(|&l| l < s)
            .clamp(1, self.points.len() - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let piece = self.lengths[i] - self.lengths[i - 1];
        let t = if piece > 0.0 {
            (s - self.lengths[i - 1]) / piece
        } else {
            0.0
        };
        let at = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        (at, (b.y - a.y).atan2(b.x - a.x))
    }

    /// How many items fit `spacing` apart from the start (Measure), one
    /// fewer on a closed path, whose end is its start.
    pub fn items_fitting(&self, spacing: f64) -> usize {
        if !spacing.is_finite() || spacing <= 0.0 {
            return 1;
        }
        // A flattened curve is a little shorter than the curve itself.
        let gaps = (self.length() / spacing + 1e-4).floor();
        let fit = if self.closed { gaps } else { gaps + 1.0 };
        (fit.max(1.0) as usize).min(MAX_ITEMS)
    }

    /// The spacing that spreads `items` evenly along the path (Divide).
    pub fn spacing_dividing(&self, items: usize) -> f64 {
        match items {
            0 | 1 => 0.0,
            n if self.closed => self.length() / n as f64,
            n => self.length() / (n - 1) as f64,
        }
    }

    /// A path array: `items` from the start every `spacing`. The first item
    /// is where the objects are; each copy moves by how far the path has
    /// travelled, and with `align` also turns as the path turns.
    pub fn array(&self, items: usize, spacing: f64, align: bool) -> Option<Vec<Placement>> {
        if !count_ok(items) || !spacing.is_finite() || spacing < 0.0 {
            return None;
        }
        let (start, start_dir) = self.at(0.0);
        Some(
            (0..items)
                .map(|k| {
                    let (at, dir) = self.at(spacing * k as f64);
                    Placement {
                        offset: Point::new(at.x - start.x, at.y - start.y),
                        pivot: start,
                        angle: if align { dir - start_dir } else { 0.0 },
                    }
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn close(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
    }

    #[test]
    fn a_rectangular_array_goes_row_by_row_from_the_original() {
        let items = rectangular(3, 2, 10.0, -5.0).unwrap();
        assert_eq!(items.len(), 6);
        assert_eq!(items[0], Placement::STAY);
        assert!(close(items[2].offset, p(20.0, 0.0)));
        assert!(close(items[3].offset, p(0.0, -5.0)));
        assert!(rectangular(0, 2, 1.0, 1.0).is_none());
        assert!(
            rectangular(1000, 1000, 1.0, 1.0).is_none(),
            "past the limit"
        );
    }

    #[test]
    fn a_polar_array_shares_a_whole_turn_or_spans_a_part_of_one() {
        assert!((polar_step(6, TAU) - TAU / 6.0).abs() < 1e-12);
        assert!((polar_step(3, PI) - FRAC_PI_2).abs() < 1e-12);
        assert!((polar_step(4, -PI) + PI / 3.0).abs() < 1e-12, "clockwise");
        let line = Entity::Line {
            start: p(10.0, 0.0),
            end: p(12.0, 0.0),
        };
        let turned = polar(p(0.0, 0.0), p(11.0, 0.0), 4, TAU, true).unwrap();
        let Entity::Line { start, end } = turned[1].apply(&line) else {
            panic!("a line stays a line");
        };
        assert!(close(start, p(0.0, 10.0)) && close(end, p(0.0, 12.0)));
        let kept = polar(p(0.0, 0.0), p(11.0, 0.0), 4, TAU, false).unwrap();
        let Entity::Line { start, end } = kept[1].apply(&line) else {
            panic!("a line stays a line");
        };
        assert!(
            close(start, p(-1.0, 11.0)) && close(end, p(1.0, 11.0)),
            "not turned"
        );
    }

    #[test]
    fn a_path_is_measured_along_its_length() {
        let path = Path::of(&Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        })
        .unwrap();
        assert!((path.length() - 20.0).abs() < 1e-12);
        let (at, dir) = path.at(15.0);
        assert!(close(at, p(10.0, 5.0)));
        assert!((dir - FRAC_PI_2).abs() < 1e-12);
        assert_eq!(path.items_fitting(5.0), 5);
        assert!((path.spacing_dividing(5) - 5.0).abs() < 1e-12);
        let back = path.reversed();
        assert!(close(back.start(), p(10.0, 10.0)));
    }

    #[test]
    fn a_path_array_moves_and_turns_with_the_path() {
        let path = Path::of(&Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        })
        .unwrap();
        let items = path.array(3, 10.0, true).unwrap();
        assert_eq!(
            items[0],
            Placement {
                pivot: p(0.0, 0.0),
                ..Placement::STAY
            }
        );
        // A point just above the start goes to just left of the last item.
        assert!(close(items[2].apply_point(p(0.0, 1.0)), p(9.0, 10.0)));
        let plain = path.array(3, 10.0, false).unwrap();
        assert!(close(plain[2].apply_point(p(0.0, 1.0)), p(10.0, 11.0)));
    }

    #[test]
    fn a_circle_path_is_closed_and_shares_its_length() {
        let path = Path::of(&Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        })
        .unwrap();
        assert!(path.is_closed());
        assert!((path.length() - TAU).abs() < 1e-4);
        assert_eq!(path.items_fitting(TAU / 4.0), 4);
        let text = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(2.0, 1.0),
        };
        assert!(Path::of(&text).is_some_and(|p| (p.length() - 6.0).abs() < 1e-12));
    }
}
