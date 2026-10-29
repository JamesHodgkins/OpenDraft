//! Which objects a read looks at: one space, and optionally the objects
//! whose box touches a window. Shared by the entity query, the audit and the
//! summary, so they agree on what "in this region" means.

use od_api::ops::{BoundsArg, SpaceFilter};
use od_api::{ApiError, ErrorCode};
use od_core::{Bounds, Document, LayoutId, Object, Point};

/// A window given by a caller: finite, and not inside out.
pub(super) fn finite_bounds(b: &BoundsArg) -> Result<Bounds, ApiError> {
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

/// The space asked for: model space (`None`) by default, else a layout that
/// must exist.
pub(super) fn space_of(
    document: &Document,
    space: Option<&SpaceFilter>,
) -> Result<Option<LayoutId>, ApiError> {
    match space {
        None | Some(SpaceFilter::Model) => Ok(None),
        Some(SpaceFilter::Paper { layout_id }) => {
            let id = LayoutId(layout_id.0);
            if document.layout(id).is_none() {
                return Err(
                    ApiError::new(ErrorCode::InvalidReference, "No such layout.")
                        .detail("layout_id", layout_id),
                );
            }
            Ok(Some(id))
        }
    }
}

/// The objects of `space` in draw order, or those whose box touches
/// `window`.
pub(super) fn objects_in<'a>(
    document: &'a Document,
    space: Option<LayoutId>,
    window: Option<&BoundsArg>,
) -> Result<Vec<&'a Object>, ApiError> {
    Ok(match window {
        Some(b) => document.objects_intersecting(space, finite_bounds(b)?),
        None => document
            .objects()
            .iter()
            .filter(|o| o.layout == space)
            .collect(),
    })
}
