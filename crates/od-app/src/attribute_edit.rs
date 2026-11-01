//! ATTEDIT: change the values of one block reference's attributes, one at a
//! time, with the order and tags the block defines. Enter keeps a value. A
//! value that is a data field is only replaced by plain text after the user
//! says so.

use od_commands::{Answer, Cancelled, Editor, Options, Picked};
use od_core::block::{AttributeReference, FieldText};
use od_core::{Entity, EntityId};

/// The block reference to edit: the one preselected, or one the user clicks.
pub(crate) fn pick_attributed_insert(
    ed: &mut Editor,
    use_selection: bool,
) -> Result<EntityId, Cancelled> {
    let has_attributes = |ed: &Editor, id| {
        ed.read(move |doc| match doc.get(id) {
            Some(Entity::Insert(i)) => Some(!i.attributes.is_empty()),
            _ => None,
        })
    };
    let selected = if use_selection { ed.pickset() } else { &[] };
    if let [id] = selected {
        let id = *id;
        if has_attributes(ed, id)? == Some(true) {
            return Ok(id);
        }
    }
    loop {
        let mut options = Options::<Picked>::new("Select block reference");
        if !use_selection {
            // Picking another block from the editor: Esc goes back to it.
            options = options.esc_backs();
        }
        let Answer::Value(picked) = ed.ask_object(options)? else {
            return Err(Cancelled);
        };
        match has_attributes(ed, picked.id)? {
            Some(true) => return Ok(picked.id),
            Some(false) => ed.say("That block reference has no attributes."),
            None => ed.say("That is not a block reference."),
        }
    }
}

/// The new value of `r`, asked for: `None` keeps it.
fn ask_new(ed: &Editor, r: &AttributeReference) -> Result<Option<FieldText>, Cancelled> {
    let shown = r.value.display();
    let message = format!("Enter new value for {} <{shown}>", r.tag);
    let Answer::Value(text) = ed.ask_string(Options::<String>::new(message))? else {
        return Ok(None);
    };
    if text == shown {
        return Ok(None);
    }
    if r.value.has_fields() {
        let question = Options::<String>::new(format!(
            "{} holds a data field. Replace it with this text?",
            r.tag
        ))
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default("No".to_owned());
        if ed.get_keyword(question)? != "Yes" {
            return Ok(None);
        }
    }
    Ok(Some(FieldText::Literal(text)))
}

pub fn edit_attributes(ed: &mut Editor) -> Result<(), Cancelled> {
    let id = pick_attributed_insert(ed, true)?;
    let Some(mut insert) = ed.read(move |doc| match doc.get(id) {
        Some(Entity::Insert(i)) => Some((**i).clone()),
        _ => None,
    })?
    else {
        return Err(Cancelled);
    };
    let mut changed = 0;
    for r in &mut insert.attributes {
        if let Some(value) = ask_new(ed, r)? {
            r.value = value;
            changed += 1;
        }
    }
    if changed == 0 {
        return Ok(());
    }
    ed.transaction("Edit attributes", move |tx| {
        tx.modify(id, |_| Entity::Insert(Box::new(insert.clone())));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use od_core::block::{DataField, FieldRun, FieldValue};

    use super::*;
    use crate::attribute_test_util::{attributed_insert, values};

    #[test]
    fn enter_keeps_a_value_and_typing_replaces_it_in_one_undo_step() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        state.start_command("ATTEDIT", edit_attributes);
        state.settle_command();
        for answer in ["", "new b"] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        assert!(!state.command_running());
        assert_eq!(values(&state, id), ["dflt", "new b"]);
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert_eq!(values(&state, id), ["dflt", "dflt"]);
    }

    #[test]
    fn nothing_changed_is_no_undo_step() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let before = state.session().revision();
        state.start_command("ATTEDIT", edit_attributes);
        state.settle_command();
        for answer in ["", ""] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        assert_eq!(state.session().revision(), before);
    }

    #[test]
    fn replacing_a_data_field_needs_a_yes() {
        let (mut state, id) = attributed_insert();
        let field = DataField::new("AcVar", "x", Some(FieldValue::Text("f".into())));
        let Some(Entity::Insert(insert)) = state.session().document().get(id) else {
            panic!("an insert")
        };
        let mut insert = (**insert).clone();
        insert.attributes[0].value = FieldText::Runs(vec![FieldRun::Field(field)]);
        state.perform(od_commands::ModifyEntities::new(
            "Field",
            vec![(id, Entity::Insert(Box::new(insert)))],
        ));
        state.select_ids([id], false);
        state.start_command("ATTEDIT", edit_attributes);
        state.settle_command();
        for answer in ["typed", "N", ""] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        assert_eq!(values(&state, id), ["f", "dflt"], "No keeps the field");
    }
}
