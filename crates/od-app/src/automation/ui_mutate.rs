//! Selection and undo/redo: operations on the app's state, not the
//! drawing's content.

use std::collections::BTreeSet;

use od_api::limits::GET_IDS_MAX;
use od_api::session_ops::{
    HistoryArgs, HistoryDirection, HistoryResult, SelectionMode, SelectionSetArgs,
};
use od_api::{ApiError, ErrorCode, WireId};
use od_commands::{HistoryError, Provenance};
use od_core::EntityId;

use super::guard::{open_for_change, Caller};
use super::mutate::changes_dto;
use super::{document_id, read::parse};
use crate::state::AppState;

fn encode(value: &impl serde::Serialize) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}

pub fn set_selection(
    state: &mut AppState,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: SelectionSetArgs = parse(arguments)?;
    if args.entity_ids.len() > GET_IDS_MAX {
        return Err(ApiError::new(
            ErrorCode::PayloadTooLarge,
            format!("At most {GET_IDS_MAX} ids can be selected at once."),
        )
        .detail("limit", GET_IDS_MAX));
    }
    let index = open_for_change(state, instance, &args.document_id, None)?;
    let doc = &mut state.docs[index];
    let document = doc.session.document();
    let mut wanted = BTreeSet::new();
    let mut unselectable = Vec::new();
    if args.mode != SelectionMode::Clear {
        for id in &args.entity_ids {
            let Some(object) = document.object(EntityId(id.0)) else {
                if args.mode == SelectionMode::Remove {
                    continue;
                }
                return Err(ApiError::new(ErrorCode::EntityNotFound, "No such entity.")
                    .detail("entity_id", id));
            };
            if args.mode != SelectionMode::Remove && !document.is_selectable(object) {
                unselectable.push(id.to_string());
            } else {
                wanted.insert(object.id);
            }
        }
    }
    if !unselectable.is_empty() && !args.ignore_unselectable {
        return Err(ApiError::new(
            ErrorCode::InvalidReference,
            "Some entities are hidden, frozen or locked, so they cannot be selected.",
        )
        .detail("unselectable", unselectable.join(",")));
    }
    match args.mode {
        SelectionMode::Replace => doc.selection = wanted,
        SelectionMode::Add => doc.selection.extend(wanted),
        SelectionMode::Remove => doc.selection.retain(|id| !wanted.contains(id)),
        SelectionMode::Clear => doc.selection.clear(),
    }
    encode(&od_api::ops::SelectionResult {
        revision: WireId(doc.session.revision()),
        entity_ids: doc.selection.iter().map(|id| WireId(id.0)).collect(),
    })
}

pub fn step_history(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    direction: HistoryDirection,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: HistoryArgs = parse(arguments)?;
    let index = open_for_change(
        state,
        instance,
        &args.document_id,
        Some(args.expected_revision.0),
    )?;
    let doc = &mut state.docs[index];
    let before = doc.session.revision();
    // Your own steps, unless the user also gave you the history of others.
    let allowed = |made_by: Provenance| {
        caller.history_any
            || matches!(made_by, Provenance::Automation(grant) if grant == caller.grant_id)
    };
    let outcome = match direction {
        HistoryDirection::Undo => doc.session.undo_checked(args.step_token.0, allowed),
        HistoryDirection::Redo => doc.session.redo_checked(args.step_token.0, allowed),
    };
    let step = outcome.map_err(|error| match error {
        HistoryError::Empty => ApiError::new(ErrorCode::InvalidArguments, "There is nothing to do."),
        HistoryError::Moved { actual } => ApiError::new(
            ErrorCode::StaleRevision,
            "The step at the top of the history is not the one you saw.",
        )
        .retryable()
        .detail("actual_step_token", actual),
        HistoryError::NotPermitted(_) => ApiError::new(
            ErrorCode::ScopeDenied,
            "That step was not made by this connection. Ask the user to allow changing their history.",
        )
        .detail("required", "history.any"),
    })?;
    doc.prune_selection();
    encode(&HistoryResult {
        document_id: document_id(instance, doc.uid),
        step_name: step.name,
        revision_before: WireId(before),
        revision_after: WireId(doc.session.revision()),
        changes: changes_dto(&step.changes),
    })
}
