use od_core::Point;

use super::*;
use crate::block_cmd::define_block;
use crate::files::Choice;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn submit_all(state: &mut AppState, answers: &[&str]) {
    for answer in answers {
        state.cmd_submit(answer);
        state.settle_command();
    }
}

/// A drawing with block "Box" (a 2 by 1 rectangle, base point at its lower
/// left corner, (10, 10)), converted to one reference, which is selected.
fn with_box() -> AppState {
    let mut state = AppState::new();
    state.add_entity(Entity::Rectangle {
        min: p(10.0, 10.0),
        max: p(12.0, 11.0),
    });
    state.select_all();
    state.start_command("BLOCK", define_block);
    state.settle_command();
    submit_all(&mut state, &["Box", "10,10"]);
    state.select_all();
    state
}

fn reference_bounds(state: &AppState) -> od_core::Bounds {
    let doc = &state.docs[state.active];
    doc.drawing().document().objects()[0].entity.bounds()
}

fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

#[test]
fn bedit_on_a_selected_reference_shows_the_block_at_the_origin() {
    let mut state = with_box();
    state.bedit();
    assert!(state.in_block_editor());
    let doc = &state.docs[state.active];
    assert_eq!(doc.session.document().len(), 1);
    let b = doc.session.document().objects()[0].entity.bounds();
    assert_eq!((b.min, b.max), (p(0.0, 0.0), p(2.0, 1.0)));
    assert!(doc.selection.is_empty());
    assert!(doc.tab_title().ends_with("Block Editor: Box"));
}

#[test]
fn bsave_redefines_the_block_in_the_drawing_and_bclose_puts_the_drawing_back() {
    let mut state = with_box();
    let before = reference_bounds(&state);
    state.bedit();
    state.add_entity(Entity::Line {
        start: p(0.0, 0.0),
        end: p(5.0, 0.0),
    });
    assert_eq!(reference_bounds(&state), before, "not saved yet");
    assert!(state.bsave());
    assert!(said(&state, "Block \"Box\" saved."));
    assert!(state.in_block_editor(), "BSAVE keeps editing");
    assert!((reference_bounds(&state).max.x - 15.0).abs() < 1e-9);

    state.bclose();
    assert!(!state.in_block_editor(), "nothing unsaved: closes at once");
    let doc = &state.docs[state.active];
    assert_eq!(doc.selection.len(), 1, "the selection comes back");
    assert_eq!(doc.session.document().validate(), Ok(()));
    assert_eq!(state.undo().as_deref(), Some("Save block Box"));
    assert_eq!(reference_bounds(&state), before);
}

#[test]
fn bclose_asks_about_unsaved_changes_and_can_discard_them() {
    let mut state = with_box();
    let before = reference_bounds(&state);
    state.bedit();
    state.add_entity(Entity::Circle {
        center: p(0.0, 0.0),
        radius: 9.0,
    });
    state.bclose();
    assert!(state.in_block_editor(), "it waits for the answer");
    assert!(
        state.docs[state.active]
            .block_edit
            .as_ref()
            .unwrap()
            .closing
    );
    state.answer_block_close(Choice::Cancel);
    assert!(state.in_block_editor(), "Cancel stays in the Block Editor");
    assert!(
        !state.docs[state.active]
            .block_edit
            .as_ref()
            .unwrap()
            .closing
    );
    state.bclose();
    state.answer_block_close(Choice::Discard);
    assert!(!state.in_block_editor());
    assert_eq!(reference_bounds(&state), before);
}

#[test]
fn bclose_can_save_on_the_way_out() {
    let mut state = with_box();
    state.bedit();
    state.add_entity(Entity::Circle {
        center: p(0.0, 0.0),
        radius: 9.0,
    });
    state.bclose();
    state.answer_block_close(Choice::Save);
    assert!(!state.in_block_editor());
    assert!((reference_bounds(&state).min.x - 1.0).abs() < 1e-9);
}

#[test]
fn without_a_selection_bedit_asks_which_block() {
    let mut state = with_box();
    state.docs[state.active].selection.clear();
    state.bedit();
    state.settle_command();
    assert!(state.cmd_prompt().text.starts_with("Enter block name"));
    state.answer_keyword("Box");
    state.settle_command();
    assert!(state.in_block_editor());
}

#[test]
fn a_drawing_with_unsaved_block_changes_does_not_close() {
    let mut state = with_box();
    state.new_document();
    state.active = 0;
    state.bedit();
    state.add_entity(Entity::Circle {
        center: p(0.0, 0.0),
        radius: 9.0,
    });
    state.request_close(0);
    assert_eq!(state.docs.len(), 2);
    assert!(state.confirm.is_none());
    assert!(state.docs[0].block_edit.as_ref().unwrap().closing);
    assert!(!state.try_exit());
}
