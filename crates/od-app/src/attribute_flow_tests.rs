//! Attributes from end to end through the commands: define, make a block,
//! insert it and be asked for the values, explode it.

use od_core::Entity;

use crate::block_cmd::define_block;
use crate::explode::explode_objects;
use crate::insert_cmd::insert_block;
use crate::state::AppState;

fn submit_all(state: &mut AppState, answers: &[&str]) {
    for answer in answers {
        state.cmd_submit(answer);
        state.settle_command();
    }
}

fn run(
    state: &mut AppState,
    name: &str,
    command: fn(&mut od_commands::Editor) -> Result<(), od_commands::Cancelled>,
    a: &[&str],
) {
    state.start_command(name, command);
    state.settle_command();
    submit_all(state, a);
}

/// A drawing whose only object is an insert of block "Tag", made from a line
/// and the attribute NAME (default "n/a"), plus the same attribute again.
fn with_tag_block(modes: &[&str]) -> AppState {
    let mut state = AppState::new();
    state.add_entity(Entity::Line {
        start: od_core::Point::new(0.0, 0.0),
        end: od_core::Point::new(4.0, 0.0),
    });
    let mut answers: Vec<&str> = modes.to_vec();
    answers.extend(["", "name", "", "n/a", "0,1", "1", "0"]);
    run(
        &mut state,
        "ATTDEF",
        crate::attdef_cmd::define_attribute,
        &answers,
    );
    assert!(!state.command_running());
    state.select_all();
    run(&mut state, "BLOCK", define_block, &["Tag", "0,0"]);
    assert!(!state.command_running());
    state
}

fn last_insert_values(state: &AppState) -> Vec<String> {
    match state
        .session()
        .document()
        .objects()
        .last()
        .map(|o| &o.entity)
    {
        Some(Entity::Insert(i)) => i.attributes.iter().map(|r| r.value.display()).collect(),
        other => panic!("the last object is not an insert: {other:?}"),
    }
}

#[test]
fn a_block_made_from_a_definition_asks_for_its_value_when_inserted() {
    let mut state = with_tag_block(&[]);
    assert_eq!(
        last_insert_values(&state),
        ["n/a"],
        "BLOCK leaves the default"
    );
    run(&mut state, "INSERT", insert_block, &["tag", "10,10", "Bob"]);
    assert!(!state.command_running());
    assert_eq!(last_insert_values(&state), ["Bob"]);
    assert_eq!(state.session().document().validate(), Ok(()));
    state.undo();
    assert_eq!(state.session().document().len(), 1, "one undo step");
}

#[test]
fn enter_takes_the_default_and_attreq_off_does_not_ask() {
    let mut state = with_tag_block(&[]);
    run(&mut state, "INSERT", insert_block, &["tag", "10,10", ""]);
    assert_eq!(last_insert_values(&state), ["n/a"]);
    state
        .defaults()
        .attribute
        .change(|m| m.prompt_on_insert = false);
    run(&mut state, "INSERT", insert_block, &["tag", "20,10"]);
    assert!(!state.command_running(), "no value was asked for");
    assert_eq!(last_insert_values(&state), ["n/a"]);
}

#[test]
fn a_preset_attribute_is_not_asked_and_a_verified_one_is() {
    let mut state = with_tag_block(&["P"]);
    run(&mut state, "INSERT", insert_block, &["tag", "10,10"]);
    assert!(!state.command_running(), "preset is not asked");
    let mut state = with_tag_block(&["V"]);
    run(
        &mut state,
        "INSERT",
        insert_block,
        &["tag", "10,10", "wrong", "N", "right", "Y"],
    );
    assert!(!state.command_running());
    assert_eq!(last_insert_values(&state), ["right"]);
}

#[test]
fn escaping_at_a_value_inserts_nothing() {
    let mut state = with_tag_block(&[]);
    run(&mut state, "INSERT", insert_block, &["tag", "10,10"]);
    assert!(state.command_running(), "waiting for the value");
    state.cancel_command();
    assert_eq!(state.session().document().len(), 1);
}

#[test]
fn exploding_an_insert_leaves_its_values_as_text() {
    let mut state = with_tag_block(&[]);
    run(&mut state, "INSERT", insert_block, &["tag", "10,10", "Bob"]);
    let last = state.session().document().objects().last().map(|o| o.id);
    state.select_ids(last, false);
    run(&mut state, "EXPLODE", explode_objects, &[]);
    let texts: Vec<String> = state
        .session()
        .document()
        .objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::MText(m) => Some(m.contents.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Bob"]);
    assert_eq!(state.session().document().validate(), Ok(()));
}

#[test]
fn copies_of_one_definition_get_their_own_ids_in_a_block() {
    let mut state = AppState::new();
    run(
        &mut state,
        "ATTDEF",
        crate::attdef_cmd::define_attribute,
        &["", "one", "", "", "0,0", "1", "0"],
    );
    let copy = state.session().document().objects()[0].entity.clone();
    state.add_entity(copy.translated(0.0, 3.0));
    state.select_all();
    run(&mut state, "BLOCK", define_block, &["Twins", "0,0"]);
    let doc = state.session().document();
    assert_eq!(doc.validate(), Ok(()));
    let block = doc.block_named("Twins").unwrap();
    let ids: Vec<u64> = block.attribute_definitions().map(|(_, d)| d.id.0).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}
