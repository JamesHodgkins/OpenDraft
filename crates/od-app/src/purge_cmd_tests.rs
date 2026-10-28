use super::*;
use crate::purge_line::purge_prompt;
use crate::state::AppState;
use od_core::{Attr, Attributes, Entity, LayerId, Point};

fn line(x1: f64, x2: f64) -> Entity {
    Entity::Line {
        start: Point::new(x1, 0.0),
        end: Point::new(x2, 0.0),
    }
}

fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

fn run(state: &mut AppState, purge: Purge) {
    state.start_command("PURGE", move |ed| purge_run(ed, purge));
    state.settle_command();
}

fn linetype_count(state: &AppState) -> usize {
    state.session().document().linetypes().records().len()
}

fn answer(state: &mut AppState, answers: &[&str]) {
    for a in answers {
        state.cmd_submit(a);
        state.settle_command();
    }
}

#[test]
fn purging_everything_named_is_one_undo_step() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 5.0));
    let before = linetype_count(&state);
    run(&mut state, Purge::default());
    assert!(linetype_count(&state) < before);
    assert_eq!(state.session().document().objects().len(), 1);
    state.undo();
    assert_eq!(linetype_count(&state), before);
}

#[test]
fn unnamed_objects_go_only_when_asked_for() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 5.0));
    state.add_entity(line(1.0, 1.0));
    run(&mut state, Purge::default());
    assert_eq!(state.session().document().objects().len(), 2);
    run(
        &mut state,
        Purge {
            only: Some(Vec::new()),
            zero_length: true,
            ..Purge::default()
        },
    );
    assert_eq!(state.session().document().objects().len(), 1);
    assert!(said(&state, "Deleting 1 object of zero-length geometry."));
}

#[test]
fn what_a_drawing_uses_is_kept() {
    let mut state = AppState::new();
    let mut attrs = Attributes::on_layer(LayerId::DEFAULT);
    attrs.linetype = Attr::Set(od_core::LinetypeId::DASHED);
    state.perform(od_commands::AddCopies::new(
        "Add",
        vec![(line(0.0, 5.0), attrs)],
    ));
    run(&mut state, Purge::default());
    let doc = state.session().document();
    assert!(doc.linetypes().named("DASHED").is_some());
    assert!(doc.linetypes().named("DOT").is_none());
    run(&mut state, Purge::default());
    assert!(said(&state, "Nothing to purge."));
}

#[test]
fn purging_chosen_items_leaves_the_rest() {
    let mut state = AppState::new();
    run(
        &mut state,
        Purge {
            only: Some(vec![Item::Linetype(od_core::LinetypeId::DOT)]),
            ..Purge::default()
        },
    );
    let doc = state.session().document();
    assert!(doc.linetypes().named("DOT").is_none());
    assert!(doc.linetypes().named("DASHED").is_some());
}

#[test]
fn confirming_each_item_deletes_only_those_answered_yes() {
    let mut state = AppState::new();
    let only = Some(vec![
        Item::Linetype(od_core::LinetypeId::DASHED),
        Item::Linetype(od_core::LinetypeId::DOT),
        Item::Linetype(od_core::LinetypeId::HIDDEN),
    ]);
    run(
        &mut state,
        Purge {
            only,
            confirm: true,
            ..Purge::default()
        },
    );
    answer(&mut state, &["Y", "N", "A"]);
    let doc = state.session().document();
    assert!(doc.linetypes().named("DASHED").is_none());
    assert!(doc.linetypes().named("DOT").is_some(), "answered No");
    assert!(doc.linetypes().named("HIDDEN").is_none(), "All");
    assert!(doc.linetypes().named("CENTER").is_some(), "not offered");
}

#[test]
fn escape_while_confirming_changes_nothing() {
    let mut state = AppState::new();
    let before = linetype_count(&state);
    run(
        &mut state,
        Purge {
            nested: true,
            confirm: true,
            ..Purge::default()
        },
    );
    state.cancel_command();
    assert_eq!(linetype_count(&state), before);
}

#[test]
fn the_command_line_asks_the_kind_then_the_names_then_whether_to_verify() {
    let mut state = AppState::new();
    state.start_command("-PURGE", purge_prompt);
    state.settle_command();
    answer(&mut state, &["LT", "DA*,HID*", "N"]);
    let doc = state.session().document();
    assert!(doc.linetypes().named("DOT").is_some(), "not named");
    assert!(doc.linetypes().named("DASHED").is_none());
    assert!(doc.linetypes().named("DASHDOT").is_none());
    assert!(doc.linetypes().named("HIDDEN").is_none());
    assert!(doc.linetypes().named("CENTER").is_some());
}

#[test]
fn the_command_line_says_why_a_named_item_stays_and_what_it_does_not_have() {
    let mut state = AppState::new();
    state.start_command("-PURGE", purge_prompt);
    state.settle_command();
    answer(&mut state, &["LA", "0"]);
    assert!(said(&state, "\"0\" cannot be purged: Layer 0 is permanent"));
    assert!(said(&state, "Nothing to purge."));

    state.start_command("-PURGE", purge_prompt);
    state.settle_command();
    answer(&mut state, &["R"]);
    assert!(said(&state, "OpenDraft does not store them."));
}

#[test]
fn the_command_line_without_verifying_takes_one_level_and_all_takes_names_only() {
    let mut state = AppState::new();
    state.add_entity(line(1.0, 1.0));
    state.start_command("-PURGE", purge_prompt);
    state.settle_command();
    answer(&mut state, &["A", "N"]);
    assert_eq!(
        state.session().document().objects().len(),
        1,
        "All leaves unnamed objects"
    );
    state.start_command("-PURGE", purge_prompt);
    state.settle_command();
    answer(&mut state, &["Z", "N"]);
    assert!(state.session().document().objects().is_empty());
}

#[test]
fn the_dialog_opens_with_nothing_ticked_and_keeps_ticks_when_it_refreshes() {
    let mut state = AppState::new();
    state.open_purge();
    assert!(!state.purge.items.is_empty());
    assert!(state.purge.items.iter().all(|(_, on)| !on));
    assert!(
        !state.purge.blocked.is_empty(),
        "layer 0 stays, with a reason"
    );
    state.purge.items[0].1 = true;
    let ticked = state.purge.items[0].0.item.clone();
    state.refresh_purge();
    let again = state.purge.items.iter().find(|(p, _)| p.item == ticked);
    assert!(again.is_some_and(|(_, on)| *on));
    assert_eq!(state.purge.items.iter().filter(|(_, on)| *on).count(), 1);
}

#[test]
fn a_block_has_a_picture_and_an_unknown_one_does_not() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 5.0));
    state.select_all();
    state.start_command("BLOCK", crate::block_cmd::define_block);
    state.settle_command();
    answer(&mut state, &["Tick", "0,0"]);
    let doc = state.session().document();
    let id = doc.blocks()[0].id;
    let picture = block_picture(doc, id);
    assert!(picture.is_some_and(|p| p.width > 0 && p.height > 0));
    assert!(block_picture(doc, od_core::BlockId(9_999)).is_none());
}
