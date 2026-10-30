//! `entities.apply_batch`: add, patch, replace and delete entities as one
//! undo step. Everything that can be checked without editing is checked
//! first; the edits then run in a checked transaction that validates the
//! whole drawing before recording and reverts if it is not valid.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::mem::discriminant;

use od_api::convert::{attributes_from, entity_from_new, is_writable, patch_entity, unsupported};
use od_api::limits::{BATCH_OPERATIONS_MAX, POINTS_PER_BATCH_MAX};
use od_api::ops::SpaceFilter;
use od_api::write::{
    BatchArgs, BatchResult, ChangesDto, NewEntity, NewGeometry, Operation, Target,
};
use od_api::{ApiError, ErrorCode, WireId};
use od_commands::{AddCopies, Changes, CheckedError, ModifyAttributes, ModifyEntities, Provenance};
use od_core::{Document, Entity, EntityId, LayoutId};

use super::guard::{open_for_change, Caller};
use super::hatch_mutate;
use super::{document_id, read::parse};
use crate::state::AppState;

fn refuse(code: ErrorCode, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message)
}

/// What each operation will touch, worked out before the first edit.
fn check_shape(args: &BatchArgs) -> Result<(), ApiError> {
    if args.operations.len() > BATCH_OPERATIONS_MAX {
        return Err(refuse(
            ErrorCode::PayloadTooLarge,
            format!("A batch can have at most {BATCH_OPERATIONS_MAX} operations."),
        )
        .detail("limit", BATCH_OPERATIONS_MAX));
    }
    let mut points = 0usize;
    for op in &args.operations {
        if let Operation::Add { entity, .. } | Operation::Replace { entity, .. } = op {
            if let od_api::write::NewGeometry::Polyline { points: p, .. } = &entity.geometry {
                points += p.len();
            }
        }
    }
    if points > POINTS_PER_BATCH_MAX {
        return Err(refuse(
            ErrorCode::PayloadTooLarge,
            format!("A batch can have at most {POINTS_PER_BATCH_MAX} polyline points."),
        )
        .detail("limit", POINTS_PER_BATCH_MAX));
    }
    Ok(())
}

/// A hatch that follows `id` would be left without its boundary if the
/// object turned into another kind.
fn followed_by_hatch(document: &Document, id: EntityId) -> bool {
    document.objects().iter().any(|o| match &o.entity {
        Entity::Hatch(h) => h.associative && h.source_ids().any(|s| s == id),
        _ => false,
    })
}

/// Which existing object an operation names, checked against the drawing
/// and the batch so far. Returns `None` for a reference to an earlier add
/// (resolved while running).
fn check_targets(document: &Document, ops: &[Operation]) -> Result<(), ApiError> {
    let mut refs: HashSet<&str> = HashSet::new();
    let mut touched: HashSet<u64> = HashSet::new();
    let mut gone_refs: HashSet<&str> = HashSet::new();
    for (index, op) in ops.iter().enumerate() {
        let at = |e: ApiError| e.detail("operation", index);
        let target = match op {
            Operation::Add { client_ref, entity } => {
                if client_ref.is_empty() || client_ref.len() > 64 {
                    return Err(at(refuse(
                        ErrorCode::InvalidArguments,
                        "client_ref must be 1 to 64 characters.",
                    )));
                }
                if let NewGeometry::Hatch { boundary, .. } = &entity.geometry {
                    for target in boundary {
                        match target {
                            Target::Id { entity_id }
                                if document.object(EntityId(entity_id.0)).is_none() =>
                            {
                                return Err(at(refuse(
                                    ErrorCode::EntityNotFound,
                                    "A boundary entity is not there.",
                                )
                                .detail("entity_id", entity_id)));
                            }
                            Target::Ref { client_ref: named }
                                if !refs.contains(named.as_str())
                                    || gone_refs.contains(named.as_str()) =>
                            {
                                return Err(at(refuse(
                                    ErrorCode::InvalidReference,
                                    format!(
                                        "client_ref {named:?} is not added earlier in this batch."
                                    ),
                                )));
                            }
                            _ => {}
                        }
                    }
                }
                if !refs.insert(client_ref) {
                    return Err(at(refuse(
                        ErrorCode::InvalidArguments,
                        format!("client_ref {client_ref:?} is used twice."),
                    )));
                }
                continue;
            }
            Operation::Patch { target, .. }
            | Operation::Replace { target, .. }
            | Operation::Delete { target } => target,
        };
        match target {
            Target::Id { entity_id } => {
                if document.object(EntityId(entity_id.0)).is_none() {
                    return Err(at(refuse(ErrorCode::EntityNotFound, "No such entity.")
                        .detail("entity_id", entity_id)));
                }
                if !touched.insert(entity_id.0) {
                    return Err(at(refuse(
                        ErrorCode::InvalidArguments,
                        "An existing entity can be the target of only one operation per batch.",
                    )
                    .detail("entity_id", entity_id)));
                }
            }
            Target::Ref { client_ref } => {
                if !refs.contains(client_ref.as_str()) {
                    return Err(at(refuse(
                        ErrorCode::InvalidReference,
                        format!("client_ref {client_ref:?} is not added earlier in this batch."),
                    )));
                }
                if gone_refs.contains(client_ref.as_str()) {
                    return Err(at(refuse(
                        ErrorCode::InvalidReference,
                        format!("{client_ref:?} was deleted earlier in this batch."),
                    )));
                }
                if matches!(op, Operation::Delete { .. }) {
                    gone_refs.insert(client_ref);
                }
            }
        }
    }
    Ok(())
}

