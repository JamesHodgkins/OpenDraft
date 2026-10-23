use od_core::Point;

use super::*;
use crate::explode::explode_objects;
use crate::state::AppState;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn line(a: Point, b: Point) -> Entity {
    Entity::Line { start: a, end: b }
}

fn submit_all(state: &mut AppState, answers: &[&str]) {
    for answer in answers {
        state.cmd_submit(answer);
        state.settle_command();
    }
}

/// Run BLOCK on everything in the drawing with these answers.
fn block_all(state: &mut AppState, answers: &[&str]) {
    state.select_all();
    state.start_command("BLOCK", define_block);
    state.settle_command();
    submit_all(state, answers);
}

fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

#[test]
fn block_turns_the_selection_into_one_reference_in_one_undo_step() {
    let mut state = AppState::new();
    state.add_entity(line(p(0.0, 0.0), p(4.0, 0.0)));
    state.add_entity(line(p(4.0, 0.0), p(4.0, 3.0)));
    block_all(&mut state, &["Corner", "4,0"]);
    assert!(!state.command_running());
    let doc = &state.session().document();
    assert_eq!(doc.blocks().len(), 1);
    assert_eq!(doc.len(), 1);
    let Entity::Insert(i) = &doc.objects()[0].entity else {
        panic!("the objects became a reference")
    };
    assert_eq!(i.insertion, p(4.0, 0.0));
    assert_eq!(
        i.picture(),
        [
            line(p(0.0, 0.0), p(4.0, 0.0)),
            line(p(4.0, 0.0), p(4.0, 3.0))
        ]
    );
    assert!(said(&state, "Block \"Corner\" defined with 2 objects."));
    assert_eq!(doc.validate(), Ok(()));

    state.undo();
    let doc = &state.session().document();
    assert!(doc.blocks().is_empty() && doc.len() == 2);
}

#[test]
fn bad_names_are_refused_and_the_list_option_lists() {
    let mut state = AppState::new();
    state.add_entity(line(p(0.0, 0.0), p(1.0, 0.0)));
    block_all(&mut state, &["a*b", "?", "Good", "0,0"]);
    assert!(said(&state, "cannot contain"));
    assert!(said(&state, "No blocks are defined"));
    assert_eq!(state.session().document().blocks()[0].name, "Good");
}

#[test]
fn redefining_asks_first_and_updates_every_reference() {
    let mut state = AppState::new();
    state.add_entity(line(p(0.0, 0.0), p(1.0, 0.0)));
    block_all(&mut state, &["Mark", "0,0"]);
    // A new circle, redefined as the block: the old reference follows.
    state.add_entity(Entity::Circle {
        center: p(20.0, 0.0),
        radius: 1.0,
    });
    let circle = state.session().document().objects()[1].id;
    state.docs[state.active].selection.clear();
    state.select_ids([circle], false);
    state.start_command("BLOCK", define_block);
    state.settle_command();
    // "No" asks for a name again.
    submit_all(&mut state, &["mark", "N", "MARK", "Y", "20,0"]);
    assert!(!state.command_running());
    let doc = &state.session().document();
    assert_eq!(doc.blocks().len(), 1, "redefined, not added");
    assert!(said(&state, "Block \"Mark\" redefined with 1 object."));
    for object in doc.objects() {
        let Entity::Insert(i) = &object.entity else {
            panic!("only references are left")
        };
        assert!(matches!(i.picture()[..], [Entity::Circle { .. }]));
    }
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn a_block_cannot_be_redefined_to_hold_itself() {
    let mut state = AppState::new();
    state.add_entity(line(p(0.0, 0.0), p(1.0, 0.0)));
    block_all(&mut state, &["Loop", "0,0"]);
    block_all(&mut state, &["Loop", "Y", "0,0"]);
    assert!(said(&state, "cannot contain an insert of itself"));
    assert_eq!(state.session().document().blocks()[0].entities.len(), 1);
}

#[test]
fn exploding_a_reference_gives_back_its_objects_and_purge_removes_the_block() {
    let mut state = AppState::new();
    state.add_entity(line(p(0.0, 0.0), p(1.0, 0.0)));
    block_all(&mut state, &["Tick", "0,0"]);
    state.start_command("PURGE", purge_blocks);
    state.settle_command();
    assert!(said(&state, "No unreferenced blocks found."));

    state.select_all();
    state.start_command("Explode", explode_objects);
    state.settle_command();
    let doc = &state.session().document();
    assert_eq!(doc.objects()[0].entity, line(p(0.0, 0.0), p(1.0, 0.0)));
    assert_eq!(doc.blocks().len(), 1, "exploding keeps the definition");

    state.start_command("PURGE", purge_blocks);
    state.settle_command();
    assert!(said(&state, "Deleting block \"Tick\"."));
    assert!(state.session().document().blocks().is_empty());
}

#[test]
fn purge_follows_blocks_used_only_inside_unused_ones() {
    let mut state = AppState::new();
    state.add_entity(line(p(0.0, 0.0), p(1.0, 0.0)));
    block_all(&mut state, &["Inner", "0,0"]);
    block_all(&mut state, &["Outer", "0,0"]);
    let doc = &state.session().document();
    assert!(unused_blocks(doc).is_empty());
    let mut bare = (*doc).clone();
    let ids: Vec<_> = bare.objects().iter().map(|o| o.id).collect();
    for id in ids {
        bare.remove(id);
    }
    let names: Vec<_> = unused_blocks(&bare)
        .iter()
        .map(|b| b.name.clone())
        .collect();
    assert_eq!(names, ["Inner", "Outer"]);
}
