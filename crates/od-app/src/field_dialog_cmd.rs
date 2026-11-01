//! FIELD: the Field dialog, then a new multiline text that holds the field,
//! as AutoCAD does when no text is being edited. Selecting an object for an
//! object field closes the dialog for the pick and opens it again with the
//! object. `-FIELD` (in `field_cmd.rs`) puts a field in an attribute from
//! command-line prompts.

use od_commands::{Answer, Cancelled, Editor, FieldAnswer, Options, Picked};
use od_core::block::{FieldDraft, ObjectChoice};
use od_core::{Entity, MText};

use crate::defaults::EditorDefaults;
use crate::draw::add;
use crate::field_cmd::field_context;

/// The object the user clicks, ready for the dialog; `None` when they back
/// out with Esc, which returns to the dialog.
fn pick_object(ed: &mut Editor) -> Result<Option<ObjectChoice>, Cancelled> {
    loop {
        let ask = Options::<Picked>::new("Select object for field").esc_backs();
        let Ok(Answer::Value(picked)) = ed.ask_object(ask) else {
            return Ok(None);
        };
        let (id, context) = (picked.id, field_context(ed));
        match ed.read(move |doc| ObjectChoice::of(doc, id, &context))? {
            Some(choice) if !choice.properties.is_empty() => return Ok(Some(choice)),
            _ => ed.say("That object has no property a field can show."),
        }
    }
}

pub fn insert_field_dialog(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut draft = FieldDraft::new(true);
    let field = loop {
        match ed.ask_field_dialog(draft.clone())? {
            FieldAnswer::Ok(field) => break *field,
            FieldAnswer::PickObject(back) => {
                draft = *back;
                if let Some(choice) = pick_object(ed)? {
                    draft.pick(choice);
                }
            }
        }
    };
    let at = ed.get_point("Specify start point of multiline text")?;
    let context = field_context(ed);
    let field = ed.read(move |doc| field.evaluated(doc, &context))?;
    let style = ed.read(|doc| doc.current_text_style())?;
    let mut text = MText::new(at, ed.defaults().text.get().height, "", style);
    if text.insert_field(0, field).is_none() {
        return Err(Cancelled);
    }
    add(ed, Entity::MText(Box::new(text)), "Field")
}

#[cfg(test)]
mod tests {
    use od_commands::Reply;
    use od_core::block::{DataField, ObjectChoice, ObjectProperty};
    use od_core::{EntityId, Point};

    use super::*;
    use crate::state::AppState;

    fn texts(state: &AppState) -> Vec<MText> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::MText(t) => Some((**t).clone()),
                _ => None,
            })
            .collect()
    }

    fn circle(state: &mut AppState) -> EntityId {
        state.add_entity(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 2.0,
        });
        state
            .session()
            .document()
            .objects()
            .last()
            .map(|o| o.id)
            .unwrap_or(EntityId(0))
    }

    fn start(state: &mut AppState) {
        state.start_command("FIELD", insert_field_dialog);
        state.settle_command();
    }

    fn dialog_is_open(state: &AppState) -> bool {
        state
            .waiting()
            .is_some_and(|a| a.wants == od_commands::Wants::FieldDialog)
    }

    #[test]
    fn ok_then_a_point_makes_a_text_holding_the_field_in_one_undo_step() {
        let mut state = AppState::new();
        let active = state.active;
        state.docs[active].path = Some(std::path::PathBuf::from("c:/work/plan.odx"));
        start(&mut state);
        assert!(dialog_is_open(&state));
        state.reply(Reply::Field(Box::new(DataField::file_name())));
        state.settle_command();
        state.cmd_submit("5,5");
        state.settle_command();
        assert!(!state.command_running());
        let made = texts(&state);
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].field_values(), ["plan.odx"]);
        assert_eq!(made[0].insertion, Point::new(5.0, 5.0));
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert!(texts(&state).is_empty(), "one undo step");
    }

    #[test]
    fn selecting_an_object_returns_to_the_dialog_with_it_and_its_value_is_shown() {
        let mut state = AppState::new();
        let c = circle(&mut state);
        start(&mut state);
        let mut draft = FieldDraft::new(true);
        draft.select_kind("Object");
        state.reply(Reply::FieldPick(Box::new(draft)));
        state.settle_command();
        assert!(!dialog_is_open(&state), "closed for the pick");
        state.answer_object(Point::new(2.0, 0.0), Some(c), false);
        state.settle_command();
        assert!(dialog_is_open(&state), "open again");
        let reopened = state
            .waiting()
            .and_then(|a| a.field_dialog.clone())
            .unwrap();
        let object = reopened
            .object
            .as_ref()
            .expect("the object is in the draft");
        assert_eq!(object.id, c);
        assert!(object
            .properties
            .iter()
            .any(|(p, _)| *p == ObjectProperty::Radius));
        // Taking its radius makes a text that shows it.
        state.reply(Reply::Field(Box::new(DataField::object_property(
            c,
            ObjectProperty::Radius,
        ))));
        state.settle_command();
        state.cmd_submit("0,0");
        state.settle_command();
        let radius = od_core::units::format_length(state.session().document().units(), 2.0);
        assert_eq!(texts(&state)[0].field_values(), [radius]);
    }

    #[test]
    fn esc_while_picking_goes_back_to_the_dialog_and_cancel_makes_nothing() {
        let mut state = AppState::new();
        circle(&mut state);
        start(&mut state);
        let mut draft = FieldDraft::new(true);
        draft.select_kind("Object");
        state.reply(Reply::FieldPick(Box::new(draft)));
        state.settle_command();
        state.esc_in_command();
        state.settle_command();
        assert!(dialog_is_open(&state), "Esc backs out to the dialog");
        state.cancel_command();
        assert!(!state.command_running());
        assert!(texts(&state).is_empty());
    }

    #[test]
    fn a_point_offers_its_layer_only_and_the_dialog_opens_again() {
        let mut state = AppState::new();
        state.add_entity(Entity::Point {
            at: Point::new(1.0, 1.0),
        });
        let point = state
            .session()
            .document()
            .objects()
            .last()
            .map(|o| o.id)
            .unwrap();
        start(&mut state);
        let mut draft = FieldDraft::new(true);
        draft.select_kind("Object");
        state.reply(Reply::FieldPick(Box::new(draft)));
        state.settle_command();
        state.answer_object(Point::new(1.0, 1.0), Some(point), false);
        state.settle_command();
        let choice = ObjectChoice::of(
            state.session().document(),
            point,
            &od_core::block::FieldContext::default(),
        )
        .unwrap();
        // A point has only a layer, so it is accepted; it must offer that.
        assert!(choice
            .properties
            .iter()
            .all(|(p, _)| *p == ObjectProperty::Layer));
        assert!(dialog_is_open(&state));
    }
}
