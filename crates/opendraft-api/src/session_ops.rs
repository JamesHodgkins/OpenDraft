//! Operations on the app's state rather than the drawing's content:
//! selection, and undo/redo of the history.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::write::ChangesDto;
use crate::WireId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    Replace,
    Add,
    Remove,
    Clear,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectionSetArgs {
    pub document_id: String,
    pub mode: SelectionMode,
    /// Ignored for `clear`.
    #[serde(default)]
    pub entity_ids: Vec<WireId>,
    /// Skip entities that are hidden, frozen or locked instead of failing.
    #[serde(default)]
    pub ignore_unselectable: bool,
}

/// Ask the user to select objects in the app. They pick as usual and press
/// Enter, or Esc to refuse; the operation then carries the ids.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectionRequestArgs {
    /// The drawing in the active tab.
    pub document_id: String,
    /// What to ask, in a few words (200 characters at most, plain text). The
    /// app labels it as the assistant's.
    pub prompt: String,
    /// Accept only these kinds, e.g. `["line", "circle"]`; other picks are
    /// dropped from the selection.
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    /// Accept only objects on this layer.
    #[serde(default)]
    pub layer_id: Option<WireId>,
    /// Fewest to finish with: 1 by default; 0 allows none.
    #[serde(default)]
    pub min_count: Option<usize>,
    /// Most to finish with: 1000 by default and at most.
    #[serde(default)]
    pub max_count: Option<usize>,
    /// Start from the user's current selection (default: empty).
    #[serde(default)]
    pub keep_current_selection: bool,
    /// Seconds to wait for the user: 300 by default, 900 at most.
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
    /// Refuse if the drawing has changed since this revision.
    #[serde(default)]
    pub expected_revision: Option<WireId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HistoryDirection {
    Undo,
    Redo,
}

/// Undo or redo the step at the top of the history, if it is the one the
/// caller saw and the caller may touch it.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryArgs {
    pub document_id: String,
    pub expected_revision: WireId,
    /// The `token` of the undo (or redo) step in the drawing summary, or the
    /// `history_token` an edit returned.
    pub step_token: WireId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryResult {
    pub document_id: String,
    pub step_name: String,
    pub revision_before: WireId,
    pub revision_after: WireId,
    pub changes: ChangesDto,
}

/// A step in the history, as the drawing summary shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepSummary {
    pub token: WireId,
    pub name: String,
    /// `user`, or `automation` for a step an automation connection made.
    pub made_by: String,
}
