#![allow(clippy::print_stdout, clippy::print_stderr)] // a command-line tool
//! Spatial index costs: building, querying, editing and cloning.
//!
//! Run in release mode:
//!   cargo run --release -p od-core --example bench_index
//!
//! Plain circles scattered uniformly (the same drawing for every row), with a
//! sparse query (about 1 object), a dense one (about 1% of them) and the
//! whole drawing, each also answered by a scan, which is what the index
//! replaced.

use std::time::Instant;

use od_core::{Bounds, Document, Entity, EntityId, Point};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
}

fn circles(n: usize) -> Vec<Entity> {
    let mut rng = Lcg(42);
    let side = (n as f64).sqrt() * 10.0;
    (0..n)
        .map(|_| Entity::Circle {
            center: Point::new(rng.next() * side, rng.next() * side),
            radius: 1.0 + rng.next() * 4.0,
        })
        .collect()
}

fn replacement_at_origin() -> Entity {
    Entity::Circle {
        center: Point::new(5.0, 5.0),
        radius: 2.0,
    }
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

/// Median of `runs` timings of `f`, in microseconds.
fn median_us(runs: usize, mut f: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[runs / 2]
}

fn main() {
    println!("{:>9}  {:<34} {:>12}", "objects", "step", "time");
    for n in [10_000, 100_000, 1_000_000] {
        let entities = circles(n);
        let side = (n as f64).sqrt() * 10.0;

        let t = Instant::now();
        let mut doc = Document::default();
        doc.begin_bulk_load();
        for e in entities.iter().cloned() {
            doc.add(e);
        }
        let loaded = ms(t);
        let t = Instant::now();
        doc.finish_bulk_load();
        println!(
            "{n:>9}  load, tree built once at the end {:>9.1} ms",
            loaded + ms(t)
        );
        println!("{n:>9}    of which the tree {:>24.1} ms", ms(t));

        let t = Instant::now();
        let mut one_by_one = Document::default();
        for e in entities.iter().cloned() {
            one_by_one.add(e);
        }
        println!("{n:>9}  load, tree updated per object {:>8.1} ms", ms(t));
        drop(one_by_one);

        let at = |frac: f64| {
            let c = side * 0.37;
            let half = side * frac / 2.0;
            Bounds::from_points(
                Point::new(c - half, c - half),
                Point::new(c + half, c + half),
            )
        };
        for (label, rect) in [
            ("sparse", at(0.001)),
            ("dense (1% of the area)", at(0.1)),
            ("whole drawing", at(1.5)),
        ] {
            let found = doc.objects_intersecting(None, rect).len();
            let tree = median_us(20, || {
                std::hint::black_box(doc.objects_intersecting(None, rect));
            });
            let scan = median_us(20, || {
                let n = doc
                    .objects()
                    .iter()
                    .filter(|o| o.bounds().intersects(&rect))
                    .count();
                std::hint::black_box(n);
            });
            println!(
                "{n:>9}  query {label} ({found} found) {tree:>10.1} us, a scan {scan:>9.1} us"
            );
        }

        let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();
        let mut moved = Lcg(7);
        let mut replacement = || Entity::Circle {
            center: Point::new(moved.next() * side, moved.next() * side),
            radius: 2.0,
        };
        let one = median_us(20, || {
            doc.replace(ids[n / 2], replacement());
        });
        println!("{n:>9}  replace one object {one:>25.1} us");
        for (label, share) in [("1%", 100), ("10%", 10), ("50%", 2)] {
            let changes: Vec<(EntityId, Entity)> = ids
                .iter()
                .step_by(share)
                .map(|id| (*id, replacement()))
                .collect();
            let count = changes.len();
            let t = Instant::now();
            doc.replace_many(changes);
            println!(
                "{n:>9}  replace {label} ({count}) in one call {:>14.1} ms",
                ms(t)
            );
        }

        if n == 100_000 {
            // Many objects with one and the same box: the worst case for a tree.
            let mut stacked = Document::default();
            stacked.begin_bulk_load();
            for _ in 0..n {
                stacked.add(replacement_at_origin());
            }
            let t = Instant::now();
            stacked.finish_bulk_load();
            println!("{n:>9}  build with every box identical {:>11.1} ms", ms(t));
        }
        let t = Instant::now();
        let copy = doc.clone();
        println!("{n:>9}  clone the drawing {:>26.1} ms", ms(t));
        drop(copy);
        println!();
    }
}
