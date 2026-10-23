#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Writes a small DXF of splines (plain, rational, fitted, closed), for
//! checking with other programs (see `tests/fixtures/check_export.py`).
//!
//!     cargo run -p od-files --example export_splines -- out.dxf

use od_core::{Document, Entity, KnotParameterization, Point, Spline};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = Point::new;
    let mut document = Document::default();
    let plain =
        Spline::from_control_points(3, vec![p(0.0, 0.0), p(2.0, 4.0), p(5.0, -1.0), p(8.0, 3.0)]);
    let rational = Spline::new_nurbs(
        2,
        vec![p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        Some(vec![1.0, 0.5f64.sqrt(), 1.0]),
        false,
        false,
    );
    let fitted = Spline::from_fit_points(
        vec![p(0.0, 10.0), p(3.0, 13.0), p(6.0, 9.0), p(9.0, 12.0)],
        KnotParameterization::Chord,
    );
    let closed = Spline::new_nurbs(
        1,
        vec![p(20.0, 0.0), p(24.0, 0.0), p(22.0, 3.0), p(20.0, 0.0)],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        None,
        true,
        false,
    );
    for s in [plain, rational, fitted, closed] {
        document.add(Entity::Spline(Box::new(s.unwrap())));
    }
    let notes = od_files::save(std::path::Path::new(&path), &document).unwrap();
    for note in notes {
        println!("note: {note}");
    }
}
