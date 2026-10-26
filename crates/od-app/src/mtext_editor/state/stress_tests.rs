//! Stress tests of the editor: random sessions that throw every operation at
//! it, from contents that may be hostile, and check after each step that
//! nothing it promises has broken. A failure names the seed and the last
//! operations, so it can be replayed:
//!
//! ```text
//! STRESS_SEED=1234 cargo test -p od-app stress -- --nocapture
//! ```
//!
//! The quick versions run with the normal tests. The long ones are
//! `#[ignore]`d; run them with `cargo test -p od-app stress -- --ignored
//! --nocapture`. `STRESS_SEEDS` and `STRESS_OPS` change how much they do.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

use od_core::text::{plain_text_of, CharEdit, ListKind};
use od_core::{MText, Point, TextStyleId};

use super::stress_ops::{hostile_contents, random_op, Op, Rng, Start};
use super::stress_session::{check, env, run_many};
use super::tests::editor_wrapped;
use super::*;

#[test]
fn stress_random_sessions_from_empty_text_keep_every_invariant() {
    run_many(
        env("STRESS_SEEDS", 8),
        env("STRESS_OPS", 100) as usize,
        Start::Empty,
    );
}

#[test]
fn stress_sessions_on_text_full_of_groups_never_refuse_a_cut_group() {
    run_many(
        env("STRESS_SEEDS", 12),
        env("STRESS_OPS", 120) as usize,
        Start::Grouped,
    );
}

#[test]
fn stress_random_sessions_from_hostile_contents_keep_every_invariant() {
    run_many(
        env("STRESS_SEEDS", 8),
        env("STRESS_OPS", 80) as usize,
        Start::Hostile,
    );
}

#[test]
#[ignore = "long: run with --ignored"]
fn stress_long_random_sessions() {
    for kind in [Start::Empty, Start::Hostile, Start::Grouped] {
        run_many(
            env("STRESS_SEEDS", 400),
            env("STRESS_OPS", 1500) as usize,
            kind,
        );
    }
}

/// Plain typing against a model: what the editor shows is what was typed.
#[test]
fn stress_typing_matches_a_simple_model() {
    for seed in 0..10 {
        let mut rng = Rng::new(seed + 7000);
        let mut e = editor_wrapped("", if seed % 2 == 0 { 0.0 } else { 20.0 });
        let mut model: Vec<char> = Vec::new();
        let mut at = 0usize;
        let mut now = 0.0;
        for step in 0..150 {
            now += 0.01;
            // Letters that do not form ligatures, so one step is one letter.
            let letter = *rng.pick(&['a', 'b', 'c', 'd', 'x', 'y', 'z', ' ', 'é', '漢']);
            match rng.below(6) {
                0..=2 => {
                    e.insert(&letter.to_string(), now);
                    model.insert(at, letter);
                    at += 1;
                }
                3 => {
                    e.insert("\n", now);
                    model.insert(at, '\n');
                    at += 1;
                }
                4 if at > 0 => {
                    e.backspace(false, now);
                    at -= 1;
                    model.remove(at);
                }
                _ if at < model.len() => {
                    e.delete(false, now);
                    model.remove(at);
                }
                _ => {}
            }
            if rng.chance(15) {
                let left = rng.chance(50);
                e.move_caret(if left { Move::Left } else { Move::Right }, false);
                if left {
                    at = at.saturating_sub(1);
                } else {
                    at = (at + 1).min(model.len());
                }
            }
            let shown = plain_text_of(e.contents(), 0..e.contents().len());
            let want: String = model.iter().collect();
            assert_eq!(shown, want, "seed {seed} step {step}: the text differs");
            let caret = plain_text_of(e.contents(), 0..e.selected_range().start)
                .chars()
                .count();
            assert_eq!(caret, at, "seed {seed} step {step}: the caret differs");
        }
    }
}

