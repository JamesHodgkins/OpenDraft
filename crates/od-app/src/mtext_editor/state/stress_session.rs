//! The stress sessions: apply random operations, check what must hold after
//! each, and run many of them. See `stress_tests.rs`.

use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};

use od_core::text::{layout_text, parse};
use od_core::TextStyle;

use super::stress_ops::{apply, hostile_contents, random_op, Op, Rng};
use super::tests::editor_wrapped;
use super::*;

/// What must hold after every operation.
pub(super) fn check(e: &MTextEditor) -> Result<(), String> {
    let contents = e.contents();
    if contents.len() > od_core::text::TextLimits::default().max_content_bytes {
        return Err(format!("contents grew past the limit: {}", contents.len()));
    }
    parse(contents).map_err(|why| format!("contents no longer parse: {why:?}"))?;
    let layout = e.layout();
    if layout.carets.is_empty() || layout.lines.is_empty() {
        return Err("the layout has no caret or no line".into());
    }
    let range = e.selected_range();
    if range.end > contents.len()
        || !contents.is_char_boundary(range.start)
        || !contents.is_char_boundary(range.end)
    {
        return Err(format!("selection {range:?} is not inside the contents"));
    }
    let (from, to) = e.selection();
    let last = layout.carets.len() - 1;
    if from > last || to > last || e.caret() > last {
        return Err(format!("caret indexes {from},{to} beyond {last}"));
    }
    if layout.carets.iter().any(|c| c.source > contents.len()) {
        return Err("a caret stop lies beyond the contents".into());
    }
    let finite = layout.frame_width.is_finite()
        && layout.frame_height.is_finite()
        && layout.actual_width.is_finite()
        && layout.actual_height.is_finite()
        && layout
            .glyphs
            .iter()
            .all(|g| g.x.is_finite() && g.y.is_finite())
        && layout.carets.iter().all(|c| c.x.is_finite());
    if !finite {
        return Err("the layout holds a number that is not finite".into());
    }
    for r in layout.selection_rects(from, to) {
        if ![r.x0, r.x1, r.top, r.bottom].iter().all(|v| v.is_finite()) {
            return Err("a selection box is not finite".into());
        }
    }
    // The layout the editor holds is the layout of what it holds.
    let fresh = layout_text(e.text(), &TextStyle::standard());
    if fresh.carets.len() != layout.carets.len() || fresh.glyphs.len() != layout.glyphs.len() {
        return Err("the held layout is stale".into());
    }
    // The selection reaches the last letter when everything is selected.
    if range == (0..contents.len()) && !contents.is_empty() {
        if let Some(glyph) = layout.glyphs.last() {
            let right = layout
                .selection_rects(from.min(to), from.max(to))
                .iter()
                .map(|r| r.x1)
                .fold(f64::MIN, f64::max);
            if right < glyph.x - 1e-6 {
                return Err(format!(
                    "the highlight stops at {right} before the last letter at {}",
                    glyph.x
                ));
            }
        }
    }
    Ok(())
}

