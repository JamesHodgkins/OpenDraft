//! A hatch added through `entities.apply_batch`: built from boundary shapes
//! the way the HATCH command builds it (joined into loops, remembered as the
//! hatch's sources, checked that it can be drawn), so a hatch made by a
//! caller is the same hatch a person would have made.

use std::collections::HashMap;

use od_api::write::{HatchPattern, Target};
use od_api::{ApiError, ErrorCode};
use od_core::hatch::{
    builtin, loops_and_sources, BoundarySource, HatchFill, HatchLimits, PatternType,
};
use od_core::{Document, Entity, EntityId, HatchEntity, Point};

use super::mutate::resolve;
use crate::hatch_cmd::check_tolerance;
use crate::text::plural;

/// Ends closer than this count as meeting, as for the HATCH command.
const GAP: f64 = 1e-6;

fn refuse(code: ErrorCode, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message)
}

/// The hatch for `boundary` (entities of the drawing, or added earlier in the
/// batch and found in `added`) filled with `pattern`.
pub(super) fn build(
    document: &Document,
    boundary: &[Target],
    pattern: &HatchPattern,
    associative: bool,
    added: &HashMap<String, EntityId>,
) -> Result<Entity, ApiError> {
    if boundary.is_empty() {
        return Err(refuse(
            ErrorCode::InvalidArguments,
            "A hatch needs at least one boundary.",
        ));
    }
    let ids: Vec<EntityId> = boundary
        .iter()
        .map(|t| resolve(t, added))
        .collect::<Result<_, _>>()?;
    let mut shapes = Vec::with_capacity(ids.len());
    for id in &ids {
        let object = document
            .object(*id)
            .ok_or_else(|| refuse(ErrorCode::EntityNotFound, "A boundary entity is not there."))?;
        shapes.push(object.entity.clone());
    }
    let (found, ignored) = loops_and_sources(&shapes, GAP).map_err(|why| {
        refuse(
            ErrorCode::InvalidGeometry,
            format!("The boundary cannot be hatched: {why}."),
        )
    })?;
    if ignored > 0 {
        return Err(refuse(
            ErrorCode::InvalidReference,
            format!(
                "{} cannot bound a hatch: a boundary must be a closed shape.",
                plural(ignored, "boundary entity")
            ),
        ));
    }
    let loops = found
        .into_iter()
        .map(|(l, used)| {
            l.with_sources(
                used.iter()
                    .map(|&i| BoundarySource::Entity(ids[i]))
                    .collect(),
            )
        })
        .collect();
    let mut hatch = HatchEntity::new(loops, fill(pattern)?);
    hatch.associative = associative;
    if !associative {
        hatch = hatch.detached();
    }
    hatch
        .evaluate(check_tolerance(&hatch), &HatchLimits::default())
        .map_err(|why| {
            refuse(
                ErrorCode::InvalidGeometry,
                format!("The hatch cannot be drawn: {why}. Try another scale."),
            )
        })?;
    Ok(Entity::Hatch(Box::new(hatch)))
}

fn fill(pattern: &HatchPattern) -> Result<HatchFill, ApiError> {
    match pattern {
        HatchPattern::Solid => Ok(HatchFill::Solid),
        HatchPattern::Named { name, scale, angle } => {
            let catalogue = builtin();
            let def = catalogue.find(name).ok_or_else(|| {
                refuse(
                    ErrorCode::InvalidArguments,
                    format!(
                        "The pattern {name:?} is not known. Use one of: {}.",
                        catalogue.names().collect::<Vec<_>>().join(", ")
                    ),
                )
            })?;
            od_core::hatch::HatchPattern::from_definition(
                def,
                PatternType::Predefined,
                *scale,
                *angle,
                Point::new(0.0, 0.0),
            )
            .map(HatchFill::Pattern)
            .map_err(|why| {
                refuse(
                    ErrorCode::InvalidArguments,
                    format!("The pattern cannot be used: {why}."),
                )
            })
        }
    }
}