fn layout_of(
    document: &Document,
    space: &Option<SpaceFilter>,
) -> Result<Option<LayoutId>, ApiError> {
    match space {
        None => Ok(document.active_layout()),
        Some(SpaceFilter::Model) => Ok(None),
        Some(SpaceFilter::Paper { layout_id }) => {
            let id = LayoutId(layout_id.0);
            document.layout(id).map(|_| Some(id)).ok_or_else(|| {
                refuse(ErrorCode::InvalidReference, "No such layout.")
                    .detail("layout_id", layout_id)
            })
        }
    }
}

pub(super) fn resolve(
    target: &Target,
    added: &HashMap<String, EntityId>,
) -> Result<EntityId, ApiError> {
    match target {
        Target::Id { entity_id } => Ok(EntityId(entity_id.0)),
        Target::Ref { client_ref } => added
            .get(client_ref)
            .copied()
            .ok_or_else(|| refuse(ErrorCode::InvalidReference, "Unknown client_ref.")),
    }
}

fn add(
    tx: &mut od_commands::Transaction,
    entity: &NewEntity,
    added: &HashMap<String, EntityId>,
) -> Result<EntityId, ApiError> {
    let document = tx.document();
    let built = match &entity.geometry {
        NewGeometry::Hatch {
            boundary,
            pattern,
            associative,
        } => hatch_mutate::build(document, boundary, pattern, *associative, added)?,
        geometry => entity_from_new(document, geometry)?,
    };
    let base = document.new_object_attributes();
    let attrs = match &entity.attributes {
        Some(input) => attributes_from(document, base, input)?,
        None => base,
    };
    let layout = layout_of(document, &entity.space)?;
    tx.perform(AddCopies::new("Add", vec![(built, attrs)]).in_layout(layout));
    tx.document()
        .objects()
        .last()
        .map(|o| o.id)
        .ok_or_else(|| refuse(ErrorCode::InternalError, "The new entity vanished."))
}

fn run_operation(
    tx: &mut od_commands::Transaction,
    op: &Operation,
    added: &mut HashMap<String, EntityId>,
) -> Result<(), ApiError> {
    match op {
        Operation::Add { client_ref, entity } => {
            let id = add(tx, entity, added)?;
            added.insert(client_ref.clone(), id);
        }
        Operation::Patch { target, patch } => {
            let id = resolve(target, added)?;
            let Some(object) = tx.document().object(id) else {
                return Err(refuse(ErrorCode::EntityNotFound, "No such entity."));
            };
            let (old, old_attrs) = (object.entity.clone(), object.attrs.clone());
            if !is_writable(&old) {
                return Err(unsupported(&old));
            }
            if let Some(geometry) = &patch.geometry {
                let new = patch_entity(tx.document(), &old, geometry)?;
                if new != old {
                    tx.perform(ModifyEntities::new("Modify", vec![(id, new)]));
                }
            }
            set_attributes(tx, id, old_attrs, patch.attributes.as_ref())?;
        }
        Operation::Replace { target, entity } => {
            let id = resolve(target, added)?;
            let Some(object) = tx.document().object(id) else {
                return Err(refuse(ErrorCode::EntityNotFound, "No such entity."));
            };
            let (old, old_attrs) = (object.entity.clone(), object.attrs.clone());
            if !is_writable(&old) {
                return Err(unsupported(&old));
            }
            let new = entity_from_new(tx.document(), &entity.geometry)?;
            if discriminant(&new) != discriminant(&old) && followed_by_hatch(tx.document(), id) {
                return Err(refuse(
                    ErrorCode::InvalidReference,
                    "An associative hatch follows this entity, so its kind cannot change.",
                )
                .detail("entity_id", WireId(id.0)));
            }
            if new != old {
                tx.perform(ModifyEntities::new("Modify", vec![(id, new)]));
            }
            set_attributes(tx, id, old_attrs, entity.attributes.as_ref())?;
        }
        Operation::Delete { target } => {
            let id = resolve(target, added)?;
            tx.remove(vec![id]);
        }
    }
    Ok(())
}