/// One session: `ops` random operations on contents made from `seed`.
pub(super) fn session(seed: u64, ops: usize, hostile: bool) -> Result<(), String> {
    let mut rng = Rng::new(seed);
    let start = if hostile {
        hostile_contents(&mut rng, rng_pieces(seed))
    } else {
        String::new()
    };
    let width = if rng.chance(60) {
        5.0 + rng.unit() * 60.0
    } else {
        0.0
    };
    let mut e = editor_wrapped(&start, width);
    check(&e).map_err(|why| format!("on opening {start:?}: {why}"))?;
    let (first, original_width) = (e.contents().to_owned(), e.text().reference_width);
    let probe = std::env::var("STRESS_PROBE").is_ok();
    let mut trace: Vec<Op> = Vec::new();
    let mut now = 0.0;
    for step in 0..ops {
        let op = random_op(&mut rng);
        now += rng.unit() * 0.9;
        trace.push(op.clone());
        let ran = catch_unwind(AssertUnwindSafe(|| {
            apply(&mut e, &op, now);
            check(&e)?;
            if probe {
                // One step back and forth must land where it started.
                let here = (e.contents().to_owned(), e.text().reference_width);
                if e.undo() {
                    check(&e).map_err(|why| format!("after one undo: {why}"))?;
                    if !e.redo()
                        || (e.contents(), e.text().reference_width) != (here.0.as_str(), here.1)
                    {
                        return Err(format!(
                            "undo then redo did not return: {:?} became {:?}",
                            here.0,
                            e.contents()
                        ));
                    }
                }
            }
            Ok(())
        }));
        let note = |trace: &[Op]| {
            let mut out = String::new();
            for op in trace.iter().rev().take(8).rev() {
                let _ = write!(out, "\n    {op:?}");
            }
            out
        };
        match ran {
            Err(_) => {
                return Err(format!(
                    "panicked at step {step} (start {start:?}); last operations:{}",
                    note(&trace)
                ))
            }
            Ok(Err(why)) => {
                return Err(format!(
                    "{why} at step {step} (start {start:?}); last operations:{}",
                    note(&trace)
                ))
            }
            Ok(Ok(())) => {}
        }
    }
    // Undo all the way back, then redo all the way forward.
    // A session may end part way back in its history: go to the end of it.
    while e.redo() {}
    let last = e.contents().to_owned();
    let last_width = e.text().reference_width;
    let mut guard = 0;
    let mut down = vec![last.clone()];
    while e.undo() {
        guard += 1;
        if guard > 100_000 {
            return Err("undo never ends".into());
        }
        down.push(e.contents().to_owned());
    }
    check(&e).map_err(|why| format!("after undoing everything: {why}"))?;
    // The history keeps the last 128 steps, so a long session cannot go back
    // to its start; only one that dropped nothing is held to that.
    let reached_start = e.history_complete();
    if reached_start
        && (e.contents() != first || (e.text().reference_width - original_width).abs() > 1e-9)
    {
        return Err(format!(
            "undoing everything ({} steps) did not restore the start: {:?} != {first:?}",
            down.len() - 1,
            e.contents()
        ));
    }
    let mut up = vec![e.contents().to_owned()];
    while e.redo() {
        guard += 1;
        if guard > 200_000 {
            return Err("redo never ends".into());
        }
        up.push(e.contents().to_owned());
    }
    down.reverse();
    if let Some(at) = (0..up.len().min(down.len())).find(|&i| up[i] != down[i]) {
        return Err(format!(
            "redo step {at} of {} differs from the undo that led there:
  redo {:?}
  undo {:?}",
            up.len(),
            up[at],
            down[at]
        ));
    }
    if e.contents() != last || (e.text().reference_width - last_width).abs() > 1e-9 {
        return Err(format!(
            "redoing everything did not restore the end ({} undo steps, {} redo steps):
  now  {:?} (width {})
  want {last:?} (width {last_width})",
            down.len(),
            up.len(),
            e.contents(),
            e.text().reference_width
        ));
    }
    Ok(())
}

fn rng_pieces(seed: u64) -> usize {
    1 + (Rng::new(seed ^ 0xABCD).below(40))
}

pub(super) fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub(super) fn run_many(seeds: u64, ops: usize, hostile: bool) {
    if let Ok(one) = std::env::var("STRESS_SEED") {
        let seed = one.parse().expect("STRESS_SEED is a number");
        if let Err(why) = session(seed, ops, hostile) {
            panic!("seed {seed}: {why}");
        }
        return;
    }
    let mut failures = Vec::new();
    for seed in 0..seeds {
        if let Err(why) = session(seed, ops, hostile) {
            failures.push(format!("seed {seed}: {why}"));
            if failures.len() >= 3 {
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
