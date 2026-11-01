//! BLOCK with its dialog: the Block Definition dialog, laid out as
//! AutoCAD's. The pick buttons close it while the user picks in the drawing,
//! and it opens again with what was picked. `-BLOCK` is the command-line form
//! (`block_cmd.rs`).

use od_commands::{BlockDialogButton, Cancelled, Editor, Options, PointOptions};
use od_core::block::block_draft::BlockDraft;
use od_core::Point;

use crate::block_cmd::{make_block, Made};
use crate::block_editor::request_edit;

/// The block the draft's name stands for, if there is one: its id and name
/// as the drawing spells it.
fn existing(ed: &Editor, name: &str) -> Result<Option<(od_core::BlockId, String)>, Cancelled> {
    let name = name.to_owned();
    ed.read(move |doc| doc.block_named(&name).map(|b| (b.id, b.name.clone())))
}

pub fn define_block_dialog(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut draft = BlockDraft::new(ed.pickset());
    draft.unit = ed.read(|doc| doc.units().insert)?;
    let found = loop {
        let (button, d) = ed.ask_block_dialog(draft)?;
        draft = d;
        match button {
            // Esc in either pick goes back to the dialog, as in AutoCAD.
            BlockDialogButton::PickPoint => {
                let ask = PointOptions::new("Specify insertion base point").esc_backs();
                if let Ok(at) = ed.get_point(ask) {
                    (draft.x, draft.y, draft.base_on_screen) = (at.x, at.y, false);
                }
            }
            BlockDialogButton::SelectObjects => {
                let ask = Options::<()>::new("Select objects")
                    .allow_empty()
                    .esc_backs();
                if let Ok(chosen) = ed.ask_selection(ask) {
                    draft.objects = chosen;
                    draft.objects_on_screen = false;
                }
            }
            BlockDialogButton::Ok => {
                if let Some(why) = draft.problem() {
                    ed.say(why);
                    continue;
                }
                let found = existing(ed, draft.name.trim())?;
                let Some((_, shown)) = &found else {
                    break found;
                };
                let question =
                    Options::new(format!("Block \"{shown}\" already exists. Redefine it?"))
                        .keyword("Yes", "Y")
                        .keyword("No", "N")
                        .default("No".to_owned());
                if ed.get_keyword(question)? == "Yes" {
                    break found;
                }
            }
        }
    };
    let base = if draft.base_on_screen {
        ed.get_point("Specify insertion base point")?
    } else {
        draft.base()
    };
    let chosen = if draft.objects_on_screen {
        ed.get_selection("Select objects")?
    } else {
        draft.objects.clone()
    };
    let made = Made {
        name: found
            .as_ref()
            .map_or_else(|| draft.name.trim().to_owned(), |(_, n)| n.clone()),
        existing: found.map(|(id, _)| id),
        base: Point::new(base.x, base.y),
        chosen,
        action: draft.action,
        description: draft.description.clone(),
        unit: draft.unit,
    };
    if let Some(id) = make_block(ed, made)? {
        if draft.open_in_editor {
            request_edit(ed, id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use od_commands::{BlockDialogButton, Reply, Wants};
    use od_core::block::block_draft::ObjectsAction;
    use od_core::{Entity, Point};

    use super::*;
    use crate::state::AppState;

    fn line(x: f64) -> Entity {
        Entity::Line {
            start: Point::new(x, 0.0),
            end: Point::new(x + 1.0, 0.0),
        }
    }

    fn drawing() -> AppState {
        let mut state = AppState::new();
        state.add_entity(line(0.0));
        state.add_entity(line(5.0));
        state
    }

    fn open(state: &mut AppState) -> BlockDraft {
        state.start_command("BLOCK", define_block_dialog);
        state.settle_command();
        let ask = state.waiting().expect("the dialog is open");
        assert_eq!(ask.wants, Wants::BlockDialog);
        ask.block_dialog.clone().map(|d| *d).expect("a draft")
    }

    fn press(state: &mut AppState, draft: BlockDraft, button: BlockDialogButton) {
        state.reply(Reply::BlockDialog(Box::new(draft), button));
        state.settle_command();
    }

    fn blocks(state: &AppState) -> Vec<String> {
        let doc = state.session().document();
        doc.blocks().iter().map(|b| b.name.clone()).collect()
    }

    #[test]
    fn the_dialog_opens_on_the_selection_and_ok_converts_it_with_a_typed_base_point() {
        let mut state = drawing();
        state.select_all();
        let mut draft = open(&mut state);
        assert_eq!(draft.objects.len(), 2);
        draft.name = "Pair".into();
        draft.x = 1.0;
        draft.description = "two lines".into();
        draft.unit = od_core::InsertUnits::from_code(4).unwrap();
        press(&mut state, draft, BlockDialogButton::Ok);
        assert!(!state.command_running());
        let doc = state.session().document();
        let block = doc.block_named("Pair").unwrap();
        assert_eq!(
            (block.description.as_str(), block.base.x),
            ("two lines", 1.0)
        );
        assert_eq!(block.unit.label(), "Millimeters");
        let kinds: Vec<_> = doc
            .objects()
            .iter()
            .map(|o| o.entity.kind().wire_name())
            .collect();
        assert_eq!(kinds, ["block_reference"], "converted to a reference");
        assert_eq!(doc.validate(), Ok(()));
        state.undo();
        assert_eq!(state.session().document().len(), 2);
        assert!(blocks(&state).is_empty());
    }

    #[test]
    fn retain_keeps_the_objects_and_delete_removes_them_without_a_reference() {
        for (action, left) in [(ObjectsAction::Retain, 2), (ObjectsAction::Delete, 0)] {
            let mut state = drawing();
            state.select_all();
            let mut draft = open(&mut state);
            draft.name = "B".into();
            draft.action = action;
            press(&mut state, draft, BlockDialogButton::Ok);
            assert_eq!(state.session().document().len(), left, "{action:?}");
            assert_eq!(blocks(&state), ["B"]);
        }
    }

    #[test]
    fn pick_point_and_select_objects_come_back_to_the_dialog_with_what_was_picked() {
        let mut state = drawing();
        let mut draft = open(&mut state);
        assert!(draft.objects.is_empty());
        draft.name = "Picked".into();
        press(&mut state, draft.clone(), BlockDialogButton::PickPoint);
        state.cmd_submit("3,4");
        state.settle_command();
        let draft = state
            .waiting()
            .unwrap()
            .block_dialog
            .clone()
            .map(|d| *d)
            .unwrap();
        assert_eq!(
            (draft.x, draft.y, draft.name.as_str()),
            (3.0, 4.0, "Picked")
        );
        press(&mut state, draft, BlockDialogButton::SelectObjects);
        state.select_all();
        state.answer_enter();
        state.settle_command();
        let draft = state
            .waiting()
            .unwrap()
            .block_dialog
            .clone()
            .map(|d| *d)
            .unwrap();
        assert_eq!(draft.objects.len(), 2);
        press(&mut state, draft, BlockDialogButton::Ok);
        assert!(!state.command_running());
        assert_eq!(
            state
                .session()
                .document()
                .block_named("Picked")
                .unwrap()
                .base,
            Point::new(3.0, 4.0)
        );
    }

    #[test]
    fn specify_on_screen_asks_for_the_point_and_the_objects_after_ok() {
        let mut state = drawing();
        let mut draft = open(&mut state);
        draft.name = "Later".into();
        draft.base_on_screen = true;
        draft.objects_on_screen = true;
        press(&mut state, draft, BlockDialogButton::Ok);
        state.cmd_submit("2,2");
        state.settle_command();
        state.select_all();
        state.answer_enter();
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(
            state
                .session()
                .document()
                .block_named("Later")
                .unwrap()
                .base,
            Point::new(2.0, 2.0)
        );
    }

    #[test]
    fn a_name_in_use_asks_before_redefining_and_no_returns_to_the_dialog() {
        let mut state = drawing();
        state.select_all();
        let mut draft = open(&mut state);
        draft.name = "Same".into();
        press(&mut state, draft, BlockDialogButton::Ok);
        state.add_entity(line(9.0));
        state.select_all();
        let mut draft = open(&mut state);
        draft.name = "SAME".into();
        draft.objects = vec![state.session().document().objects().last().unwrap().id];
        press(&mut state, draft, BlockDialogButton::Ok);
        state.cmd_submit("N");
        state.settle_command();
        // No: the dialog is open again.
        assert_eq!(state.waiting().unwrap().wants, Wants::BlockDialog);
        let draft = state
            .waiting()
            .unwrap()
            .block_dialog
            .clone()
            .map(|d| *d)
            .unwrap();
        press(&mut state, draft, BlockDialogButton::Ok);
        state.cmd_submit("Y");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(blocks(&state), ["Same"], "one block, redefined");
    }

    #[test]
    fn open_in_block_editor_opens_it_on_the_new_block() {
        let mut state = drawing();
        state.select_all();
        let mut draft = open(&mut state);
        draft.name = "Edit me".into();
        draft.open_in_editor = true;
        press(&mut state, draft, BlockDialogButton::Ok);
        state.take_block_requests();
        assert!(state.in_block_editor());
    }

    #[test]
    fn cancel_makes_nothing() {
        let mut state = drawing();
        state.select_all();
        let _ = open(&mut state);
        state.cancel_command();
        assert!(blocks(&state).is_empty());
        assert_eq!(state.session().document().len(), 2);
    }

    #[test]
    fn esc_while_picking_goes_back_to_the_dialog_with_what_was_typed() {
        for button in [
            BlockDialogButton::PickPoint,
            BlockDialogButton::SelectObjects,
        ] {
            let mut state = drawing();
            let mut draft = open(&mut state);
            draft.name = "Typed".into();
            draft.description = "kept".into();
            press(&mut state, draft, button);
            assert!(state.command_running(), "{button:?}: asking in the drawing");
            assert_ne!(state.waiting().unwrap().wants, Wants::BlockDialog);
            state.cancel();
            state.settle_command();
            assert!(state.command_running(), "{button:?}: Esc did not end it");
            let back = state.waiting().unwrap();
            assert_eq!(back.wants, Wants::BlockDialog);
            let back = back.block_dialog.clone().map(|d| *d).unwrap();
            assert_eq!(
                (back.name.as_str(), back.description.as_str()),
                ("Typed", "kept")
            );
            assert!(
                back.objects.is_empty() && back.x == 0.0,
                "nothing was picked"
            );
            // Esc again at the dialog's own prompt-less state is Cancel: it ends.
            state.cancel_command();
            assert!(!state.command_running());
        }
    }

    #[test]
    fn esc_at_the_questions_after_ok_still_ends_the_command() {
        let mut state = drawing();
        let mut draft = open(&mut state);
        draft.name = "Later".into();
        draft.base_on_screen = true;
        draft.objects_on_screen = true;
        press(&mut state, draft, BlockDialogButton::Ok);
        state.cancel();
        state.settle_command();
        assert!(!state.command_running());
        assert!(blocks(&state).is_empty());
    }
}
