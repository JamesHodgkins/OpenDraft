use super::tests::{at, click, enter, entities, fresh, key, number, p, polyline, said};
use super::*;
use crate::pedit_memory::memory;
use crate::state::AppState;
use od_commands::{ModifyAttributes, ModifyLayers, Reply};
use od_core::hatch::{loops_from_entities, BoundarySource, HatchEntity, HatchFill, HatchPattern};
use od_core::{Attributes, Point};

/// Two polylines with a gap and a crossing of their lines at (5, 0), selected
/// for Multiple.
fn gap_pair() -> AppState {
    let mut state = fresh();
    state.add_entity(polyline(&[(0.0, 0.0), (4.5, 0.0)]));
    state.add_entity(polyline(&[(5.0, -0.4), (5.0, 3.0)]));
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    state
}

#[test]
fn multiple_join_extends_ends_that_nearly_meet() {
    let mut state = gap_pair();
    key(&mut state, "Join");
    number(&mut state, 1.0);
    let now = entities(&state);
    assert_eq!(now.len(), 1);
    let Entity::Polyline { points, .. } = &now[0] else {
        panic!("a polyline");
    };
    assert_eq!(points, &vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 3.0)]);
}

#[test]
fn the_join_type_can_be_changed_and_is_remembered() {
    let mut state = gap_pair();
    key(&mut state, "Join");
    key(&mut state, "Jointype");
    key(&mut state, "Add");
    assert_eq!(memory().join_type, od_core::pedit::JoinType::Add);
    number(&mut state, 1.0);
    let Entity::Polyline { points, .. } = &entities(&state)[0] else {
        panic!("a polyline");
    };
    assert_eq!(points.len(), 4, "a segment crosses the gap");
}

#[test]
fn without_a_fuzz_distance_the_gap_stays() {
    let mut state = gap_pair();
    key(&mut state, "Join");
    enter(&mut state);
    assert_eq!(entities(&state).len(), 2);
}

/// A drawing whose second object is on a second layer, which may be locked.
fn two_layers(lock: bool) -> AppState {
    let mut state = fresh();
    state.add_entity(polyline(&[(0.0, 0.0), (4.0, 0.0)]));
    state.add_entity(polyline(&[(4.0, 0.0), (4.0, 3.0)]));
    let other = state.new_layer();
    let second = state.session().document().objects()[1].id;
    state.perform(ModifyAttributes::new(
        "To other",
        vec![(second, Attributes::on_layer(other))],
    ));
    if lock {
        let mut layer = state.session().document().layer(other).cloned().unwrap();
        layer.locked = true;
        state.perform(ModifyLayers::new("Lock", vec![layer]));
    }
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    click(&mut state, 0);
    key(&mut state, "Join");
    let ids = vec![state.session().document().objects()[1].id];
    state.reply(Reply::Selection(ids));
    state.settle_command();
    state
}

#[test]
fn join_reports_objects_whose_properties_were_dropped() {
    let state = two_layers(false);
    assert_eq!(entities(&state).len(), 1);
    assert!(said(&state, "1 joined object had other properties"));
}

#[test]
fn an_object_on_a_locked_layer_is_not_joined() {
    let state = two_layers(true);
    assert_eq!(entities(&state).len(), 2);
    assert!(said(&state, "0 segments added to polyline."));
}

#[test]
fn an_associative_hatch_follows_a_vertex_moved_by_pedit() {
    let mut state = fresh();
    let square = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    state.add_entity(square.clone());
    let boundary = state.session().document().objects()[0].id;
    let (mut loops, _) = loops_from_entities(&[square], 1e-6).unwrap();
    loops[0].sources = vec![BoundarySource::Entity(boundary)];
    let pattern = HatchPattern::user_defined(1.0, 0.0, false, Point::default()).unwrap();
    let mut hatch = HatchEntity::new(loops, HatchFill::Pattern(pattern));
    hatch.associative = true;
    state.add_entity(Entity::Hatch(Box::new(hatch)));
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    click(&mut state, 0);
    key(&mut state, "Edit vertex");
    key(&mut state, "Next");
    key(&mut state, "Next");
    key(&mut state, "Move");
    at(&mut state, 9.0, 9.0);
    let Entity::Hatch(h) = &entities(&state)[1] else {
        panic!("a hatch");
    };
    assert_eq!(h.bounds().unwrap().max, p(9.0, 9.0));
    key(&mut state, "eXit");
    key(&mut state, "Undo");
    let Entity::Hatch(h) = &entities(&state)[1] else {
        panic!("a hatch");
    };
    assert_eq!(h.bounds().unwrap().max, p(4.0, 4.0), "one undo, both back");
}
