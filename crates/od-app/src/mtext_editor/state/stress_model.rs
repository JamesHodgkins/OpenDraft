//! Editing sessions as a user does them, through the real input layer and
//! checked against a plain string model: moving the caret about, selecting
//! with Shift, chopping sections out and pasting them elsewhere, Tab, Enter,
//! Backspace and Delete, typing over a selection. After every step the text,
//! the selection and the clipboard must be what the model says.
//!
//! The text is made of letters that cannot join into one cluster (no `f`
//! next to `i`, no Arabic), so one caret step is one letter. Replay one
//! session with `STRESS_SEED=<n>`.

use od_core::text::plain_text_of;

use super::stress_ops::Rng;
use super::stress_steps::{free_step, letters_before, step, Free, Model};
use super::tests::editor_wrapped;
use super::MTextEditor;
use crate::mtext_editor::input::handle_events;

fn session(seed: u64, steps: usize, width: f64) -> Result<(), String> {
    let mut rng = Rng::new(seed);
    let mut e = editor_wrapped("", width);
    let mut m = Model {
        text: Vec::new(),
        anchor: 0,
        active: 0,
        clipboard: String::new(),
    };
    let mut trace: Vec<String> = Vec::new();
    // What the editor shows: the letters and the selection as letter positions.
    let state = |e: &MTextEditor| {
        let range = e.selected_range();
        let at = |byte: usize| plain_text_of(e.contents(), 0..byte).chars().count();
        (
            plain_text_of(e.contents(), 0..e.contents().len()),
            (at(range.start), at(range.end)),
        )
    };
    for i in 0..steps {
        let before = state(&e);
        // A second apart, so no two edits share an undo step.
        let now = i as f64;
        if rng.chance(20) {
            let (kind, name) = free_step(&mut rng, &mut e, now);
            trace.push(name);
            let (shown, got) = state(&e);
            let old: String = m.text.iter().collect();
            match kind {
                Free::Moved if shown != old => {
                    return fail(
                        seed,
                        i,
                        &trace,
                        format!("a move changed the text to {shown:?}"),
                    );
                }
                Free::Deleted { backwards } => {
                    // The text lost one run touching the caret (or the selection).
                    let (a, b) = m.range();
                    let lost = old.chars().count() - shown.chars().count();
                    let chars: Vec<char> = old.chars().collect();
                    let keep = |from: usize, to: usize| -> String {
                        chars[..from].iter().chain(&chars[to..]).collect()
                    };
                    let ok = if m.has_selection() {
                        shown == keep(a, b)
                    } else if backwards {
                        (lost > 0 || a == 0) && lost <= a && shown == keep(a - lost, a)
                    } else {
                        (lost > 0 || a == chars.len())
                            && a + lost <= chars.len()
                            && shown == keep(a, a + lost)
                    };
                    if !ok {
                        return fail(
                            seed,
                            i,
                            &trace,
                            format!("a word deletion gave {shown:?} from {old:?} at {a}..{b}"),
                        );
                    }
                }
                Free::Moved => {}
            }
            // Take the editor's word for where it put the caret.
            m.text = shown.chars().collect();
            let (anchor, active) = e.selection();
            (m.anchor, m.active) = (letters_before(&e, anchor), letters_before(&e, active));
            if state(&e).1 != m.range() {
                return fail(
                    seed,
                    i,
                    &trace,
                    format!(
                        "selection {:?}, caret indexes say {:?}",
                        state(&e).1,
                        m.range()
                    ),
                );
            }
            let _ = got;
            continue;
        }
        let (events, name) = step(&mut rng, &mut m, width > 0.0);
        trace.push(name);
        let done = handle_events(&mut e, &events, now);
        // A copy or cut hands its text back for the clipboard.
        if let Some(copied) = done.copied {
            if copied != m.clipboard {
                return fail(
                    seed,
                    i,
                    &trace,
                    format!("clipboard {copied:?}, wanted {:?}", m.clipboard),
                );
            }
        }
        let (shown, got) = state(&e);
        let want: String = m.text.iter().collect();
        if shown != want {
            return fail(seed, i, &trace, format!("text {shown:?}, wanted {want:?}"));
        }
        if got != m.range() {
            return fail(
                seed,
                i,
                &trace,
                format!("selection {got:?}, wanted {:?}", m.range()),
            );
        }
        // An edit is one undo step: back goes the text and the selection,
        // and forward again brings both.
        if shown != before.0 {
            if !e.undo() || state(&e) != before {
                return fail(
                    seed,
                    i,
                    &trace,
                    format!("undo gave {:?}, wanted {before:?}", state(&e)),
                );
            }
            if !e.redo() || state(&e) != (shown.clone(), got) {
                return fail(
                    seed,
                    i,
                    &trace,
                    format!("redo gave {:?}, wanted {:?}", state(&e), (shown, got)),
                );
            }
        }
    }
    // Everything done can be undone, and redone to the same text.
    let last = e.contents().to_owned();
    while e.undo() {}
    while e.redo() {}
    if e.contents() != last {
        return fail(seed, steps, &trace, "redo did not return to the end".into());
    }
    Ok(())
}

fn fail(seed: u64, step: usize, trace: &[String], why: String) -> Result<(), String> {
    let recent: Vec<&String> = trace.iter().rev().take(8).rev().collect();
    Err(format!(
        "seed {seed} step {step}: {why}\n  last steps: {recent:?}"
    ))
}

fn run(seeds: u64, steps: usize) {
    if let Ok(one) = std::env::var("STRESS_SEED") {
        let seed = one.parse().expect("STRESS_SEED is a number");
        for width in [0.0, 12.0] {
            if let Err(why) = session(seed, steps, width) {
                panic!("{why}");
            }
        }
        return;
    }
    let mut failures = Vec::new();
    for seed in 0..seeds {
        // Wrapped text moves the caret differently on screen, not in the text.
        for width in [0.0, 12.0] {
            if let Err(why) = session(seed, steps, width) {
                failures.push(why);
            }
        }
        if failures.len() >= 3 {
            break;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn stress_user_sessions_match_a_string_model() {
    run(6, 120);
}

#[test]
#[ignore = "long: run with --ignored"]
fn stress_long_user_sessions_match_a_string_model() {
    run(300, 600);
}