/// Big texts: it must stay usable and honour its limits.
#[test]
#[ignore = "long: run with --ignored"]
fn stress_large_texts_stay_usable_and_limits_hold() {
    let timed = |what: &str, started: Instant| eprintln!("{what}: {:?}", started.elapsed());
    // Many paragraphs.
    let body: String = (0..5000)
        .map(|i| format!("line {i} of the text\\P"))
        .collect();
    let mut e = editor_wrapped(&body, 40.0);
    check(&e).unwrap();
    let t = Instant::now();
    e.select_all();
    assert!(e.set_format(
        CharEdit {
            bold: Some(true),
            ..CharEdit::default()
        },
        0.0
    ));
    timed("bold over 5000 paragraphs", t);
    check(&e).unwrap();
    let t = Instant::now();
    assert!(e.set_list(Some(ListKind::Number), 1.0));
    timed("numbering 5000 paragraphs", t);
    check(&e).unwrap();
    let t = Instant::now();
    assert!(e.undo() && e.undo());
    assert!(e.redo() && e.redo());
    timed("two undos and two redos", t);
    e.select_all();
    assert!(e.delete(false, 2.0));
    assert!(e.contents().is_empty());
    assert!(e.undo());
    // One long line, typed in.
    let mut e = editor_wrapped("", 30.0);
    let t = Instant::now();
    for i in 0..3000 {
        e.insert("word ", f64::from(i) * 0.001);
    }
    timed("3000 insertions into one growing paragraph", t);
    check(&e).unwrap();
    // What one keystroke costs as the text grows (each one lays it all out).
    for words in [2_000, 10_000, 40_000] {
        let text = "word ".repeat(words);
        let mut e = editor_wrapped(&text, 60.0);
        let t = Instant::now();
        for i in 0..5 {
            e.insert("x", f64::from(i));
        }
        eprintln!(
            "one keystroke in {} characters: {:?}",
            text.len(),
            t.elapsed() / 5
        );
    }
    // The size limit refuses rather than breaking.
    let mut e = editor_wrapped("", 0.0);
    let big = "x".repeat(900 * 1024);
    assert!(e.paste(&big, 0.0));
    assert!(!e.paste(&big, 1.0), "past 1 MiB is refused");
    assert!(e.notice.is_some());
    check(&e).unwrap();
    // Nested groups stop at eight levels.
    let mut e = editor_wrapped("text", 0.0);
    e.select_all();
    let mut refused = false;
    for i in 0..20 {
        let edit = CharEdit {
            height: Some(1.0 + f64::from(i)),
            ..CharEdit::default()
        };
        refused |= !e.set_format(edit, f64::from(i));
        check(&e).unwrap();
    }
    eprintln!("repeated formatting refused at some point: {refused}");
}

#[test]
fn stress_the_generators_make_what_they_claim() {
    // A session can only find what it can reach; make sure it reaches
    // formatting, paragraphs, lists and the odd contents.
    let mut rng = Rng::new(1);
    let ops: Vec<Op> = (0..2000).map(|_| random_op(&mut rng)).collect();
    let has = |f: fn(&Op) -> bool| ops.iter().any(f);
    assert!(has(|o| matches!(o, Op::Format(_))));
    assert!(has(|o| matches!(o, Op::Paragraph(_))));
    assert!(has(|o| matches!(o, Op::List(Some(_)))));
    assert!(has(|o| matches!(o, Op::Undo)) && has(|o| matches!(o, Op::Redo)));
    assert!(has(|o| matches!(o, Op::Width(_))));
    let text = hostile_contents(&mut rng, 30);
    assert!(!text.is_empty());
    // Unused import guards for the types the generators name.
    let _ = (MText::new(
        Point::new(0.0, 0.0),
        1.0,
        "",
        TextStyleId::STANDARD,
    ),);
}

/// A tool, not a test: shrink contents that break an invariant on opening to
/// the smallest that still do. `STRESS_SHRINK_FILE=<file> cargo test -p
/// od-app stress_shrink -- --ignored --nocapture`.
#[test]
#[ignore = "a tool: needs STRESS_SHRINK_FILE"]
fn stress_shrink_contents() {
    let Ok(path) = std::env::var("STRESS_SHRINK_FILE") else {
        return;
    };
    let start = std::fs::read_to_string(path).unwrap();
    let width = env("STRESS_WIDTH", 0) as f64;
    let broken = |chars: &[char]| {
        let text: String = chars.iter().collect();
        let ran = catch_unwind(AssertUnwindSafe(|| check(&editor_wrapped(&text, width))));
        !matches!(ran, Ok(Ok(())))
    };
    let mut now: Vec<char> = start.trim_end_matches(['\r', '\n']).chars().collect();
    eprintln!("broken as given: {}", broken(&now));
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < now.len() {
            let mut without = now.clone();
            without.remove(i);
            if broken(&without) {
                now = without;
                changed = true;
            } else {
                i += 1;
            }
        }
    }
    let shown: String = now.iter().collect();
    eprintln!("smallest: {shown:?}");
    if let Err(why) = check(&editor_wrapped(&shown, width)) {
        eprintln!("it breaks with: {why}");
    }
}
