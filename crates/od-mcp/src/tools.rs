//! The MCP face of the tool catalogue: each `od_api::catalogue` tool as an
//! `rmcp` tool with its hints, the one tool only this program answers (which
//! OpenDraft instances there are), and how a call is made. What the tools
//! are is the catalogue's; what MCP needs of them is here.

use std::sync::Arc;

use od_api::catalogue::{self, Effect, ToolDef};
use od_api::ops;
use od_api::{ApiError, ErrorCode};
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde_json::{json, Value};

use crate::client::Client;
use crate::wait;

pub const INSTANCES_LIST: &str = "opendraft_instances_list";
pub use catalogue::RENDER;

/// MCP hints for a tool. Hints only: the app enforces its own grants.
fn hints(effect: Effect, open_world: bool) -> ToolAnnotations {
    let base = ToolAnnotations::new().open_world(open_world);
    match effect {
        Effect::Read => base.read_only(true).destructive(false).idempotent(true),
        Effect::UiState => base.read_only(false).destructive(false).idempotent(true),
        Effect::Adds => base.read_only(false).destructive(false).idempotent(false),
        Effect::Destructive => base.read_only(false).destructive(true).idempotent(false),
    }
}

/// A catalogue tool as MCP lists it. Waiting on an operation is this
/// program's own addition (the app never blocks), so its schema is offered
/// only here.
fn tool_for(def: &ToolDef) -> Tool {
    let mut schema = def.input_schema();
    if def.operation == ops::OPERATIONS_GET {
        if let Some(Value::Object(properties)) = schema.get_mut("properties") {
            wait::offer(properties);
        }
    }
    Tool::new(def.name, def.description, Arc::new(schema))
        .with_annotations(hints(def.effect, def.reaches_files()))
}

/// The one tool answered here: which instances can be connected to.
fn instances_tool() -> Tool {
    Tool::new(
        INSTANCES_LIST,
        "List running OpenDraft instances this server can connect to.",
        Arc::new(catalogue::no_arguments()),
    )
    .with_annotations(hints(Effect::Read, false))
}

pub fn list() -> Vec<Tool> {
    std::iter::once(instances_tool())
        .chain(catalogue::tools().iter().map(tool_for))
        .collect()
}

pub fn is_known(name: &str) -> bool {
    name == INSTANCES_LIST || catalogue::find(name).is_some()
}

fn instances(client: &Client) -> Value {
    let found: Vec<Value> = client
        .instances()
        .into_iter()
        .map(|f| {
            let mut info = serde_json::to_value(f.descriptor.info()).unwrap_or(Value::Null);
            if let Value::Object(map) = &mut info {
                map.insert("reachable".to_owned(), Value::Bool(f.reachable));
            }
            info
        })
        .collect();
    json!({ "instances": found })
}

/// Run a tool. `Err` is a domain error to show the model; an unknown tool
/// must be rejected by the caller first (see [`is_known`]).
pub fn invoke(client: &Client, name: &str, arguments: JsonObject) -> Result<Value, ApiError> {
    if name == INSTANCES_LIST {
        return Ok(instances(client));
    }
    let def = catalogue::find(name)
        .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, "Unknown tool."))?;
    match def.operation {
        ops::OPERATIONS_GET => {
            let mut arguments = arguments;
            let patience = wait::take(&mut arguments)?;
            let arguments = Value::Object(arguments);
            wait::until_finished(patience, || client.call(ops::OPERATIONS_GET, &arguments))
        }
        operation => client.call(operation, &Value::Object(arguments)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_say_what_each_tool_really_does() {
        let hints = |name: &str| {
            list()
                .into_iter()
                .find(|t| t.name == name)
                .and_then(|t| t.annotations)
                .unwrap()
        };
        for read in [
            "opendraft_instances_list",
            "opendraft_documents_list",
            "opendraft_document_get",
            "opendraft_entities_query",
            "opendraft_entities_get",
            "opendraft_layers_list",
            "opendraft_selection_get",
            "opendraft_geometry_measure",
            "opendraft_geometry_intersections",
            "opendraft_drawing_summary",
            "opendraft_drawing_audit",
            "opendraft_view_render",
            "opendraft_commands_list",
            "opendraft_operations_get",
            "opendraft_events_poll",
        ] {
            let h = hints(read);
            assert_eq!(h.read_only_hint, Some(true), "{read}");
            assert_eq!(h.destructive_hint, Some(false), "{read}");
            assert_eq!(h.idempotent_hint, Some(true), "{read}");
        }
        let edit = hints("opendraft_entities_apply_batch");
        assert_eq!(edit.read_only_hint, Some(false));
        assert_eq!(edit.destructive_hint, Some(true));
        assert_eq!(edit.idempotent_hint, Some(false));
        let select = hints("opendraft_selection_set");
        assert_eq!(select.destructive_hint, Some(false));
        assert_eq!(select.idempotent_hint, Some(true));
        assert_eq!(hints("opendraft_history_undo").destructive_hint, Some(true));
        for tool in list() {
            let reaches_files = catalogue::find(&tool.name).is_some_and(|d| d.reaches_files());
            assert_eq!(
                tool.annotations.unwrap().open_world_hint,
                Some(reaches_files),
                "{}",
                tool.name
            );
        }
        let save = hints("opendraft_files_save");
        assert_eq!(save.destructive_hint, Some(true));
        assert_eq!(hints("opendraft_operations_get").read_only_hint, Some(true));
    }

    #[test]
    fn the_listing_stays_small() {
        let tools = list();
        let size = serde_json::to_string(&tools).unwrap().len();
        let biggest = tools
            .iter()
            .map(|t| {
                (
                    serde_json::to_string(&*t.input_schema).unwrap().len(),
                    t.name.clone(),
                )
            })
            .max();
        if let Some(path) = std::env::var_os("OD_SCHEMA_DUMP") {
            let _ = std::fs::write(
                path,
                serde_json::to_string_pretty(&tools).unwrap_or_default(),
            );
        }
        // An alarm for bloat, raised when a tool is added on purpose (28 tools
        // now); the remedy for a real squeeze is in ARCHITECTURE.md.
        assert!(
            size < 56_000,
            "tools/list is {size} bytes; biggest schema {biggest:?}"
        );
    }
}
