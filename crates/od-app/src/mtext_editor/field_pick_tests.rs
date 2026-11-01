//! Picking an object for a field while the text editor is open.

use od_core::{Entity, EntityId, Point};

use crate::mtext::draw_mtext;
use crate::state::AppState;

/// A drawing with a circle (radius 2) and the editor open on a new text.
fn editing() -> (AppState, EntityId) {
    let mut state = AppState::new();
    state.add_entity(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 2.0,
    });
    let circle = state
        .session()
        .document()
        .objects()
        .last()
        .map(|o| o.id)
        .unwrap();
    state.start_command("MTEXT", draw_mtext);
    state.settle_command();
    for input in ["10,50", "50,20"] {
        state.cmd_submit(input);
        state.settle_command();
    }
    state.sync_mtext_editor();
    assert!(state.mtext_editing());
    (state, circle)
}

fn editor(state: &mut AppState) -> &mut super::MTextEditor {
    let active = state.active;
    state.docs[active].mtext_editor.as_mut().unwrap()
}

#[test]
fn while_picking_the_canvas_takes_an_object_click_and_afterwards_it_does_not() {
    let (mut state, _) = editing();
    let active = state.active;
    assert!(!state.docs[active].view.is_prompting());
    editor(&mut state).field_pick = true;
    state.pump_command();
    assert!(
        state.docs[active].view.is_prompting(),
        "clicks are answers now"
    );
    editor(&mut state).field_pick = false;
    state.pump_command();
    assert!(!state.docs[active].view.is_prompting());
}

#[test]
fn a_click_on_an_object_hands_it_to_the_dialog_and_ends_the_pick() {
    let (mut state, circle) = editing();
    {
        let e = editor(&mut state);
        e.field_dialog = true;
        e.field_pick = true;
    }
    state.answer_field_pick(None);
    assert!(
        editor(&mut state).field_pick,
        "a click on nothing keeps waiting"
    );
    state.answer_field_pick(Some(circle));
    let e = editor(&mut state);
    assert!(!e.field_pick && e.field_dialog, "the dialog is still open");
    let choice = e.field_object.take().expect("the object is handed over");
    assert_eq!(choice.id, circle);
    assert!(choice.properties.iter().any(|(_, v)| !v.is_empty()));
}

#[test]
fn a_click_when_not_picking_is_not_taken() {
    let (mut state, circle) = editing();
    state.answer_field_pick(Some(circle));
    assert!(editor(&mut state).field_object.is_none());
}

#[test]
fn an_object_field_made_from_the_pick_shows_its_value_in_the_text() {
    use od_core::block::{DataField, FieldStatus, FieldValue, ObjectProperty};
    let (mut state, circle) = editing();
    {
        let e = editor(&mut state);
        e.field_pick = true;
    }
    state.answer_field_pick(Some(circle));
    let choice = editor(&mut state).field_object.take().unwrap();
    let (_, shown) = choice
        .properties
        .iter()
        .find(|(p, _)| *p == ObjectProperty::Radius)
        .cloned()
        .unwrap();
    // What the dialog host does with the pick.
    let mut field = DataField::object_property(circle, ObjectProperty::Radius);
    field.cached = Some(FieldValue::Text(shown.clone()));
    field.status = FieldStatus::Success;
    assert!(editor(&mut state).insert_field(field, 0.0));
    assert_eq!(editor(&mut state).text().field_values(), [shown]);
    state.finish_mtext_editor(super::Outcome::Accept);
    state.settle_command();
    let made = state
        .session()
        .document()
        .objects()
        .iter()
        .find_map(|o| match &o.entity {
            Entity::MText(t) => Some((**t).clone()),
            _ => None,
        })
        .expect("the text was made");
    assert_eq!(made.fields.len(), 1);
    assert_eq!(
        made.fields[0].object_refs,
        [od_core::block::FieldObjectRef::Entity(circle)]
    );
}
