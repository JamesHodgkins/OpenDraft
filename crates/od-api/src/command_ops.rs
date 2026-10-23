//! `commands.list` and `commands.invoke`: the registry as an escape hatch
//! for commands that have been explicitly marked safe to run on request.
//! Structured tools are preferred; this exists for view and mode commands.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::WireId;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListArgs {
    /// Also list commands automation may not run (default: only those it
    /// may run or may start with the prompts staying in the app).
    #[serde(default)]
    pub include_denied: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InvokeArgs {
    /// A command id from `commands.list` with `invocable: true`.
    pub command_id: String,
    /// The drawing the command acts on. It must be the one the user has
    /// open in the active tab: automation never switches tabs.
    pub document_id: String,
    /// Required for a command that changes the drawing.
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

/// Start an interactive command. Its prompts stay in the app: the user
/// points and types the answers.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartArgs {
    /// A command id from `commands.list` with policy `interactive_in_app`.
    /// If entities are selected when it starts, it uses them and skips its
    /// first selection step (as when the user starts it).
    pub command_id: String,
    /// The drawing in the active tab.
    pub document_id: String,
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandItem {
    pub id: String,
    pub label: String,
    /// Whether it can run right now in the active drawing.
    pub enabled: bool,
    /// Whether its mode is on (a toggle).
    pub checked: bool,
    /// `denied`, `immediate_safe` or `interactive_in_app`.
    pub policy: String,
    /// `read_only`, `ui_state`, `drawing_mutation`, `file_read`,
    /// `file_write` or `application_lifecycle`.
    pub effect: String,
    /// `policy` allows it and it is enabled now.
    pub invocable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandsResult {
    /// The drawing in the active tab: what an invoked command acts on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_document_id: Option<String>,
    pub commands: Vec<CommandItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InvokeResult {
    pub command_id: String,
    pub document_id: String,
    pub revision_before: WireId,
    pub revision_after: WireId,
    /// Whether the command's mode is on afterwards (a toggle).
    pub checked: bool,
    /// What the app said in its status bar.
    pub message: String,
}
