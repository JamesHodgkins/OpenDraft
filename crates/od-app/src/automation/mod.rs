//! The automation boundary: what an out-of-process caller (the MCP server)
//! may ask of the running app. Requests are answered on the UI thread by
//! plain functions over `AppState`; the transport (`ipc`, `host`) only moves
//! framed JSON to and from them.

mod audit;
#[cfg(test)]
mod audit_tests;
#[cfg(test)]
mod budget_tests;
mod command_ops;
mod command_runs;
#[cfg(test)]
mod command_tests;
mod connection;
mod core;
#[cfg(test)]
mod core_tests;
mod events;
#[cfg(test)]
mod file_guard_tests;
mod file_ops;
mod file_state;
#[cfg(test)]
mod file_tests;
mod geometry;
#[cfg(test)]
mod geometry_tests;
mod grants;
#[cfg(test)]
mod grants_tests;
mod guard;
#[cfg(test)]
mod hatch_batch_tests;
mod hatch_mutate;
mod host;
#[cfg(test)]
mod host_tests;
mod idempotency;
#[cfg(test)]
mod key_tests;
mod layer_mutate;
mod mutate;
#[cfg(test)]
mod mutate_tests;
mod parts;
#[cfg(test)]
mod parts_tests;
mod policy;
mod query;
mod read;
mod render;
#[cfg(test)]
mod render_png_tests;
#[cfg(test)]
mod run_tests;
mod scope;
#[cfg(test)]
mod selection_guard_tests;
#[cfg(test)]
mod selection_host_tests;
mod selection_request;
#[cfg(test)]
mod selection_tests;
mod settings_page;
mod setup;
#[cfg(test)]
mod socket_tests;
mod summary;
#[cfg(test)]
mod summary_tests;
#[cfg(test)]
mod tests;
mod ui_mutate;
#[cfg(test)]
mod ui_tests;

pub use self::core::{fresh_instance_id, AutomationCore, InProcess};
pub use self::grants::Coverage;
pub use events::EventLog;
pub use file_ops::{answer_consent, consent_text};
pub use file_state::FileOps;
pub use guard::Caller;
pub use host::SocketServer;
pub use idempotency::Replays;
pub use mutate::apply_batch;
pub use policy::classified;
pub use read::handle_read;
pub(crate) use selection_request::withdraw as withdraw_selection_requests;
pub use settings_page::{page as settings_page, ConnectionRow};

use od_api::{ops, ApiError};
use od_commands::Registry;

use crate::state::{AppState, Doc};

/// Run one operation on the UI thread. The caller has already checked the
/// connection's scope. An operation that accepts an `idempotency_key` and
/// is sent one it has already done, with the same arguments, is not done
/// again: the first answer comes back, marked `replayed`.
pub fn dispatch(
    state: &mut AppState,
    registry: &Registry<AppState>,
    instance: &str,
    caller: &Caller,
    operation: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let (arguments, key) = idempotency::take_key(operation, arguments)?;
    let Some(key) = key else {
        return route(state, registry, instance, caller, operation, arguments);
    };
    match state
        .replays
        .look(caller.grant_id, operation, &key, &arguments)
    {
        idempotency::Seen::Answer(answer) => return Ok(answer),
        idempotency::Seen::Reused => return Err(idempotency::reused(&key)),
        idempotency::Seen::New => {}
    }
    let result = route(
        state,
        registry,
        instance,
        caller,
        operation,
        arguments.clone(),
    );
    if let Ok(answer) = &result {
        state
            .replays
            .keep(caller.grant_id, operation, &key, &arguments, answer);
    }
    result
}

fn route(
    state: &mut AppState,
    registry: &Registry<AppState>,
    instance: &str,
    caller: &Caller,
    operation: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    use od_api::session_ops::HistoryDirection::{Redo, Undo};
    match operation {
        ops::ENTITIES_APPLY_BATCH => apply_batch(state, instance, caller, arguments),
        ops::LAYERS_APPLY_BATCH => layer_mutate::apply_layers(state, instance, caller, arguments),
        ops::PARTS_DRAW => parts::draw(state, instance, caller, arguments),
        ops::SELECTION_SET => ui_mutate::set_selection(state, instance, arguments),
        ops::SELECTION_REQUEST => selection_request::start(state, instance, caller, arguments),
        ops::HISTORY_UNDO => ui_mutate::step_history(state, instance, caller, Undo, arguments),
        ops::HISTORY_REDO => ui_mutate::step_history(state, instance, caller, Redo, arguments),
        ops::GEOMETRY_MEASURE => geometry::measure_entities(state, instance, arguments),
        ops::GEOMETRY_INTERSECTIONS => geometry::intersect_entities(state, instance, arguments),
        ops::VIEW_RENDER => render::render(state, instance, arguments),
        ops::FILES_OPEN => file_ops::open(state, instance, caller, arguments),
        ops::FILES_SAVE => file_ops::save(state, instance, caller, arguments),
        ops::FILES_SAVE_AS => file_ops::save_as(state, instance, caller, arguments),
        ops::FILES_EXPORT => file_ops::export(state, instance, caller, arguments),
        ops::OPERATIONS_GET => file_ops::get(state, instance, caller, arguments),
        ops::OPERATIONS_CANCEL => file_ops::cancel(state, instance, caller, arguments),
        ops::COMMANDS_START_INTERACTIVE => {
            command_runs::start(state, registry, instance, caller, arguments)
        }
        ops::EVENTS_POLL => command_runs::poll_events(state, instance, arguments),
        ops::COMMANDS_LIST => command_ops::list(state, registry, instance, arguments),
        ops::COMMANDS_INVOKE => command_ops::invoke(state, registry, instance, arguments),
        _ => handle_read(state, instance, operation, arguments),
    }
}

/// `dispatch` with the real command table, for tests.
#[cfg(test)]
pub(crate) fn dispatch_test(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    operation: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let registry = crate::commands::build_registry();
    dispatch(state, &registry, instance, caller, operation, arguments)
}

/// The wire id of an open drawing: tied to this run (`instance`) and to the
/// drawing's uid, never to its tab position.
pub fn document_id(instance: &str, uid: u64) -> String {
    format!("doc_{instance}_{uid}")
}

/// Find the open drawing a wire id names, or say why there is none.
/// A drawing with the Block Editor open is refused: its session slot holds
/// the temporary block, which must never answer under the drawing's id.
pub(crate) fn resolve<'a>(
    state: &'a AppState,
    instance: &str,
    id: &str,
) -> Result<&'a Doc, ApiError> {
    let uid = id
        .strip_prefix("doc_")
        .and_then(|rest| rest.strip_prefix(instance))
        .and_then(|rest| rest.strip_prefix('_'))
        .and_then(|n| n.parse::<u64>().ok());
    let doc = uid
        .and_then(|uid| state.docs.iter().find(|d| d.uid == uid && !d.placeholder))
        .ok_or_else(|| {
            ApiError::new(
                od_api::ErrorCode::DocumentNotFound,
                "No open drawing has that id.",
            )
            .detail("document_id", id)
        })?;
    if doc.block_edit.is_some() {
        return Err(ApiError::block_editor_open());
    }
    Ok(doc)
}
