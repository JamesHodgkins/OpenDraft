//! Undoing what one assistant task changed. Only its own steps, and only
//! while they are still on top of a drawing's history: the moment you have
//! edited since, what is beneath your edit is left alone, because undoing it
//! would mean undoing your work or reaching past it.

use od_commands::Provenance;

use crate::state::AppState;
use crate::text::plural;

/// Undo up to `limit` steps made by the connection `grant`, in whichever open
/// drawings they sit on top of. `Ok` says what was done, and what was left;
/// `Err` is why nothing could be.
pub fn task(state: &mut AppState, grant: u64, limit: u32) -> Result<String, String> {
    if state.command_running() {
        return Err("Finish or cancel the running command first.".to_owned());
    }
    let mine = |made_by: Provenance| made_by == Provenance::Automation(grant);
    let limit = limit as usize;
    let mut undone = 0;
    for doc in &mut state.docs {
        while undone < limit {
            let Some(top) = doc.session.history().top_undo() else {
                break;
            };
            if doc.session.undo_checked(top.token, mine).is_err() {
                break;
            }
            undone += 1;
        }
        doc.prune_selection();
    }
    match undone {
        0 => Err(
            "Nothing could be undone: you have made changes since, or it is already undone."
                .to_owned(),
        ),
        n if n < limit => Ok(format!(
            "Undid {} of {}. The rest are beneath changes you made since, and were left.",
            plural(n, "change"),
            limit
        )),
        n => Ok(format!("Undid {}.", plural(n, "change"))),
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;

    fn draw(state: &mut AppState, by: Provenance) {
        let line = Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        };
        let _ = state.docs[0].session.checked_transaction("Draw", by, |tx| {
            tx.add(line);
            Ok::<_, ()>(())
        });
    }

    #[test]
    fn it_never_takes_more_steps_than_it_was_told_the_task_made() {
        let mut state = AppState::new();
        draw(&mut state, Provenance::Automation(7));
        draw(&mut state, Provenance::Automation(7));
        let said = task(&mut state, 7, 1).unwrap();
        assert_eq!(said, "Undid 1 change.");
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn it_stops_at_a_step_that_is_not_the_tasks_and_says_what_was_left() {
        let mut state = AppState::new();
        draw(&mut state, Provenance::Automation(7));
        draw(&mut state, Provenance::User);
        draw(&mut state, Provenance::Automation(7));
        let said = task(&mut state, 7, 2).unwrap();
        assert!(said.starts_with("Undid 1 change of 2"), "{said}");
        assert_eq!(state.session().document().len(), 2);
        assert!(
            task(&mut state, 7, 2).is_err(),
            "the user's step is now on top"
        );
    }

    #[test]
    fn another_tasks_steps_are_left_alone() {
        let mut state = AppState::new();
        draw(&mut state, Provenance::Automation(8));
        assert!(task(&mut state, 7, 1).is_err());
        assert_eq!(state.session().document().len(), 1);
    }
}
