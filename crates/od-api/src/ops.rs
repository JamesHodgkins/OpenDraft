//! The read operations: their names, the scope each needs, their arguments
//! and their results. Arguments reject unknown fields and carry JSON Schema
//! so the MCP tool schemas come from the same types the app parses.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{BoundsDto, EntityDto};
use crate::{Scope, WireId};

pub const DOCUMENTS_LIST: &str = "documents.list";
pub const DOCUMENT_GET: &str = "document.get";
pub const ENTITIES_QUERY: &str = "entities.query";
pub const ENTITIES_GET: &str = "entities.get";
pub const LAYERS_LIST: &str = "layers.list";
pub const SELECTION_GET: &str = "selection.get";
pub const GEOMETRY_MEASURE: &str = "geometry.measure";
pub const GEOMETRY_INTERSECTIONS: &str = "geometry.intersections";
pub const COMMANDS_LIST: &str = "commands.list";
pub const COMMANDS_INVOKE: &str = "commands.invoke";
pub const COMMANDS_START_INTERACTIVE: &str = "commands.start_interactive";
pub const EVENTS_POLL: &str = "events.poll";
pub const FILES_OPEN: &str = "files.open";
pub const FILES_SAVE: &str = "files.save";
pub const FILES_SAVE_AS: &str = "files.save_as";
pub const FILES_EXPORT: &str = "files.export";
pub const OPERATIONS_GET: &str = "operations.get";
pub const OPERATIONS_CANCEL: &str = "operations.cancel";
pub const VIEW_RENDER: &str = "view.render";
pub const ENTITIES_APPLY_BATCH: &str = "entities.apply_batch";
pub const LAYERS_APPLY_BATCH: &str = "layers.apply_batch";
pub const SELECTION_SET: &str = "selection.set";
pub const HISTORY_UNDO: &str = "history.undo";
pub const HISTORY_REDO: &str = "history.redo";

/// The scope an operation needs, or `None` for an unknown operation.
pub fn required_scope(operation: &str) -> Option<Scope> {
    match operation {
        DOCUMENTS_LIST
        | DOCUMENT_GET
        | ENTITIES_QUERY
        | ENTITIES_GET
        | LAYERS_LIST
        | SELECTION_GET
        | GEOMETRY_MEASURE
        | GEOMETRY_INTERSECTIONS
        | VIEW_RENDER
        | COMMANDS_LIST
        | EVENTS_POLL
        // Operations are private to the connection that started them, and
        // that needed its own grant already.
        | OPERATIONS_GET
        | OPERATIONS_CANCEL => Some(Scope::DrawingRead),
        ENTITIES_APPLY_BATCH | LAYERS_APPLY_BATCH | HISTORY_UNDO | HISTORY_REDO => {
            Some(Scope::DrawingWrite)
        }
        SELECTION_SET => Some(Scope::UiSelection),
        COMMANDS_INVOKE | COMMANDS_START_INTERACTIVE => Some(Scope::CommandInvoke),
        FILES_OPEN => Some(Scope::FileRead),
        FILES_SAVE | FILES_SAVE_AS | FILES_EXPORT => Some(Scope::FileWrite),
        _ => None,
    }
}

impl JsonSchema for WireId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WireId".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^(0|[1-9][0-9]*)$",
            "description": "decimal string id"
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentArgs {
    /// An id from the documents list.
    pub document_id: String,
}

/// Which space to look at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceFilter {
    Model,
    Paper { layout_id: WireId },
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryArgs {
    pub document_id: String,
    /// Entity kinds to keep, e.g. `["line", "circle"]`.
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    #[serde(default)]
    pub layer_id: Option<WireId>,
    /// Defaults to model space.
    #[serde(default)]
    pub space: Option<SpaceFilter>,
    /// Keep entities whose bounds intersect this box.
    #[serde(default)]
    pub bounds: Option<BoundsArg>,
    /// From a previous result's `next_cursor`, with the same filters.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Page size: 100 by default, 1000 at most.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundsArg {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetArgs {
    pub document_id: String,
    /// At most 1000 ids; results come back in this order.
    pub ids: Vec<WireId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentItem {
    pub document_id: String,
    pub title: String,
    pub active: bool,
    pub modified: bool,
    /// False while the Block Editor has the document; the rest is then
    /// withheld.
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<WireId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentsResult {
    pub documents: Vec<DocumentItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub document_id: String,
    pub title: String,
    pub revision: WireId,
    pub modified: bool,
    /// A command is running in the drawing; writes would be refused.
    pub busy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy_command: Option<String>,
    pub entity_count: usize,
    pub model_entity_count: usize,
    pub layer_count: usize,
    pub layout_count: usize,
    pub block_count: usize,
    pub current_layer_id: WireId,
    pub units: UnitsSummary,
    /// Of model space, or `None` when it is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<BoundsDto>,
    pub layouts: Vec<LayoutSummary>,
    /// What undo and redo would act on next, for `history.undo`/`redo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo_step: Option<crate::session_ops::StepSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redo_step: Option<crate::session_ops::StepSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitsSummary {
    pub length: String,
    pub angle: String,
    pub length_precision: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutSummary {
    pub layout_id: WireId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub revision: WireId,
    pub entities: Vec<EntityDto>,
    /// Present while more remain; pass it back with the same filters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetResult {
    pub revision: WireId,
    /// One slot per requested id; `null` for an id that does not exist.
    pub entities: Vec<Option<EntityDto>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerItem {
    pub layer_id: WireId,
    pub name: String,
    pub color: crate::entity::ColorDto,
    pub linetype: String,
    pub lineweight_hundredths_mm: u16,
    pub visible: bool,
    pub frozen: bool,
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayersResult {
    pub revision: WireId,
    pub current_layer_id: WireId,
    pub layers: Vec<LayerItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelectionResult {
    pub revision: WireId,
    pub entity_ids: Vec<WireId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_read_operation_needs_only_read_access() {
        for op in [
            DOCUMENTS_LIST,
            DOCUMENT_GET,
            ENTITIES_QUERY,
            ENTITIES_GET,
            LAYERS_LIST,
            SELECTION_GET,
        ] {
            assert_eq!(required_scope(op), Some(Scope::DrawingRead));
        }
        assert_eq!(
            required_scope(ENTITIES_APPLY_BATCH),
            Some(Scope::DrawingWrite)
        );
        assert_eq!(required_scope("files.delete"), None);
    }

    #[test]
    fn query_arguments_reject_unknown_fields_and_bad_ids() {
        let ok = r#"{"document_id":"d","layer_id":"3","limit":5}"#;
        assert!(serde_json::from_str::<QueryArgs>(ok).is_ok());
        assert!(serde_json::from_str::<QueryArgs>(r#"{"document_id":"d","force":1}"#).is_err());
        assert!(serde_json::from_str::<QueryArgs>(r#"{"document_id":"d","layer_id":3}"#).is_err());
    }

    #[test]
    fn the_schema_for_an_id_is_a_decimal_string() {
        let schema = schemars::schema_for!(GetArgs);
        let text = serde_json::to_string(&schema).unwrap();
        assert!(text.contains("^(0|[1-9][0-9]*)$"), "{text}");
    }
}
