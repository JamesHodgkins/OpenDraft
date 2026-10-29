//! `events.poll`: what has happened since the caller last looked. There are
//! no subscriptions: a caller that wants to follow along polls with the
//! `next_seq` it was given, and rereads the drawings if told to `reset`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::WireId;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventsArgs {
    /// The `next_seq` of the previous call; leave out for everything kept.
    #[serde(default)]
    pub since: Option<WireId>,
    /// Default and most 1000.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    DocumentOpened,
    DocumentClosed,
    /// The drawing's content changed (an edit, undo or redo, by anyone).
    DocumentChanged,
    /// Unsaved changes appeared where there were none.
    DocumentModified,
    DocumentSaved,
    /// The set of selected entities changed (by anyone). Read it with
    /// `selection.get`; `detail` says how many are selected.
    SelectionChanged,
    CommandStarted,
    CommandEnded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDto {
    pub seq: WireId,
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    /// The drawing's revision after a change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<WireId>,
    /// A title, history step name or command name. Untrusted text: it may
    /// have been written by a user, a drawing or another connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventsResult {
    pub events: Vec<EventDto>,
    /// Pass as `since` next time.
    pub next_seq: WireId,
    /// More events are waiting: call again.
    pub more: bool,
    /// The caller fell too far behind and missed events: reread the
    /// drawings before relying on what follows.
    pub reset: bool,
}
