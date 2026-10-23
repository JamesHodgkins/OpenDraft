//! `commands.list` and `commands.invoke`: the registry, gated by the
//! automation policy table.

use od_api::command_ops::{CommandItem, CommandsResult, InvokeArgs, InvokeResult, ListArgs};
use od_api::{ApiError, ErrorCode, WireId};
use od_commands::{AutomationPolicy, CommandEffect, ExecuteError, Registry};

use super::guard::open_for_change;
use super::{document_id, read::parse};
use crate::state::AppState;

fn policy_name(policy: AutomationPolicy) -> &'static str {
    match policy {
        AutomationPolicy::Denied => "denied",
        AutomationPolicy::ImmediateSafe => "immediate_safe",
        AutomationPolicy::InteractiveInApp => "interactive_in_app",
    }
}

fn effect_name(effect: CommandEffect) -> &'static str {
    match effect {
        CommandEffect::ReadOnly => "read_only",
        CommandEffect::UiState => "ui_state",
        CommandEffect::DrawingMutation => "drawing_mutation",
        CommandEffect::FileRead => "file_read",
        CommandEffect::FileWrite => "file_write",
        CommandEffect::ApplicationLifecycle => "application_lifecycle",
    }
}

fn encode(value: &impl serde::Serialize) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}

pub fn list(
    state: &AppState,
    registry: &Registry<AppState>,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: ListArgs = parse(arguments)?;
    let commands = registry
        .iter()
        .filter(|c| args.include_denied || c.automation != AutomationPolicy::Denied)
        .map(|c| {
            let enabled = registry.is_enabled(c.id, state);
            CommandItem {
                id: c.id.to_owned(),
                label: c.label.to_owned(),
                enabled,
                checked: registry.is_checked(c.id, state),
                policy: policy_name(c.automation).to_owned(),
                effect: effect_name(c.effect).to_owned(),
                invocable: enabled && c.automation == AutomationPolicy::ImmediateSafe,
            }
        })
        .collect();
    let active = state
        .docs
        .get(state.active)
        .filter(|d| d.block_edit.is_none());
    encode(&CommandsResult {
        active_document_id: active.map(|d| document_id(instance, d.uid)),
        commands,
    })
}

pub fn invoke(
    state: &mut AppState,
    registry: &Registry<AppState>,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: InvokeArgs = parse(arguments)?;
    let command = registry.get(&args.command_id).ok_or_else(|| {
        ApiError::new(ErrorCode::InvalidArguments, "No such command.")
            .detail("command_id", &args.command_id)
    })?;
    if command.automation != AutomationPolicy::ImmediateSafe {
        let interactive = command.automation == AutomationPolicy::InteractiveInApp;
        return Err(ApiError::new(
            ErrorCode::CommandNotAutomatable,
            if interactive {
                "This command asks the user for points and choices, so it cannot be run from here. Use the entity tools to make the change."
            } else {
                "This command is not available to automation."
            },
        )
        .detail("command_id", command.id)
        .detail("policy", policy_name(command.automation))
        .detail("effect", effect_name(command.effect)));
    }
    // A command that changes the drawing must be pinned to a revision.
    let expected = match (command.effect, args.expected_revision) {
        (CommandEffect::DrawingMutation, None) => {
            return Err(ApiError::new(
                ErrorCode::InvalidArguments,
                "This command changes the drawing: give expected_revision.",
            ));
        }
        (CommandEffect::DrawingMutation, Some(r)) => Some(r.0),
        _ => None,
    };
    // A dialog works on a copy of settings it writes back on OK, which would
    // quietly undo what the command just did.
    if state.modal_open() {
        return Err(ApiError::new(
            ErrorCode::OperationBusy,
            "A dialog is open in OpenDraft. Ask the user to close it, then try again.",
        )
        .retryable());
    }
    let index = open_for_change(state, instance, &args.document_id, expected)?;
    if index != state.active {
        return Err(ApiError::new(
            ErrorCode::InvalidReference,
            "Commands act on the drawing in the active tab. Ask the user to switch to it.",
        )
        .detail("document_id", &args.document_id));
    }
    let before = state.docs[index].session.revision();
    registry
        .execute(command.id, state)
        .map_err(|error| match error {
            ExecuteError::Disabled(_) | ExecuteError::Unknown(_) => ApiError::new(
                ErrorCode::InvalidArguments,
                "The command is not available right now: its conditions are not met.",
            )
            .detail("command_id", command.id),
        })?;
    let doc = &state.docs[index];
    encode(&InvokeResult {
        command_id: command.id.to_owned(),
        document_id: document_id(instance, doc.uid),
        revision_before: WireId(before),
        revision_after: WireId(doc.session.revision()),
        checked: registry.is_checked(command.id, state),
        message: state.message.clone(),
    })
}
