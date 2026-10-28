//! `layers.apply_batch`: create layers, change their properties and set the
//! current one as one undo step.

use std::collections::{BTreeMap, HashMap};

use od_api::layer_ops::{
    LayerOperation, LayerProps, LayerTarget, LayersBatchArgs, LayersBatchResult,
};
use od_api::limits::BATCH_OPERATIONS_MAX;
use od_api::{ApiError, ErrorCode, WireId};
use od_commands::{AddLayer, CheckedError, ModifyLayers, Provenance, SetCurrentLayer, Transaction};
use od_core::{Color, Document, Layer, LayerId, Lineweight};

use super::guard::{open_for_change, Caller};
use super::{document_id, read::parse};
use crate::state::AppState;

use od_api::entity::ColorDto;

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::InvalidArguments, message)
}

/// AutoCAD's rule for layer names: not empty, not long, and none of these.
fn check_name(
    document: &Document,
    name: &str,
    except: Option<LayerId>,
) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 255 {
        return Err(invalid("A layer name must be 1 to 255 characters."));
    }
    if name
        .chars()
        .any(|c| c.is_control() || "<>/\\\":;?*|=`".contains(c))
    {
        return Err(invalid(
            "A layer name cannot contain control characters or any of < > / \\ \" : ; ? * | = `",
        ));
    }
    if document.layer_name_taken(name, except) {
        return Err(invalid(format!("A layer called {name:?} already exists.")));
    }
    Ok(name.to_owned())
}

fn apply_props(document: &Document, layer: &mut Layer, props: &LayerProps) -> Result<(), ApiError> {
    if let Some(name) = &props.name {
        if layer.id == LayerId::DEFAULT && *name != layer.name {
            return Err(invalid("Layer 0 cannot be renamed."));
        }
        if *name != layer.name {
            layer.name = check_name(document, name, Some(layer.id))?;
        }
    }
    match &props.color {
        None => {}
        Some(ColorDto::ByLayer) => return Err(invalid("A layer's colour cannot be by_layer.")),
        Some(ColorDto::Foreground) => layer.color = Color::Foreground,
        Some(ColorDto::Rgb { r, g, b }) => layer.color = Color::Rgb(*r, *g, *b),
    }
    if let Some(name) = &props.linetype {
        let record = document.linetypes().named(name).ok_or_else(|| {
            ApiError::new(ErrorCode::InvalidReference, "No such linetype.").detail("linetype", name)
        })?;
        layer.linetype = record.id;
    }
    if let Some(hundredths) = props.lineweight_hundredths_mm {
        let weight = Lineweight(hundredths);
        if !Lineweight::STANDARD.contains(&weight) {
            return Err(invalid(format!(
                "{hundredths} is not a standard lineweight (hundredths of a mm)."
            )));
        }
        layer.lineweight = weight;
    }
    if let Some(on) = props.visible {
        layer.on = on;
    }
    if let Some(frozen) = props.frozen {
        if frozen && layer.id == document.current_layer() {
            return Err(invalid("The current layer cannot be frozen."));
        }
        layer.frozen = frozen;
    }
    if let Some(locked) = props.locked {
        layer.locked = locked;
    }
    Ok(())
}

fn resolve(
    document: &Document,
    target: &LayerTarget,
    created: &HashMap<String, LayerId>,
) -> Result<LayerId, ApiError> {
    match target {
        LayerTarget::Id { layer_id } => {
            let id = LayerId(layer_id.0);
            document.layer(id).map(|_| id).ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidReference, "No such layer.")
                    .detail("layer_id", layer_id)
            })
        }
        LayerTarget::Ref { client_ref } => created.get(client_ref).copied().ok_or_else(|| {
            ApiError::new(
                ErrorCode::InvalidReference,
                format!("client_ref {client_ref:?} is not created earlier in this batch."),
            )
        }),
    }
}

fn run_operation(
    tx: &mut Transaction,
    op: &LayerOperation,
    created: &mut HashMap<String, LayerId>,
) -> Result<(), ApiError> {
    match op {
        LayerOperation::Create {
            client_ref,
            name,
            props,
        } => {
            if client_ref.is_empty() || client_ref.len() > 64 || created.contains_key(client_ref) {
                return Err(invalid("client_ref must be unique and 1 to 64 characters."));
            }
            let name = check_name(tx.document(), name, None)?;
            let id = tx.allocate_layer_id();
            if props.name.is_some() {
                return Err(invalid("Give a new layer's name as `name`, not in props."));
            }
            let mut layer = Layer::new(id, name);
            apply_props(tx.document(), &mut layer, props)?;
            tx.perform(AddLayer::new(layer));
            created.insert(client_ref.clone(), id);
        }
        LayerOperation::Update { target, props } => {
            let id = resolve(tx.document(), target, created)?;
            let Some(old) = tx.document().layer(id).cloned() else {
                return Err(invalid("No such layer."));
            };
            let mut new = old.clone();
            apply_props(tx.document(), &mut new, props)?;
            if new != old {
                tx.perform(ModifyLayers::new("Modify layer", vec![new]));
            }
        }
        LayerOperation::SetCurrent { target } => {
            let id = resolve(tx.document(), target, created)?;
            if tx.document().layer(id).is_some_and(|l| l.frozen) {
                return Err(invalid("A frozen layer cannot be the current layer."));
            }
            if id != tx.document().current_layer() {
                tx.perform(SetCurrentLayer::new(id));
            }
        }
    }
    Ok(())
}

pub fn apply_layers(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: LayersBatchArgs = parse(arguments)?;
    if args.operations.len() > BATCH_OPERATIONS_MAX {
        return Err(ApiError::new(
            ErrorCode::PayloadTooLarge,
            format!("A batch can have at most {BATCH_OPERATIONS_MAX} operations."),
        )
        .detail("limit", BATCH_OPERATIONS_MAX));
    }
    let index = open_for_change(
        state,
        instance,
        &args.document_id,
        Some(args.expected_revision.0),
    )?;
    let doc = &mut state.docs[index];
    let before = doc.session.revision();
    let name = super::mutate::step_name(args.label.as_deref());
    let outcome =
        doc.session
            .checked_transaction(&name, Provenance::Automation(caller.grant_id), |tx| {
                let mut created = HashMap::new();
                for (n, op) in args.operations.iter().enumerate() {
                    run_operation(tx, op, &mut created).map_err(|e| e.detail("operation", n))?;
                }
                Ok::<_, ApiError>(created)
            });
    let (created, _) = match outcome {
        Ok(done) => done,
        Err(CheckedError::Edit(error)) => return Err(error),
        Err(CheckedError::Invalid(why)) => {
            return Err(ApiError::new(
                ErrorCode::DocumentInvalid,
                "The edit would leave the drawing invalid, so nothing was changed.",
            )
            .detail("reason", why));
        }
    };
    let after = doc.session.revision();
    let changed = after != before;
    let result = LayersBatchResult {
        document_id: document_id(instance, doc.uid),
        revision_before: WireId(before),
        revision_after: WireId(after),
        changed,
        history_step: changed.then_some(name),
        history_token: changed
            .then(|| doc.session.history().top_undo().map(|s| WireId(s.token)))
            .flatten(),
        ids: created
            .into_iter()
            .map(|(r, id)| (r, WireId(id.0)))
            .collect::<BTreeMap<_, _>>(),
        current_layer_id: WireId(doc.session.document().current_layer().0),
    };
    serde_json::to_value(&result)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}
