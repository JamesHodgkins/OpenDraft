//! ATTDEF and ATTEDIT as dialogs: the Attribute Definition dialog, and the
//! Attribute Editor (AutoCAD's EATTEDIT). The command-line forms
//! are `-ATTDEF` (`attdef_cmd.rs`) and `-ATTEDIT` (`attribute_edit.rs`).

use od_commands::{AttributeEditorAnswer, Cancelled, Editor, PointOptions};
use od_core::block::attribute_draft::{DefinitionDraft, EditorDraft};
use od_core::block::{AttributeDefinitionId, AttributeFlags};
use od_core::{Attributes, Entity, EntityId, Insert};

use crate::attribute_edit::pick_attributed_insert;
use crate::defaults::EditorDefaults;

/// ATTDEF with its dialog: the modes, the attribute and its text, then where
/// it goes (picked on the screen, or typed in the dialog). The modes and
/// height are remembered for the next time.
pub fn attdef_dialog(ed: &mut Editor) -> Result<(), Cancelled> {
    let defaults = ed.defaults();
    let modes = defaults.attribute.get();
    let style = ed.read(|doc| doc.current_text_style())?;
    let mut draft = DefinitionDraft::new(
        AttributeFlags(modes.flags),
        modes.lock_position,
        defaults.text.get().height,
        style,
    );
    loop {
        draft = ed.ask_attribute_definition(draft)?;
        match draft.problem() {
            None => break,
            Some(why) => ed.say(why),
        }
    }
    defaults.attribute.change(|m| {
        // The modes as chosen, not as stored (a constant clears two of them).
        m.flags = AttributeFlags(0)
            .with(AttributeFlags::INVISIBLE, draft.invisible)
            .with(AttributeFlags::CONSTANT, draft.constant)
            .with(AttributeFlags::VERIFY, draft.verify)
            .with(AttributeFlags::PRESET, draft.preset)
            .0;
        m.lock_position = draft.lock_position;
    });
    defaults.text.change(|m| m.height = draft.height);
    let at = if draft.on_screen {
        let shown = draft.clone();
        ed.get_point(
            PointOptions::new("Specify start point of text").ghost(move |at| {
                let id = AttributeDefinitionId(1);
                vec![Entity::AttributeDefinition(Box::new(
                    shown.definition(id, at),
                ))]
            }),
        )?
    } else {
        draft.typed_point()
    };
    ed.transaction("Define attribute", move |tx| {
        let id = AttributeDefinitionId(tx.document().next_attribute_definition_id());
        tx.add(Entity::AttributeDefinition(Box::new(
            draft.definition(id, at),
        )));
        Ok(())
    })
}

/// The insert `id` and its own properties.
fn insert_of(ed: &Editor, id: EntityId) -> Result<Option<(Insert, Attributes)>, Cancelled> {
    ed.read(move |doc| {
        doc.object(id).and_then(|o| match &o.entity {
            Entity::Insert(i) => Some(((**i).clone(), o.attrs.clone())),
            _ => None,
        })
    })
}

/// ATTEDIT with the Attribute Editor, on the block reference that
/// was selected or that the user picks. Apply keeps it open; each Apply and
/// OK is one undo step.
pub fn eattedit_dialog(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut id = pick_attributed_insert(ed, true)?;
    let mut keep: Option<(usize, usize)> = None;
    // The editor as the user left it when they went to pick another block.
    let mut back: Option<EditorDraft> = None;
    loop {
        let Some((insert, own)) = insert_of(ed, id)? else {
            return Err(Cancelled);
        };
        let draft = back.take().unwrap_or_else(|| {
            let mut draft = EditorDraft::new(&insert, &own);
            if let Some((selected, tab)) = keep {
                draft.selected = selected.min(draft.rows.len().saturating_sub(1));
                draft.tab = tab;
            }
            draft
        });
        match ed.ask_attribute_editor(draft)? {
            AttributeEditorAnswer::SelectBlock(draft) => match pick_attributed_insert(ed, false) {
                Ok(picked) => {
                    id = picked;
                    keep = None;
                }
                // Esc: the same block, with what was typed.
                Err(_) => back = Some(draft),
            },
            AttributeEditorAnswer::Apply(draft) => {
                keep = Some((draft.selected, draft.tab));
                change(ed, id, &draft, &insert, &own)?;
            }
            AttributeEditorAnswer::Ok(draft) => {
                return change(ed, id, &draft, &insert, &own);
            }
        }
    }
}

