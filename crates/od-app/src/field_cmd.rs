//! FIELD and UPDATEFIELD: put a data field into an attribute, and bring the
//! fields of block references up to date. FIELD is asked on the command line
//! (an Insert Field dialog is not built); it offers the object properties and
//! the file name that `od_core::block` evaluates.

use od_commands::{Answer, Cancelled, Editor, ModifyEntities, Options};
use od_core::block::{update_fields, DataField, FieldContext, FieldRun, FieldText, ObjectProperty};
use od_core::Entity;

use crate::attribute_edit::pick_attributed_insert;
use crate::text::plural;

/// What the drawing knows about itself that a field may show, as the command
/// was started with it.
pub(crate) fn field_context(ed: &Editor) -> FieldContext {
    ed.context()
        .get::<FieldContext>()
        .map(|c| (*c).clone())
        .unwrap_or_default()
}

/// The shortest answer that tells the property from the others.
fn key(property: ObjectProperty) -> &'static str {
    match property {
        ObjectProperty::Length => "Le",
        ObjectProperty::Layer => "La",
        other => &other.name()[..1],
    }
}

/// UPDATEFIELD: evaluate every data field in the attributes of the selected
/// block references. One undo step.
pub fn update_field(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let context = field_context(ed);
    let update = ed.read(move |doc| update_fields(doc, &ids, &context))?;
    if update.found == 0 {
        ed.say("No data fields in the selected objects.");
        return Ok(());
    }
    let mut report = format!(
        "{} found, {} updated",
        plural(update.found, "field"),
        update.changed
    );
    if update.failed > 0 {
        report.push_str(&format!(
            ", {} could not be evaluated and keep their last value",
            update.failed
        ));
    }
    ed.say(format!("{report}."));
    let edits = update.edits;
    if edits.is_empty() {
        return Ok(());
    }
    ed.transaction("Update fields", move |tx| {
        tx.perform(ModifyEntities::new("Update fields", edits));
        Ok(())
    })
}

/// The attribute of `tags` the user means: the first on Enter.
fn ask_tag(ed: &Editor, tags: &[String]) -> Result<usize, Cancelled> {
    let [first, rest @ ..] = tags else {
        return Err(Cancelled);
    };
    if rest.is_empty() {
        return Ok(0);
    }
    loop {
        let message = format!("Enter tag of the attribute that will show the field <{first}>");
        let Answer::Value(typed) = ed.ask_string(Options::<String>::new(message))? else {
            return Ok(0);
        };
        match tags
            .iter()
            .position(|t| t.eq_ignore_ascii_case(typed.trim()))
        {
            Some(at) => return Ok(at),
            None => ed.say("No attribute has that tag."),
        }
    }
}

/// The field to put in, asked for: the file name, or a property of an object
/// picked.
fn ask_field(ed: &mut Editor) -> Result<DataField, Cancelled> {
    let kind = Options::<String>::new("Field type")
        .keyword("Object property", "O")
        .keyword("Filename", "F")
        .default("Object property".to_owned());
    if ed.get_keyword(kind)? == "Filename" {
        return Ok(DataField::file_name());
    }
    loop {
        let Answer::Value(picked) = ed.ask_object(Options::new("Select object"))? else {
            return Err(Cancelled);
        };
        let id = picked.id;
        let Some(properties) = ed.read(move |doc| doc.get(id).map(ObjectProperty::of))? else {
            continue;
        };
        let Some(first) = properties.first().copied() else {
            continue;
        };
        let mut options = Options::<String>::new("Property").default(first.name().to_owned());
        for p in &properties {
            options = options.keyword(p.name(), key(*p));
        }
        let chosen = ed.get_keyword(options)?;
        let property = ObjectProperty::from_name(&chosen).unwrap_or(first);
        return Ok(DataField::object_property(id, property));
    }
}

/// FIELD: make an attribute of a block reference show a data field.
pub fn insert_field(ed: &mut Editor) -> Result<(), Cancelled> {
    let id = pick_attributed_insert(ed, true)?;
    let Some(mut insert) = ed.read(move |doc| match doc.get(id) {
        Some(Entity::Insert(i)) => Some((**i).clone()),
        _ => None,
    })?
    else {
        return Err(Cancelled);
    };
    let tags: Vec<String> = insert.attributes.iter().map(|r| r.tag.clone()).collect();
    let at = ask_tag(ed, &tags)?;
    let field = ask_field(ed)?;
    let context = field_context(ed);
    let field = ed.read(move |doc| field.evaluated(doc, &context))?;
    let shown = field.display();
    insert.attributes[at].value = FieldText::Runs(vec![FieldRun::Field(field)]);
    let tag = tags[at].clone();
    ed.transaction("Insert field", move |tx| {
        tx.modify(id, |_| Entity::Insert(Box::new(insert.clone())));
        Ok(())
    })?;
    ed.say(format!("{tag} now shows {shown}."));
    Ok(())
}

