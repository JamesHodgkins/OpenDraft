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
        purge_items(ed, vec![Item::Linetype("DOT".to_owned())])
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