fn change(
    ed: &Editor,
    id: EntityId,
    draft: &EditorDraft,
    insert: &Insert,
    own: &Attributes,
) -> Result<(), Cancelled> {
    let Some(changed) = draft.apply(insert, own) else {
        return Ok(());
    };
    ed.transaction("Edit attributes", move |tx| {
        tx.modify(id, |_| Entity::Insert(Box::new(changed.clone())));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use od_commands::{Reply, Wants};
    use od_core::block::attribute_draft::Justification;
    use od_core::Point;

    use super::*;
    use crate::attribute_test_util::{attributed_insert, values};
    use crate::state::AppState;

    fn dialog(state: &mut AppState, wants: Wants) {
        let ask = state.waiting().expect("a dialog is asked for");
        assert_eq!(ask.wants, wants);
    }

    #[test]
    fn the_definition_dialog_then_a_point_adds_a_definition_and_remembers_the_modes() {
        let mut state = AppState::new();
        state.start_command("ATTDEF", attdef_dialog);
        state.settle_command();
        dialog(&mut state, Wants::AttributeDefinitionDialog);
        let mut draft = state
            .waiting()
            .and_then(|a| a.attribute_definition.clone())
            .map(|d| *d)
            .expect("it opens on a draft");
        assert!(draft.on_screen && !draft.invisible);
        draft.tag = "part".into();
        draft.prompt = "Part number?".into();
        draft.default_value = "none".into();
        draft.invisible = true;
        draft.height = 4.0;
        draft.justification = Justification::MiddleCenter;
        state.reply(Reply::AttributeDefinition(Box::new(draft)));
        state.settle_command();
        state.cmd_submit("10,20");
        state.settle_command();
        assert!(!state.command_running());
        let doc = state.session().document();
        let Some(Entity::AttributeDefinition(d)) = doc.objects().last().map(|o| &o.entity) else {
            panic!("a definition was added")
        };
        assert_eq!(
            (d.tag.as_str(), d.prompt.as_str()),
            ("PART", "Part number?")
        );
        assert!(d.flags.invisible());
        assert_eq!(d.text.position, Point::new(10.0, 20.0));
        assert_eq!(d.text.height, 4.0);
        assert_eq!(doc.validate(), Ok(()));
        assert_eq!(state.defaults().attribute.get().flags, 1, "AFLAGS");
        assert_eq!(state.defaults().text.get().height, 4.0);
        state.undo();
        assert_eq!(state.session().document().len(), 0);
    }

    #[test]
    fn a_typed_point_needs_no_pick_and_a_bad_draft_asks_again() {
        let mut state = AppState::new();
        state.start_command("ATTDEF", attdef_dialog);
        state.settle_command();
        let draft = state
            .waiting()
            .and_then(|a| a.attribute_definition.clone())
            .map(|d| *d)
            .unwrap();
        // No tag: the dialog opens again, and says why.
        state.reply(Reply::AttributeDefinition(Box::new(draft.clone())));
        state.settle_command();
        dialog(&mut state, Wants::AttributeDefinitionDialog);
        assert!(state.cmdline.log.iter().any(|l| l.contains("tag")));
        let mut good = draft;
        good.tag = "t".into();
        good.on_screen = false;
        (good.x, good.y) = (3.0, 4.0);
        state.reply(Reply::AttributeDefinition(Box::new(good)));
        state.settle_command();
        assert!(!state.command_running());
        let Some(Entity::AttributeDefinition(d)) = state
            .session()
            .document()
            .objects()
            .last()
            .map(|o| o.entity.clone())
        else {
            panic!("a definition")
        };
        assert_eq!(d.text.position, Point::new(3.0, 4.0));
    }

    #[test]
    fn cancelling_the_dialog_adds_nothing() {
        let mut state = AppState::new();
        state.start_command("ATTDEF", attdef_dialog);
        state.settle_command();
        state.cancel_command();
        assert_eq!(state.session().document().len(), 0);
    }

    fn open_editor(state: &mut AppState) -> EditorDraft {
        state.start_command("EATTEDIT", eattedit_dialog);
        state.settle_command();
        dialog(state, Wants::AttributeEditorDialog);
        state
            .waiting()
            .and_then(|a| a.attribute_editor.clone())
            .map(|d| *d)
            .expect("it opens on a draft")
    }

    #[test]
    fn the_editor_opens_on_the_selected_block_and_ok_changes_it_in_one_step() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let mut draft = open_editor(&mut state);
        assert_eq!(draft.block, "Label");
        assert_eq!(draft.rows.len(), 2);
        draft.rows[1].value = "edited".into();
        draft.rows[1].text.height = 3.0;
        state.reply(Reply::AttributeEditor(Box::new(draft), true));
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(values(&state, id), ["dflt", "edited"]);
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert_eq!(values(&state, id), ["dflt", "dflt"]);
    }

    #[test]
    fn apply_keeps_the_editor_open_and_each_apply_is_a_step() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let mut draft = open_editor(&mut state);
        draft.selected = 1;
        draft.tab = 2;
        draft.rows[0].value = "one".into();
        state.reply(Reply::AttributeEditor(Box::new(draft), false));
        state.settle_command();
        dialog(&mut state, Wants::AttributeEditorDialog);
        assert_eq!(values(&state, id), ["one", "dflt"]);
        let mut again = state
            .waiting()
            .unwrap()
            .attribute_editor
            .clone()
            .map(|d| *d)
            .unwrap();
        assert_eq!((again.selected, again.tab), (1, 2), "it stays where it was");
        assert!(!again.is_changed(), "Apply is not offered until a change");
        again.rows[1].value = "two".into();
        state.reply(Reply::AttributeEditor(Box::new(again), true));
        state.settle_command();
        assert_eq!(values(&state, id), ["one", "two"]);
        state.undo();
        assert_eq!(values(&state, id), ["one", "dflt"]);
        state.undo();
        assert_eq!(values(&state, id), ["dflt", "dflt"]);
    }

    #[test]
    fn cancel_changes_nothing() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let mut draft = open_editor(&mut state);
        draft.rows[0].value = "lost".into();
        state.cancel_command();
        assert_eq!(values(&state, id), ["dflt", "dflt"]);
    }

    #[test]
    fn esc_while_choosing_another_block_returns_to_the_editor_with_its_edits() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        let mut draft = open_editor(&mut state);
        draft.rows[0].value = "typed".into();
        state.reply(Reply::AttributeEditorSelect(Box::new(draft)));
        state.settle_command();
        assert_ne!(state.waiting().unwrap().wants, Wants::AttributeEditorDialog);
        state.cancel();
        state.settle_command();
        let back = state.waiting().unwrap();
        assert_eq!(back.wants, Wants::AttributeEditorDialog);
        let back = back.attribute_editor.clone().map(|d| *d).unwrap();
        assert_eq!(back.rows[0].value, "typed", "what was typed is still there");
        assert!(back.is_changed());
        assert_eq!(
            values(&state, id),
            ["dflt", "dflt"],
            "and nothing was applied"
        );
    }
}
