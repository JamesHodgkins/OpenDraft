//! The spatial index against a plain scan: after every kind of edit, every
//! query must find what a scan of the objects finds, in the same order.

use super::spatial::Config;
use super::*;
use crate::Point;

pub(super) type Space = Option<LayoutId>;

/// Small limits so a few dozen objects exercise every path.
pub(super) const SMALL: Config = Config {
    activate: 12,
    deactivate: 8,
    min_bulk: 4,
    bulk_fraction: 0.25,
};

pub(super) const SPACES: [Space; 3] = [None, Some(LayoutId(1)), Some(LayoutId(2))];

/// A fixed-seed generator, so a failure repeats.
pub(super) struct Rng(pub(super) u64);

impl Rng {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    pub(super) fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub(super) fn unit(&mut self) -> f64 {
        self.next() as f64 / (1u64 << 31) as f64
    }
}

pub(super) fn entity(rng: &mut Rng) -> Entity {
    let (x, y) = (rng.unit() * 100.0, rng.unit() * 100.0);
    match rng.below(10) {
        0 => Entity::Circle {
            center: Point::new(f64::NAN, y),
            radius: 1.0,
        },
        1 => Entity::Circle {
            center: Point::new(x, y),
            radius: f64::INFINITY,
        },
        2..=5 => Entity::Circle {
            center: Point::new(x, y),
            radius: rng.unit() * 6.0,
        },
        _ => Entity::Line {
            start: Point::new(x, y),
            end: Point::new(x + rng.unit() * 20.0 - 10.0, y + rng.unit() * 20.0 - 10.0),
        },
    }
}

pub(super) fn object(doc: &mut Document, rng: &mut Rng, space: Space) -> Object {
    let (id, e) = (doc.allocate_id(), entity(rng));
    Object::new(id, e, Attributes::on_layer(LayerId::DEFAULT), space)
}

pub(super) fn rect(rng: &mut Rng) -> Bounds {
    let (x, y) = (rng.unit() * 120.0 - 10.0, rng.unit() * 120.0 - 10.0);
    let (w, h) = match rng.below(4) {
        0 => (0.0, 0.0),
        1 => (rng.unit() * 200.0, rng.unit() * 2.0),
        _ => (rng.unit() * 30.0, rng.unit() * 30.0),
    };
    Bounds::from_points(Point::new(x, y), Point::new(x + w, y + h))
}

/// Every query kind agrees with a scan, in every space.
pub(super) fn agrees(doc: &Document, rng: &mut Rng) {
    doc.check_spatial_index();
    for _ in 0..6 {
        let r = rect(rng);
        for space in SPACES {
            let got: Vec<EntityId> = doc
                .objects_intersecting(space, r)
                .iter()
                .map(|o| o.id)
                .collect();
            assert_eq!(got, doc.scan_ids(space, &r), "{space:?} {r:?}");
        }
    }
}

pub(super) fn some_ids(doc: &Document, rng: &mut Rng, n: usize) -> Vec<EntityId> {
    let len = doc.len().max(1);
    (0..n)
        .map(|_| {
            doc.objects()
                .get(rng.below(len))
                .map_or(EntityId(9999), |o| o.id)
        })
        .collect()
}

#[test]
fn queries_agree_with_a_scan_through_a_long_run_of_every_kind_of_edit() {
    for seed in 1..=8u64 {
        let mut rng = Rng(seed);
        let mut doc = Document::default();
        doc.set_index_config(SMALL);
        doc.set_verify_all_queries(true);
        for step in 0..160 {
            let n = [1, 2, 5, 9, 20][rng.below(5)];
            let space = SPACES[rng.below(3)];
            match rng.below(9) {
                0 => {
                    let o = object(&mut doc, &mut rng, space);
                    doc.push(o);
                }
                1 => {
                    let objects: Vec<Object> =
                        (0..n).map(|_| object(&mut doc, &mut rng, space)).collect();
                    doc.push_many(objects);
                }
                2 => {
                    let ids = some_ids(&doc, &mut rng, n);
                    let taken = doc.remove_many(&ids);
                    if rng.below(2) == 0 {
                        doc.insert_many(taken);
                    }
                }
                3 => {
                    let items: Vec<(usize, Object)> = (0..n)
                        .map(|_| (rng.below(doc.len() + 2), object(&mut doc, &mut rng, space)))
                        .collect();
                    doc.insert_many(items);
                }
                4 => {
                    let ids = some_ids(&doc, &mut rng, n);
                    let changes = ids.into_iter().map(|id| (id, entity(&mut rng))).collect();
                    let old = doc.replace_many(changes);
                    assert!(old.len() == n);
                }
                5 => {
                    let ids = some_ids(&doc, &mut rng, 1);
                    doc.replace(ids[0], entity(&mut rng));
                }
                6 => {
                    let ids = some_ids(&doc, &mut rng, 1);
                    let attrs = Attributes::on_layer(LayerId::DEFAULT);
                    doc.replace_attrs(ids[0], attrs);
                }
                7 => {
                    let every = 2 + rng.below(4);
                    let mut seen = 0;
                    doc.replace_each(|_| {
                        seen += 1;
                        (seen % every == 0).then(|| {
                            vec![(
                                entity(&mut Rng(seen as u64)),
                                Attributes::on_layer(LayerId::DEFAULT),
                            )]
                        })
                    });
                }
                _ => {
                    let ids = some_ids(&doc, &mut rng, 1);
                    doc.remove(ids[0]);
                }
            }
            agrees(&doc, &mut rng);
            assert!(doc.position(EntityId(u64::MAX)).is_none(), "step {step}");
        }
        assert!(
            doc.spatial_stats().tree_queries > 20,
            "seed {seed} never used a tree"
        );
    }
}

pub(super) fn grid(n: usize) -> Document {
    let mut doc = Document::default();
    let side = (n as f64).sqrt().ceil() as usize;
    for i in 0..n {
        doc.add(Entity::Circle {
            center: Point::new((i % side) as f64 * 10.0, (i / side) as f64 * 10.0),
            radius: 1.0,
        });
    }
    doc
}

pub(super) fn near(x: f64, y: f64) -> Bounds {
    Bounds::from_points(Point::new(x - 2.0, y - 2.0), Point::new(x + 2.0, y + 2.0))
}
