use super::*;
use crate::overkill_line::overkill_prompt;
use crate::state::AppState;
use od_commands::{Reply, Wants};
use od_core::dimension::{Assoc, AssocPoint};
use od_core::{Dimension, Point};

fn line(x1: f64, x2: f64) -> Entity {
    Entity::Line {
        start: Point::new(x1, 0.0),
        end: Point::new(x2, 0.0),
    }
}

fn entities(state: &AppState) -> Vec<Entity> {
    let objects = state.session().document().objects();
    objects.iter().map(|o| o.entity.clone()).collect()
}

fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

/// Run the dialog command on everything, answering the dialog with `settings`.
fn killing_with(state: &mut AppState, settings: Settings) {
    state.select_all();
    state.start_command("Overkill", overkill_objects);
    state.settle_command();
    let ask = state.waiting().expect("the dialog is asked for");
    assert_eq!(ask.wants, Wants::OverkillDialog);
    state.reply(Reply::Overkill(settings));
    state.settle_command();
}

fn killing(items: &[Entity]) -> AppState {
    let mut state = AppState::new();
    for e in items {
        state.add_entity(e.clone());
    }
    killing_with(&mut state, Settings::default());
    state
}

#[test]
fn duplicates_go_and_overlaps_merge_in_one_undo_step() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = killing(&[
        line(0.0, 6.0),
        line(0.0, 6.0),
        line(4.0, 10.0),
        line(20.0, 21.0),
    ]);
    assert_eq!(entities(&state), vec![line(0.0, 10.0), line(20.0, 21.0)]);
    state.undo();
    assert_eq!(entities(&state).len(), 4);
    state.redo();
    assert_eq!(entities(&state).len(), 2);
}

#[test]
fn nothing_to_tidy_changes_nothing_and_adds_no_undo_step() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = killing(&[line(0.0, 1.0), line(5.0, 6.0)]);
    assert_eq!(entities(&state).len(), 2);
    assert!(said(&state, "No duplicate or overlapping objects found."));
    state.undo();
    assert_eq!(
        entities(&state).len(),
        1,
        "that undid an add, not an overkill"
    );
}

#[test]
fn running_it_again_finds_nothing() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = killing(&[line(0.0, 6.0), line(0.0, 6.0), line(4.0, 10.0)]);
    killing_with(&mut state, Settings::default());
    assert!(said(&state, "No duplicate or overlapping objects found."));
}

#[test]
fn objects_on_a_locked_layer_are_left_alone() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = AppState::new();
    state.add_entity(line(0.0, 6.0));
    state.add_entity(line(0.0, 6.0));
    let ids = state.session().document().objects().iter().map(|o| o.id);
    let ids: Vec<_> = ids.collect();
    let mut layer = state.session().document().layers()[0].clone();
    layer.locked = true;
    state.perform(od_commands::ModifyLayers::new("Lock", vec![layer]));
    let active = state.active;
    state.docs[active].selection = ids.into_iter().collect();
    state.start_command("Overkill", overkill_objects);
    state.settle_command();
    state.reply(Reply::Overkill(Settings::default()));
    state.settle_command();
    assert_eq!(entities(&state).len(), 2);
    assert!(said(
        &state,
        "2 objects on locked or hidden layers left alone."
    ));
}

/// Two identical lines, the second with a dimension tied to it.
fn tied_pair() -> AppState {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 6.0));
    state.add_entity(line(0.0, 6.0));
    let second = state.session().document().objects()[1].id;
    let mut dimension = Dimension::aligned(
        Point::new(0.0, 0.0),
        Point::new(6.0, 0.0),
        Point::new(0.0, 3.0),
        &od_core::DimStyle::default(),
    );
    dimension.assoc = vec![Assoc {
        point: 0,
        target: second,
        at: AssocPoint::Start,
    }];
    state.add_entity(Entity::Dimension(Box::new(dimension)));
    state
}

#[test]
fn an_object_a_dimension_is_tied_to_is_kept_and_its_copy_goes() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = tied_pair();
    let second = state.session().document().objects()[1].id;
    killing_with(&mut state, Settings::default());
    let doc = state.session().document();
    assert_eq!(doc.objects().len(), 2, "the first line went");
    assert!(doc.contains(second));
    assert!(said(
        &state,
        "that other objects are tied to were not deleted"
    ));
}

#[test]
fn without_maintaining_associative_objects_the_first_survives_as_usual() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = tied_pair();
    let second = state.session().document().objects()[1].id;
    let settings = Settings {
        maintain_associative: false,
        ..Settings::default()
    };
    killing_with(&mut state, settings);
    assert!(!state.session().document().contains(second));
}

#[test]
fn a_bad_tolerance_does_nothing_and_says_so() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = AppState::new();
    state.add_entity(line(0.0, 6.0));
    state.add_entity(line(0.0, 6.0));
    let settings = Settings {
        tolerance: f64::NAN,
        ..Settings::default()
    };
    killing_with(&mut state, settings);
    assert_eq!(entities(&state).len(), 2);
    assert!(said(
        &state,
        "OVERKILL did nothing: the tolerance must be zero or more."
    ));
}

#[test]
fn the_command_line_form_takes_options_and_shares_the_settings() {
    let _turn = crate::overkill_memory::take_turn_for_tests();
    let mut state = AppState::new();
    state.add_entity(line(0.0, 6.0));
    state.add_entity(line(0.0, 6.0));
    state.add_entity(line(4.0, 10.0));
    state.select_all();
    state.start_command("-OVERKILL", overkill_prompt);
    state.settle_command();
    // Turn partial overlaps off: only the duplicate goes.
    for answer in ["C", "N"] {
        state.cmd_submit(answer);
        state.settle_command();
    }
    state.answer_enter();
    state.settle_command();
    assert_eq!(entities(&state), vec![line(0.0, 6.0), line(4.0, 10.0)]);
}
