//! The read operations: documents, one drawing's summary, layers and
//! selection. Entity queries are in `query.rs`.

use od_api::entity::{color_dto, entity_dto};
use od_api::limits::GET_IDS_MAX;
use od_api::ops::{
    self, DocumentArgs, DocumentItem, DocumentSummary, DocumentsResult, GetArgs, GetResult,
    LayerItem, LayersResult, LayoutSummary, SelectionResult, UnitsSummary,
};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::Attr;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::{document_id, query, resolve};
use crate::state::{AppState, Doc};

pub(super) fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, ApiError> {
    // A request with no arguments arrives as `null`.
    let arguments = if arguments.is_null() {
        Value::Object(Default::default())
    } else {
        arguments
    };
    serde_json::from_value(arguments)
        .map_err(|e| ApiError::new(ErrorCode::InvalidArguments, e.to_string()))
}

fn to_value<T: Serialize>(result: &T) -> Result<Value, ApiError> {
    serde_json::to_value(result)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}

/// Answer a read operation for `instance`. Unknown operations are an error:
/// the transport has already checked the caller's scope.
pub fn handle_read(
    state: &AppState,
    instance: &str,
    operation: &str,
    arguments: Value,
) -> Result<Value, ApiError> {
    match operation {
        ops::DOCUMENTS_LIST => to_value(&documents(state, instance)),
        ops::DOCUMENT_GET => {
            let args: DocumentArgs = parse(arguments)?;
            to_value(&summary(state, instance, &args.document_id)?)
        }
        ops::ENTITIES_QUERY => to_value(&query::entities(state, instance, parse(arguments)?)?),
        ops::ENTITIES_GET => to_value(&get(state, instance, parse(arguments)?)?),
        ops::LAYERS_LIST => {
            let args: DocumentArgs = parse(arguments)?;
            to_value(&layers(resolve(state, instance, &args.document_id)?))
        }
        ops::SELECTION_GET => {
            let args: DocumentArgs = parse(arguments)?;
            to_value(&selection(resolve(state, instance, &args.document_id)?))
        }
        other => Err(ApiError::new(
            ErrorCode::InvalidArguments,
            format!("Unknown operation {other:?}."),
        )),
    }
}

fn documents(state: &AppState, instance: &str) -> DocumentsResult {
    let documents = state
        .docs
        .iter()
        .enumerate()
        .map(|(index, doc)| {
            // A drawing in the Block Editor says nothing about itself: what
            // its session holds right now is the block, not the drawing.
            let available = doc.block_edit.is_none();
            DocumentItem {
                document_id: document_id(instance, doc.uid),
                title: doc.title.clone(),
                active: index == state.active,
                modified: doc.is_modified(),
                available,
                unavailable_reason: (!available).then(|| "block_editor_open".to_owned()),
                revision: available.then(|| WireId(doc.session.revision())),
            }
        })
        .collect();
    DocumentsResult { documents }
}

fn summary(state: &AppState, instance: &str, id: &str) -> Result<DocumentSummary, ApiError> {
    let doc = resolve(state, instance, id)?;
    let index = state.docs.iter().position(|d| d.uid == doc.uid);
    let document = doc.session.document();
    let units = document.units();
    let busy_command = state
        .runner
        .as_ref()
        .filter(|_| index == Some(state.run_doc))
        .map(|runner| runner.name().to_owned());
    Ok(DocumentSummary {
        document_id: document_id(instance, doc.uid),
        title: doc.title.clone(),
        revision: WireId(doc.session.revision()),
        modified: doc.is_modified(),
        busy: busy_command.is_some(),
        busy_command,
        entity_count: document.len(),
        model_entity_count: document
            .objects()
            .iter()
            .filter(|o| o.layout.is_none())
            .count(),
        layer_count: document.layers().len(),
        layout_count: document.layouts().len(),
        block_count: document.blocks().len(),
        current_layer_id: WireId(document.current_layer().0),
        units: UnitsSummary {
            length: format!("{:?}", units.length).to_lowercase(),
            angle: format!("{:?}", units.angle).to_lowercase(),
            length_precision: units.length_precision,
        },
        bounds: document.model_bounds().map(Into::into),
        undo_step: step(doc.session.history().top_undo()),
        redo_step: step(doc.session.history().top_redo()),
        layouts: document
            .layouts()
            .iter()
            .map(|l| LayoutSummary {
                layout_id: WireId(l.id.0),
                name: l.name.clone(),
            })
            .collect(),
    })
}

fn step(info: Option<od_commands::StepInfo>) -> Option<od_api::session_ops::StepSummary> {
    info.map(|s| od_api::session_ops::StepSummary {
        token: WireId(s.token),
        name: s.name,
        made_by: match s.provenance {
            od_commands::Provenance::User => "user",
            od_commands::Provenance::Automation(_) => "automation",
        }
        .to_owned(),
    })
}

fn get(state: &AppState, instance: &str, args: GetArgs) -> Result<GetResult, ApiError> {
    if args.ids.len() > GET_IDS_MAX {
        return Err(ApiError::new(
            ErrorCode::PayloadTooLarge,
            format!("At most {GET_IDS_MAX} ids can be fetched at once."),
        )
        .detail("limit", GET_IDS_MAX));
    }
    let doc = resolve(state, instance, &args.document_id)?;
    let document = doc.session.document();
    let entities = args
        .ids
        .iter()
        .map(|id| document.object(od_core::EntityId(id.0)).map(entity_dto))
        .collect();
    Ok(GetResult {
        revision: WireId(doc.session.revision()),
        entities,
    })
}

fn layers(doc: &Doc) -> LayersResult {
    let document = doc.session.document();
    LayersResult {
        revision: WireId(doc.session.revision()),
        current_layer_id: WireId(document.current_layer().0),
        layers: document
            .layers()
            .iter()
            .map(|l| LayerItem {
                layer_id: WireId(l.id.0),
                name: l.name.clone(),
                // A layer's own colour is never by-layer.
                color: color_dto(&Attr::Set(l.color)),
                linetype: l.linetype.clone(),
                lineweight_hundredths_mm: l.lineweight.0,
                visible: l.on,
                frozen: l.frozen,
                locked: l.locked,
            })
            .collect(),
    }
}

fn selection(doc: &Doc) -> SelectionResult {
    SelectionResult {
        revision: WireId(doc.session.revision()),
        entity_ids: doc.selection.iter().map(|id| WireId(id.0)).collect(),
    }
}
