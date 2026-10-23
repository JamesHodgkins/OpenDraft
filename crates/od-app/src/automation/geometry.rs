//! `geometry.measure` and `geometry.intersections`: answers from the same
//! code the commands use, bounded by the specification's limits.

use std::collections::HashSet;

use od_api::limits::{
    INTERSECT_IDS_MAX, INTERSECT_PAIRS_MAX, INTERSECT_POINTS_MAX, MEASURE_IDS_MAX,
};
use od_api::measure::{
    measure, IntersectArgs, IntersectResult, IntersectionPoint, MeasureArgs, MeasureResult,
};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::{EntityId, Object};

use super::{read::parse, resolve};
use crate::state::AppState;

fn too_many(what: &str, limit: usize) -> ApiError {
    ApiError::new(
        ErrorCode::PayloadTooLarge,
        format!("At most {limit} {what} are allowed in one call."),
    )
    .detail("limit", limit)
}

fn encode(value: &impl serde::Serialize) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}

pub fn measure_entities(
    state: &AppState,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: MeasureArgs = parse(arguments)?;
    if args.entity_ids.len() > MEASURE_IDS_MAX {
        return Err(too_many("ids", MEASURE_IDS_MAX));
    }
    let doc = resolve(state, instance, &args.document_id)?;
    let document = doc.session.document();
    let measurements = args
        .entity_ids
        .iter()
        .map(|id| {
            document
                .object(EntityId(id.0))
                .map(|o| measure(*id, &o.entity))
        })
        .collect();
    encode(&MeasureResult {
        revision: WireId(doc.session.revision()),
        measurements,
    })
}

pub fn intersect_entities(
    state: &AppState,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: IntersectArgs = parse(arguments)?;
    if args.entity_ids.len() > INTERSECT_IDS_MAX {
        return Err(too_many("ids", INTERSECT_IDS_MAX));
    }
    let mut seen = HashSet::new();
    let ids: Vec<WireId> = args
        .entity_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(id.0))
        .collect();
    if ids.len() < 2 {
        return Err(ApiError::new(
            ErrorCode::InvalidArguments,
            "Give at least two different entities.",
        ));
    }
    let doc = resolve(state, instance, &args.document_id)?;
    let document = doc.session.document();
    let mut objects: Vec<(WireId, &Object)> = Vec::new();
    for id in &ids {
        let object = document.object(EntityId(id.0)).ok_or_else(|| {
            ApiError::new(ErrorCode::EntityNotFound, "No such entity.").detail("entity_id", id)
        })?;
        objects.push((*id, object));
    }
    let (supported, skipped): (Vec<_>, Vec<_>) = objects
        .into_iter()
        .partition(|(_, o)| !o.entity.curves().is_empty());

    let mut pairs = Vec::new();
    for (i, (_, a)) in supported.iter().enumerate() {
        for (j, (_, b)) in supported.iter().enumerate().skip(i + 1) {
            // Model and paper space, or two sheets, never meet.
            if a.layout == b.layout && a.bounds().intersects(&b.bounds()) {
                pairs.push((i, j));
            }
        }
    }
    if pairs.len() > INTERSECT_PAIRS_MAX {
        return Err(ApiError::new(
            ErrorCode::ResultTooLarge,
            format!(
                "{} pairs overlap; at most {INTERSECT_PAIRS_MAX} can be tested. Use fewer or smaller sets.",
                pairs.len()
            ),
        )
        .detail("limit", INTERSECT_PAIRS_MAX));
    }
    let mut points = Vec::new();
    for &(i, j) in &pairs {
        let (id_a, a) = supported[i];
        let (id_b, b) = supported[j];
        for p in a.entity.intersections(&b.entity) {
            if points.len() >= INTERSECT_POINTS_MAX {
                return Err(ApiError::new(
                    ErrorCode::ResultTooLarge,
                    format!("More than {INTERSECT_POINTS_MAX} intersection points."),
                )
                .detail("limit", INTERSECT_POINTS_MAX));
            }
            points.push(IntersectionPoint {
                entity_a: id_a,
                entity_b: id_b,
                point: p.into(),
            });
        }
    }
    encode(&IntersectResult {
        revision: WireId(doc.session.revision()),
        pairs_tested: pairs.len(),
        points,
        unsupported: skipped.into_iter().map(|(id, _)| id).collect(),
        notes: vec![
            "Segments lying along each other report no points.".to_owned(),
            "Only entities in the same space are compared.".to_owned(),
        ],
    })
}
