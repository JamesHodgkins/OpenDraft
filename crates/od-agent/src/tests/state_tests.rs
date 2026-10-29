//! The task states and the workflows' shape.

use od_api::catalogue;

use crate::{changes_drawing, edit, qa_review, BudgetKind, Failure, TaskState};
use TaskState::*;

fn all() -> Vec<TaskState> {
    vec![
        Idle,
        Planning,
        AwaitingApproval,
        CallingModel,
        RunningTool,
        WaitingForUser,
        Verifying,
        Paused,
        Completed,
        Failed(Failure::Budget(BudgetKind::Time)),
        Cancelled,
    ]
}

#[test]
fn a_finished_task_never_moves_again() {
    for from in all().into_iter().filter(TaskState::is_finished) {
        for to in all() {
            assert!(!from.may_become(&to), "{from:?} -> {to:?}");
        }
    }
}

#[test]
fn any_live_task_can_be_cancelled_or_fail() {
    for from in all().into_iter().filter(|s| !s.is_finished()) {
        assert!(from.may_become(&Cancelled), "{from:?}");
        assert!(from.may_become(&Failed(Failure::Internal(String::new()))));
    }
}

#[test]
fn only_a_model_turn_completes_a_task() {
    for from in all() {
        let may = from.may_become(&Completed);
        assert_eq!(may, matches!(from, CallingModel | Verifying), "{from:?}");
    }
}

#[test]
fn a_task_starts_by_planning_or_asking_the_model_and_nothing_else() {
    for to in all() {
        let may = Idle.may_become(&to);
        let expected = matches!(to, Planning | CallingModel | Cancelled | Failed(_));
        assert_eq!(may, expected, "Idle -> {to:?}");
    }
}

#[test]
fn a_tool_never_runs_straight_after_the_model_without_a_turn_to_choose_it() {
    // A tool runs from a model's turn, a finished tool, a plan or a question
    // answered; never from Idle, a paused task, or a finished one.
    for from in [Idle, Paused, Completed, Cancelled] {
        assert!(!from.may_become(&RunningTool), "{from:?}");
    }
}

#[test]
fn the_review_workflow_offers_no_tool_that_changes_the_drawing() {
    let workflow = qa_review();
    for name in workflow.tools {
        let tool = catalogue::find(name);
        assert!(tool.is_some(), "{name} is not in the catalogue");
        if let Some(tool) = tool {
            assert!(!changes_drawing(tool.operation), "{name}");
            assert!(!tool.reaches_files(), "{name}");
        }
    }
    // Even allowing writes, nothing that writes is on the list.
    let with = workflow.offered(true).len();
    assert_eq!(with, workflow.offered(false).len());
}

#[test]
fn the_edit_workflow_holds_back_exactly_the_tools_that_change_the_drawing() {
    let workflow = edit();
    let before = workflow.offered(false);
    let after = workflow.offered(true);
    let held: Vec<&str> = after
        .iter()
        .filter(|a| !before.iter().any(|b| b.name == a.name))
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        held,
        [
            "opendraft_parts_draw",
            "opendraft_entities_apply_batch",
            "opendraft_layers_apply_batch"
        ]
    );
    for name in workflow.tools {
        let tool = catalogue::find(name);
        assert!(tool.is_some(), "{name} is not in the catalogue");
        assert!(!tool.is_some_and(|t| t.reaches_files()), "{name}");
    }
}

#[test]
fn the_review_workflows_tool_list_is_a_fraction_of_the_whole() {
    let size = |tools: Vec<crate::ToolSpec>| -> usize {
        tools
            .iter()
            .map(|t| {
                t.name.len()
                    + t.description.len()
                    + serde_json::to_string(&t.schema).map_or(0, |s| s.len())
            })
            .sum()
    };
    let review = size(qa_review().offered(false));
    let all_tools: usize = catalogue::tools()
        .iter()
        .map(|t| {
            t.name.len()
                + t.description.len()
                + serde_json::to_string(&t.input_schema()).map_or(0, |s| s.len())
        })
        .sum();
    assert!(review * 3 < all_tools * 2, "{review} of {all_tools}");
}
