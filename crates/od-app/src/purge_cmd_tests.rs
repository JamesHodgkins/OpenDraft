use super::*;
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

#[test]
fn purge_all_deletes_the_unused_in_one_undo_step_and_what_that_frees() {
    let mut state = AppState::new();
    state.add_entity(line(0.0, 5.0));
    let kept = state.session().document().objects()[0].id;
    state.add_entity(line(1.0, 1.0));
    let before = state.session().document().linetypes().records().len();
    state.start_command("-PURGE", purge_all);
    state.settle_command();
    let doc = state.session().document();
    assert_eq!(doc.objects().len(), 1);
    assert!(doc.contains(kept));
    assert!(doc.linetypes().records().len() < before);
    assert!(said(&state, "Deleting 1 object of zero-length geometry."));
    state.undo();
    assert_eq!(state.session().document().objects().len(), 2);
    assert_eq!(
        state.session().document().linetypes().records().len(),
        before
    );
}

#[test]
fn what_a_drawing_uses_is_kept() {
    let mut state = AppState::new();
    let mut attrs = Attributes::on_layer(LayerId::DEFAULT);
    attrs.linetype = Attr::Set("DASHED".to_owned());
    state.perform(od_commands::AddCopies::new(
        "Add",
        vec![(line(0.0, 5.0), attrs)],
    ));
    state.start_command("-PURGE", purge_all);
    state.settle_command();
    let doc = state.session().document();
    assert!(doc.linetypes().named("DASHED").is_some());
    assert!(doc.linetypes().named("DOT").is_none());
    state.start_command("-PURGE", purge_all);
    state.settle_command();
    assert!(said(&state, "Nothing to purge."));
}

#[test]
fn purging_chosen_items_leaves_the_rest() {
    let mut state = AppState::new();
    state.add_entity(line(1.0, 1.0));
    state.start_command("PURGE", |ed| {
        let only = Some(vec![Item::Linetype("DOT".to_owned())]);
        purge_run(
            ed,
            Purge {
                only,
                nested: false,
                confirm: false,
            },
        )
    });
    state.settle_command();
    let doc = state.session().document();
    assert!(doc.linetypes().named("DOT").is_none());
    assert!(doc.linetypes().named("DASHED").is_some());
    assert_eq!(
        doc.objects().len(),
        1,
        "the zero-length line was not ticked"
    );
}

#[test]
fn confirming_each_item_deletes_only_those_answered_yes() {
    let mut state = AppState::new();
    state.start_command("PURGE", |ed| {
        let only = Some(vec![
            Item::Linetype("DASHED".to_owned()),
            Item::Linetype("DOT".to_owned()),
            Item::Linetype("HIDDEN".to_owned()),
        ]);
        purge_run(
            ed,
            Purge {
                only,
                nested: false,
                confirm: true,
            },
        )
    });
    state.settle_command();
    for answer in ["Y", "N", "A"] {
        state.cmd_submit(answer);
        state.settle_command();
    }
    let doc = state.session().document();
    assert!(doc.linetypes().named("DASHED").is_none());
    assert!(doc.linetypes().named("DOT").is_some(), "answered No");
    assert!(doc.linetypes().named("HIDDEN").is_none(), "All");
    assert!(doc.linetypes().named("CENTER").is_some(), "not offered");
}

#[test]
fn escape_while_confirming_changes_nothing() {
    let mut state = AppState::new();
    let before = state.session().document().linetypes().records().len();
    state.start_command("PURGE", |ed| {
        purge_run(
            ed,
            Purge {
                only: None,
                nested: true,
                confirm: true,
            },
        )
    });
    state.settle_command();
    state.cancel_command();
    assert_eq!(
        state.session().document().linetypes().records().len(),
        before
    );
}
