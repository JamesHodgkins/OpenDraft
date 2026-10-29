//! What the chat tells the model about where it is, and how it words what you
//! answered. Small, pure, and apart from the session's bookkeeping.

use super::worker::{Ask, Reply};
use super::Drawings;
use crate::assistant::Reach;
use crate::automation::{document_id, AutomationCore};
use crate::state::AppState;

/// What the model is told about the drawing in view, ahead of the user's
/// words. It is the user's own drawing's name and a wire id, not its content.
pub(super) fn context(state: &AppState, core: &AutomationCore, drawings: Drawings) -> String {
    if state.home {
        return "The user is on the Home page; no drawing is in view.".to_owned();
    }
    let named = |doc: &crate::state::Doc| {
        let title: String = doc
            .title
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect();
        format!(
            "\"{title}\" (document_id {})",
            document_id(core.instance_id(), doc.uid)
        )
    };
    let view = named(&state.docs[state.active]);
    match drawings {
        Drawings::Current => {
            format!("The drawing in view is {view}. It is the only drawing you may use.")
        }
        Drawings::All => {
            let all: Vec<String> = state.docs.iter().map(named).collect();
            format!(
                "The drawing in view is {view}. The user allows you to use all the open \
                 drawings: {}.",
                all.join("; ")
            )
        }
    }
}

/// What a task may reach, from the choice in the panel.
pub(super) fn reach(state: &AppState, core: &AutomationCore, drawings: Drawings) -> Reach {
    match drawings {
        Drawings::All => Reach::All,
        Drawings::Current if state.home => Reach::Only(String::new()),
        Drawings::Current => Reach::Only(document_id(
            core.instance_id(),
            state.docs[state.active].uid,
        )),
    }
}

/// Whether a tool changes a drawing, for counting what a task did.
pub(super) fn changes_drawing_tool(name: &str) -> bool {
    od_api::catalogue::find(name).is_some_and(|t| od_agent::changes_drawing(t.operation))
}

/// What was answered, for the transcript.
pub(super) fn said(ask: &Ask, reply: &Reply) -> String {
    use od_agent::{Answer, PlanOutcome};

    use crate::assistant::Decision;
    match (ask, reply) {
        (Ask::Change(_), Reply::Change(Decision::Approved)) => {
            "You allowed that change.".to_owned()
        }
        (Ask::Change(_), Reply::Change(_)) => "You declined that change.".to_owned(),
        (Ask::Plan(..), Reply::Plan(PlanOutcome::Approved)) => "You approved the plan.".to_owned(),
        (Ask::Plan(..), Reply::Plan(PlanOutcome::Rejected(Some(why)))) => {
            format!("You did not approve the plan: {why}")
        }
        (Ask::Plan(..), Reply::Plan(_)) => "You did not approve the plan.".to_owned(),
        (Ask::Question(_), Reply::Answer(Answer::Text(text))) => format!("You answered: {text}"),
        _ => "You did not answer.".to_owned(),
    }
}