#[cfg(test)]
mod tests {
    use od_core::units::format_length;
    use od_core::{Entity, EntityId, Point};

    use super::*;
    use crate::attribute_test_util::{attributed_insert, values};
    use crate::state::AppState;

    fn submit_all(state: &mut AppState, answers: &[&str]) {
        for answer in answers {
            state.cmd_submit(answer);
            state.settle_command();
        }
    }

    fn circle(state: &mut AppState, radius: f64) -> EntityId {
        state.add_entity(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius,
        });
        state
            .session()
            .document()
            .objects()
            .last()
            .map(|o| o.id)
            .unwrap_or(EntityId(0))
    }

    fn length(state: &AppState, v: f64) -> String {
        format_length(state.session().document().units(), v)
    }

    /// FIELD on the insert `id`, choosing attribute B and the radius of `of`.
    fn put_radius_field(state: &mut AppState, id: EntityId, of: EntityId) {
        state.select_ids([id], false);
        state.start_command("FIELD", insert_field);
        state.settle_command();
        submit_all(state, &["B", ""]);
        state.answer_object(Point::new(0.0, 0.0), Some(of), false);
        state.settle_command();
        submit_all(state, &["Radius"]);
        assert!(!state.command_running());
    }

    #[test]
    fn a_field_shows_its_value_at_once_and_is_one_undo_step() {
        let (mut state, id) = attributed_insert();
        let c = circle(&mut state, 2.0);
        put_radius_field(&mut state, id, c);
        assert_eq!(values(&state, id), ["dflt".to_owned(), length(&state, 2.0)]);
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert_eq!(values(&state, id), ["dflt", "dflt"]);
    }

    #[test]
    fn updatefield_follows_the_object_and_undoes_in_one_step() {
        let (mut state, id) = attributed_insert();
        let c = circle(&mut state, 2.0);
        put_radius_field(&mut state, id, c);
        state.perform(od_commands::ModifyEntities::new(
            "Resize",
            vec![(
                c,
                Entity::Circle {
                    center: Point::new(0.0, 0.0),
                    radius: 5.0,
                },
            )],
        ));
        assert_eq!(
            values(&state, id)[1],
            length(&state, 2.0),
            "stale until updated"
        );
        state.select_ids([id], false);
        state.start_command("UPDATEFIELD", update_field);
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(values(&state, id)[1], length(&state, 5.0));
        state.undo();
        assert_eq!(values(&state, id)[1], length(&state, 2.0));
    }

    #[test]
    fn updating_when_nothing_changed_makes_no_undo_step() {
        let (mut state, id) = attributed_insert();
        let c = circle(&mut state, 2.0);
        put_radius_field(&mut state, id, c);
        state.select_ids([id], false);
        let before = state.session().revision();
        state.start_command("UPDATEFIELD", update_field);
        state.settle_command();
        assert_eq!(state.session().revision(), before);
    }

    #[test]
    fn an_insert_without_fields_is_passed_over() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let before = state.session().revision();
        state.start_command("UPDATEFIELD", update_field);
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(state.session().revision(), before);
    }

    #[test]
    fn the_file_name_field_shows_the_name_of_the_saved_drawing() {
        let (mut state, id) = attributed_insert();
        let active = state.active;
        state.docs[active].path = Some(std::path::PathBuf::from("c:/work/plan.odx"));
        state.select_ids([id], false);
        state.start_command("FIELD", insert_field);
        state.settle_command();
        submit_all(&mut state, &["A", "Filename"]);
        assert!(!state.command_running());
        assert_eq!(values(&state, id)[0], "plan.odx");
    }

    #[test]
    fn a_wrong_tag_is_asked_again() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        state.start_command("FIELD", insert_field);
        state.settle_command();
        submit_all(&mut state, &["nope"]);
        assert!(state.command_running(), "still waiting for a tag");
    }
}
