//! What the chat says about the tools and the stops, in a drafter's words
//! rather than the program's.

use od_agent::{Failure, TaskState};

/// (tool, while it runs, once it has run)
const TOOLS: &[(&str, &str, &str)] = &[
    (
        "opendraft_documents_list",
        "Looking at the open drawings",
        "Looked at the open drawings",
    ),
    (
        "opendraft_document_get",
        "Reading the drawing's details",
        "Read the drawing's details",
    ),
    (
        "opendraft_entities_query",
        "Looking through the objects",
        "Looked through the objects",
    ),
    (
        "opendraft_entities_get",
        "Reading some objects",
        "Read some objects",
    ),
    (
        "opendraft_layers_list",
        "Reading the layers",
        "Read the layers",
    ),
    (
        "opendraft_selection_get",
        "Checking what is selected",
        "Checked what is selected",
    ),
    ("opendraft_geometry_measure", "Measuring", "Measured"),
    (
        "opendraft_geometry_intersections",
        "Finding where objects cross",
        "Found where objects cross",
    ),
    (
        "opendraft_drawing_summary",
        "Summarising the drawing",
        "Summarised the drawing",
    ),
    (
        "opendraft_drawing_audit",
        "Checking the drawing for problems",
        "Checked the drawing for problems",
    ),
    (
        "opendraft_view_render",
        "Looking at the drawing",
        "Looked at the drawing",
    ),
    (
        "opendraft_selection_request",
        "Waiting for you to pick objects",
        "Asked you to pick objects",
    ),
    (
        "opendraft_entities_apply_batch",
        "Changing the drawing",
        "Changed the drawing",
    ),
    (
        "opendraft_layers_apply_batch",
        "Changing the layers",
        "Changed the layers",
    ),
    ("propose_plan", "Proposing a plan", "Proposed a plan"),
    ("ask_user", "Asking you a question", "Asked you a question"),
];

pub fn doing(tool: &str) -> String {
    TOOLS
        .iter()
        .find(|t| t.0 == tool)
        .map_or_else(|| "Working".to_owned(), |t| t.1.to_owned())
}

pub fn did(tool: &str) -> String {
    TOOLS
        .iter()
        .find(|t| t.0 == tool)
        .map_or_else(|| "Did something".to_owned(), |t| t.2.to_owned())
}

/// The line above the input while a task runs; empty when it is not.
pub fn status(state: &TaskState, tool: Option<&str>) -> String {
    match state {
        TaskState::Idle | TaskState::Completed | TaskState::Cancelled | TaskState::Failed(_) => {
            String::new()
        }
        TaskState::Planning | TaskState::CallingModel | TaskState::Paused => {
            "Thinking...".to_owned()
        }
        TaskState::RunningTool => {
            tool.map_or_else(|| "Working...".to_owned(), |t| format!("{}...", doing(t)))
        }
        TaskState::AwaitingApproval => "Waiting for your approval".to_owned(),
        TaskState::WaitingForUser => "Waiting for your answer".to_owned(),
        TaskState::Verifying => "Checking its work...".to_owned(),
    }
}

/// Why a task ended, when it did not complete.
pub fn ended(state: &TaskState) -> Option<String> {
    match state {
        TaskState::Cancelled => Some("Stopped.".to_owned()),
        TaskState::Failed(failure) => Some(match failure {
            Failure::Provider(why) => format!("The model service failed: {why}"),
            Failure::Refused(why) => format!("The model service declined: {why}"),
            Failure::Budget(kind) => format!("Stopped: it reached {}.", kind.describe()),
            Failure::Model(why) => format!("The assistant could not go on: {why}"),
            Failure::Internal(why) => format!("Something went wrong inside the assistant: {why}"),
        }),
        _ => None,
    }
}