fn set_attributes(
    tx: &mut od_commands::Transaction,
    id: EntityId,
    old: od_core::Attributes,
    input: Option<&od_api::write::AttributesInput>,
) -> Result<(), ApiError> {
    let Some(input) = input else {
        return Ok(());
    };
    let new = attributes_from(tx.document(), old.clone(), input)?;
    if new != old {
        tx.perform(ModifyAttributes::new("Modify", vec![(id, new)]));
    }
    Ok(())
}

pub(super) fn changes_dto(c: &Changes) -> ChangesDto {
    let ids = |v: &[EntityId]| v.iter().map(|id| WireId(id.0)).collect();
    ChangesDto {
        added: ids(&c.added),
        removed: ids(&c.removed),
        changed: ids(&c.changed),
        layers: c.layers,
        units: c.units,
        dim_style: c.dim_style,
        layouts: c.layouts,
        blocks: c.blocks,
        linetypes: c.linetypes,
    }
}

/// A name for the undo step: printable, short.
pub(super) fn step_name(label: Option<&str>) -> String {
    let clean: String = label
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if clean.trim().is_empty() {
        "Automation edit".to_owned()
    } else {
        clean.trim().to_owned()
    }
}

pub fn apply_batch(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: BatchArgs = parse(arguments)?;
    check_shape(&args)?;
    run_edit(
        state,
        instance,
        caller,
        Edit {
            document_id: &args.document_id,
            expected_revision: args.expected_revision.0,
            label: args.label.as_deref(),
        },
        |document| check_targets(document, &args.operations),
        |tx| {
            let mut added = HashMap::new();
            for (index, op) in args.operations.iter().enumerate() {
                run_operation(tx, op, &mut added).map_err(|e| e.detail("operation", index))?;
            }
            Ok(added)
        },
    )
}

/// Which drawing an edit is for, the revision the caller saw, and the name
/// of the undo step.
pub(super) struct Edit<'a> {
    pub document_id: &'a str,
    pub expected_revision: u64,
    pub label: Option<&'a str>,
}

/// An edit made as one checked, undoable step by the caller: the drawing is
/// found and its revision checked, `check` looks at it before anything
/// changes, `edit` makes the changes (and returns the names it gave new
/// entities), and the result says what changed. A refusal at any point
/// leaves the drawing as it was.
pub(super) fn run_edit(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    edit: Edit<'_>,
    check: impl FnOnce(&Document) -> Result<(), ApiError>,
    make: impl FnOnce(&mut od_commands::Transaction) -> Result<HashMap<String, EntityId>, ApiError>,
) -> Result<serde_json::Value, ApiError> {
    let index = open_for_change(
        state,
        instance,
        edit.document_id,
        Some(edit.expected_revision),
    )?;
    let doc = &mut state.docs[index];
    let uid = doc.uid;
    let before = doc.session.revision();
    check(doc.session.document())?;

    let name = step_name(edit.label);
    let outcome =
        doc.session
            .checked_transaction(&name, Provenance::Automation(caller.grant_id), make);
    let (added, changes) = match outcome {
        Ok(done) => done,
        Err(CheckedError::Edit(error)) => return Err(error),
        Err(CheckedError::Invalid(why)) => {
            return Err(refuse(
                ErrorCode::DocumentInvalid,
                "The edit would leave the drawing invalid, so nothing was changed.",
            )
            .detail("reason", why));
        }
    };
    doc.prune_selection();
    let after = doc.session.revision();
    let ids: BTreeMap<String, WireId> = added
        .into_iter()
        .map(|(reference, id)| (reference, WireId(id.0)))
        .collect();
    let result = BatchResult {
        document_id: document_id(instance, uid),
        revision_before: WireId(before),
        revision_after: WireId(after),
        changed: after != before,
        history_step: (after != before).then_some(name),
        history_token: (after != before)
            .then(|| doc.session.history().top_undo().map(|s| WireId(s.token)))
            .flatten(),
        ids,
        changes: changes_dto(&changes),
    };
    serde_json::to_value(&result)
        .map_err(|_| refuse(ErrorCode::InternalError, "The result could not be encoded."))
}
