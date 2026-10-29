//! `entities.query`: filtered, paged lists of entities.
//!
//! A page is a slice of the filtered list in draw order. The cursor names
//! the revision and the filters it was made for, so one that is stale or
//! belongs to another query is refused rather than skipping or repeating
//! entities.

use od_api::entity::entity_dto;
use od_api::limits::page_size;
use od_api::ops::{QueryArgs, QueryResult};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::{EntityKind, LayerId, Object};

use super::resolve;
use super::scope::{objects_in, space_of};
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

    let space = space_of(document, args.space.as_ref())?;
    let layer = args.layer_id.map(|id| LayerId(id.0));
    // A word that names no kind matches nothing, as before.
    let kinds: Option<Vec<EntityKind>> = args
        .kinds
        .as_ref()
        .map(|k| k.iter().filter_map(|s| EntityKind::from_wire(s)).collect());

    let candidates: Vec<&Object> = objects_in(document, space, args.bounds.as_ref())?;
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
