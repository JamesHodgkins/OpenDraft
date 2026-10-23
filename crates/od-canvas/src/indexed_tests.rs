//! Picking, snapping and region selection through the document's spatial
//! index give exactly what a scan of the objects gives. The drawing is above
//! the size at which a space gets a tree, and every index query is checked
//! against a scan as well.

use od_core::{DimStyle, Dimension, Document, Ellipse, Entity, EntityId, Point};

use crate::hit::{pick, pick_here, probe};
use crate::select::{select, select_in, Region};
use crate::snap::{snap, SnapModes};

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
}

fn mixed(n: usize) -> Document {
    let mut rng = Rng(11);
    let style = DimStyle::default();
    let mut doc = Document::default();
    for i in 0..n {
        let (x, y) = (rng.unit() * 1000.0, rng.unit() * 600.0);
        let size = 2.0 + rng.unit() * 30.0;
        let at = Point::new(x, y);
        let entity = match i % 8 {
            0 => Entity::Line {
                start: at,
                end: Point::new(x + size, y - size / 2.0),
            },
            1 => Entity::Circle {
                center: at,
                radius: size / 2.0,
            },
            2 => Entity::Arc {
                center: at,
                radius: size,
                start: 0.3,
                end: 2.0,
            },
            3 => Entity::Rectangle {
                min: at,
                max: Point::new(x + size, y + size / 2.0),
            },
            4 => Entity::Polyline {
                points: vec![at, Point::new(x + size, y), Point::new(x + size, y + size)],
                closed: i % 16 == 4,
                bulges: vec![0.0, 0.5, 0.0],
                widths: Vec::new(),
            },
            5 => Entity::Dimension(Box::new(Dimension::aligned(
                at,
                Point::new(x + size, y),
                Point::new(x, y + 5.0),
                &style,
            ))),
            6 => match Ellipse::new(
                at,
                Point::new(size, size / 3.0),
                0.5,
                0.0,
                std::f64::consts::TAU,
            ) {
                Ok(e) => Entity::Ellipse(e),
                Err(_) => continue,
            },
            _ => Entity::Line { start: at, end: at },
        };
        doc.add(entity);
    }
    doc.set_verify_all_queries(true);
    doc
}

#[test]
fn pick_snap_and_selection_through_the_index_match_a_scan() {
    let doc = mixed(4_200);
    assert!(doc.spatial_stats().builds >= 1, "the space has a tree");
    let mut rng = Rng(5);
    let modes = SnapModes {
        nearest: true,
        quadrant: true,
        ..SnapModes::default()
    };
    for _ in 0..120 {
        let c = Point::new(rng.unit() * 1040.0 - 20.0, rng.unit() * 640.0 - 20.0);
        let tolerance = 0.5 + rng.unit() * 8.0;
        assert_eq!(
            pick_here(&doc, c, tolerance),
            pick(doc.selectable_objects(), c, tolerance),
            "pick at {c:?}"
        );
        let near = doc.visible_here_in_rect(probe(c, tolerance));
        assert_eq!(
            snap(near, c, tolerance, Some(10.0), &modes),
            snap(doc.visible_objects(), c, tolerance, Some(10.0), &modes),
            "snap at {c:?}"
        );
    }
    for k in 0..30 {
        let a = Point::new(rng.unit() * 1000.0, rng.unit() * 600.0);
        let b = Point::new(
            a.x + rng.unit() * 150.0 - 40.0,
            a.y + rng.unit() * 90.0 - 30.0,
        );
        let c = Point::new(a.x + rng.unit() * 100.0, a.y - rng.unit() * 60.0);
        let corners = Region::rect_corners(a, b);
        let regions = [
            Region::Window(corners.clone()),
            Region::Crossing(corners),
            Region::Window(vec![a, b, c]),
            Region::Crossing(vec![a, b, c, Point::new(a.x, a.y + 20.0)]),
            Region::Fence(vec![a, b, c]),
        ];
        for region in regions {
            assert_eq!(
                select_in(&doc, &region),
                select(doc.selectable_objects(), &region),
                "{region:?} (round {k})"
            );
        }
    }
    let degenerate = Region::Window(vec![Point::new(5.0, 5.0)]);
    assert!(select_in(&doc, &degenerate).is_empty());
    assert!(doc.spatial_stats().tree_queries > 100);
}

#[test]
fn what_is_visible_in_a_view_is_what_a_scan_of_its_bounds_finds() {
    let doc = mixed(4_200);
    let view = od_core::Bounds::from_points(Point::new(100.0, 100.0), Point::new(180.0, 150.0));
    let found: Vec<EntityId> = doc
        .visible_in_rect(None, &[], view)
        .iter()
        .map(|o| o.id)
        .collect();
    let scanned: Vec<EntityId> = doc
        .visible_in(None)
        .filter(|o| o.bounds().intersects(&view))
        .map(|o| o.id)
        .collect();
    assert_eq!(found, scanned);
    assert!(!found.is_empty() && found.len() < 300);
}
