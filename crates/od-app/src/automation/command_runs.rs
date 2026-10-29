//! Interactive commands started on request. The prompts belong to the user:
//! the command waits in the app for their points and typing, and the caller
//! can only start it, watch what it is asking, and cancel it. A command run
//! is an operation like a file job, polled with `operations.get`.

use od_api::command_ops::StartArgs;
use od_api::{ApiError, ErrorCode};
use od_commands::{AutomationPolicy, Outcome, Registry};

use super::file_ops::not_too_many;
use super::file_state::Kind;
use super::guard::{open_for_change, Caller};
use super::read::parse;
use crate::state::AppState;

/// How long starting a command waits for its first prompt.
const FIRST_PROMPT_WAIT: std::time::Duration = std::time::Duration::from_millis(100);
/// Longest prompt passed on.
const PROMPT_MAX: usize = 200;

pub(super) fn encode(value: &impl serde::Serialize) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}

/// Bring every running command operation (and selection request) up to
/// date: what its command is asking now, or how it ended. Cheap; called
/// wherever operations are read.
pub(super) fn sync(state: &mut AppState) {
    super::selection_request::expire(state);
    for (id, serial, uid) in state.file_ops.running_commands() {
        let running = state.command_serial == serial && state.command_running();
        if running {
            let prompt = state.waiting().map(|ask| {
                ask.message
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(PROMPT_MAX)
                    .collect::<String>()
            });
            if let Some(op) = state.file_ops.get_mut(id) {
                op.waiting_for = prompt;
            }
            continue;
        }
        let revision = state
            .docs
            .iter()
            .find(|d| d.uid == uid)
            .map(|d| d.session.revision());
        let outcome = state.command_end(serial);
        if let Some(op) = state.file_ops.get_mut(id) {
            op.waiting_for = None;
            op.revision = revision;
        }
        let selection = state
            .file_ops
            .get(id)
            .is_some_and(|op| op.kind == Kind::Selection);
        match outcome {
            Some(Outcome::Completed) if selection => super::selection_request::settle(state, id),
            Some(Outcome::Completed) => state.file_ops.succeed(id, Vec::new()),
            Some(Outcome::Crashed) => state.file_ops.fail(
                id,
                ErrorCode::OperationFailed,
                "The command stopped unexpectedly.",
            ),
            // Cancelled by the user, or replaced by another command.
            Some(Outcome::Cancelled) | None => state.file_ops.cancel(id),
        }
    }
}

/// `commands.start_interactive`.
pub fn start(
    state: &mut AppState,
    registry: &Registry<AppState>,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: StartArgs = parse(arguments)?;
    sync(state);
    not_too_many(state, caller)?;
    let command = registry.get(&args.command_id).ok_or_else(|| {
        ApiError::new(ErrorCode::InvalidArguments, "No such command.")
            .detail("command_id", &args.command_id)
    })?;
    match command.automation {
        AutomationPolicy::InteractiveInApp => {}
        AutomationPolicy::ImmediateSafe => {
            return Err(ApiError::new(
                ErrorCode::InvalidArguments,
                "This command needs no answers: run it with commands_invoke.",
            )
            .detail("command_id", command.id));
        }
        AutomationPolicy::Denied => {
            return Err(ApiError::new(
                ErrorCode::CommandNotAutomatable,
                "This command is not available to automation.",
            )
            .detail("command_id", command.id));
        }
    }
    // Starting one command would end the one the user is in the middle of.
    if let Some(runner) = state.runner.as_ref() {
        return Err(ApiError::command_busy(runner.name()));
    }
    // A dialog is the user's focus right now.
    if state.modal_open() {
        return Err(ApiError::new(
            ErrorCode::OperationBusy,
            "A dialog is open in OpenDraft. Ask the user to close it, then try again.",
        )
        .retryable());
    }
    let expected = args.expected_revision.map(|r| r.0);
    let index = open_for_change(state, instance, &args.document_id, expected)?;
    if index != state.active {
        return Err(ApiError::new(
            ErrorCode::InvalidReference,
            "Commands run in the active tab. Ask the user to switch to that drawing.",
        )
        .detail("document_id", &args.document_id));
    }
    if !registry.is_enabled(command.id, state) {
        return Err(ApiError::new(
            ErrorCode::InvalidArguments,
            "The command is not available right now: its conditions are not met (it may need objects to work on).",
        )
        .detail("command_id", command.id));
    }
    registry.execute(command.id, state).map_err(|_| {
        ApiError::new(
            ErrorCode::InvalidArguments,
            "The command could not be started.",
        )
        .detail("command_id", command.id)
    })?;
    // Let it ask its first question, so the answer can say what it is.
    state.pump_command_for(FIRST_PROMPT_WAIT);
    let uid = state.docs[index].uid;
    let serial = state.command_serial;
    let id = state
        .file_ops
        .add_command(caller.grant_id, caller.client, command.id, uid, serial);
    sync(state);
    match state.file_ops.get(id) {
        Some(op) => encode(&op.status(instance)),
        None => Err(ApiError::new(
            ErrorCode::InternalError,
            "The operation vanished.",
        )),
    }
}

/// `events.poll`.
pub fn poll_events(
    state: &mut AppState,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: od_api::event_ops::EventsArgs = parse(arguments)?;
    state.observe_events();
    let result = state.events.since(
        instance,
        args.since.map(|s| s.0),
        args.limit.unwrap_or(od_api::limits::QUERY_PAGE_DEFAULT),
    );
    encode(&result)
}
