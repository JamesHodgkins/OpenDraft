#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Prints the number of strokes and their total length for every pattern
//! hatch of a drawing, for comparing with `tests/fixtures/hatch_lengths.py`
//! (ezdxf's own generator).
//!
//!     cargo run -p od-files --example hatch_lengths -- HatchTest.dxf

use od_core::hatch::{HatchFill, HatchLimits};
use od_core::Entity;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let loaded = od_files::load(std::path::Path::new(&path)).unwrap();
    let hatches = loaded
        .document
        .objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Hatch(h) => Some(h),
            _ => None,
        });
    for (i, h) in hatches.enumerate() {
        if !matches!(h.fill, HatchFill::Pattern(_)) {
            continue;
        }
        let done = h.evaluate(1e-4, &HatchLimits::default()).unwrap();
        let total: f64 = done
            .strokes
            .iter()
            .map(|s| (s[0].x - s[1].x).hypot(s[0].y - s[1].y))
            .sum();
        println!("{i} {} {total:.6}", done.strokes.len());
        if std::env::var("OD_SHOW").is_ok_and(|v| v == i.to_string()) {
            for s in &done.strokes {
                println!(
                    "   ({:.4},{:.4}) -> ({:.4},{:.4})",
                    s[0].x, s[0].y, s[1].x, s[1].y
                );
            }
        }
    }
}
