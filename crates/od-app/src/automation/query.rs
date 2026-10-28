//! `entities.query`: filtered, paged lists of entities.
//!
//! A page is a slice of the filtered list in draw order. The cursor names
//! the revision and the filters it was made for, so one that is stale or
//! belongs to another query is refused rather than skipping or repeating
//! entities.

use od_api::entity::entity_dto;
use od_api::limits::page_size;
use od_api::ops::{BoundsArg, QueryArgs, QueryResult, SpaceFilter};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::{Bounds, EntityKind, LayerId, LayoutId, Object, Point};

use super::resolve;
use crate::state::AppState;

/// FNV-1a over the filter fields, so a cursor can tell its own query.
fn filter_digest(args: &QueryArgs) -> u64 {
    let mut filters = args.clone();
    filters.cursor = None;
    filters.limit = None;
    let text = serde_json::to_string(&filters).unwrap_or_default();
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn bad_cursor() -> ApiError {
    ApiError::new(
        ErrorCode::InvalidArguments,
        "The cursor is not valid for this query.",
    )
}

/// `revision.digest.offset`, all hex.
fn decode_cursor(cursor: &str) -> Option<(u64, u64, usize)> {
    let mut parts = cursor.split('.');
    let revision = u64::from_str_radix(parts.next()?, 16).ok()?;
    let digest = u64::from_str_radix(parts.next()?, 16).ok()?;
    let offset = usize::from_str_radix(parts.next()?, 16).ok()?;
    parts.next().is_none().then_some((revision, digest, offset))
}

fn finite_bounds(b: &BoundsArg) -> Result<Bounds, ApiError> {
    let all = [b.min_x, b.min_y, b.max_x, b.max_y];
    if all.iter().any(|v| !v.is_finite()) || b.min_x > b.max_x || b.min_y > b.max_y {
        return Err(ApiError::new(
            ErrorCode::InvalidArguments,
            "bounds must be finite with min not above max.",
        ));
    }
    Ok(Bounds {
        min: Point::new(b.min_x, b.min_y),
        max: Point::new(b.max_x, b.max_y),
    })
}

pub(super) fn entities(
    state: &AppState,
    instance: &str,
    args: QueryArgs,
) -> Result<QueryResult, ApiError> {
    let doc = resolve(state, instance, &args.document_id)?;
    let document = doc.session.document();
    let revision = doc.session.revision();
    let digest = filter_digest(&args);
    let offset = match &args.cursor {
        None => 0,
        Some(text) => match decode_cursor(text) {
            Some((r, d, offset)) if d == digest => {
                if r != revision {
                    return Err(ApiError::stale_revision(r, revision));
                }
                offset
            }
            _ => return Err(bad_cursor()),
        },
    };

    let space: Option<LayoutId> = match &args.space {
        None | Some(SpaceFilter::Model) => None,
        Some(SpaceFilter::Paper { layout_id }) => {
            let id = LayoutId(layout_id.0);
            if document.layout(id).is_none() {
                return Err(
                    ApiError::new(ErrorCode::InvalidReference, "No such layout.")
                        .detail("layout_id", layout_id),
                );
            }
            Some(id)
        }
    };
    let layer = args.layer_id.map(|id| LayerId(id.0));
    // A word that names no kind matches nothing, as before.
    let kinds: Option<Vec<EntityKind>> = args
        .kinds
        .as_ref()
        .map(|k| k.iter().filter_map(|s| EntityKind::from_wire(s)).collect());

    let candidates: Vec<&Object> = match &args.bounds {
        Some(b) => document.objects_intersecting(space, finite_bounds(b)?),
        None => document
            .objects()
            .iter()
            .filter(|o| o.layout == space)
            .collect(),
    };
    let matching = |o: &&Object| {
        layer.is_none_or(|l| o.attrs.layer == l)
            && kinds.as_ref().is_none_or(|k| k.contains(&o.entity.kind()))
    };

    let limit = page_size(args.limit);
    let mut page = candidates.into_iter().filter(matching).skip(offset);
    let entities: Vec<_> = page
        .by_ref()
        .take(limit)
        .map(|o| entity_dto(document, o))
        .collect();
    let next_cursor = page
        .next()
        .map(|_| format!("{revision:x}.{digest:x}.{:x}", offset + entities.len()));
    Ok(QueryResult {
        revision: WireId(revision),
        entities,
        next_cursor,
    })
}
