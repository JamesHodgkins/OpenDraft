//! The small checks and shapes `entities.construct` shares between its steps.

use od_api::{ApiError, ErrorCode};
use od_core::{Attributes, Entity, EntityId, LayoutId, Point};

pub(super) fn refuse(code: ErrorCode, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message)
}

/// An existing object a step acts on.
pub(super) struct Source {
    pub id: EntityId,
    pub entity: Entity,
    pub attrs: Attributes,
    pub layout: Option<LayoutId>,
}

pub(super) fn point(what: &str, p: &od_api::entity::PointDto) -> Result<Point, ApiError> {
    if p.x.is_finite() && p.y.is_finite() {
        Ok(Point::new(p.x, p.y))
    } else {
        Err(refuse(
            ErrorCode::InvalidArguments,
            format!("{what} must be a finite point."),
        ))
    }
}

pub(super) fn number(what: &str, v: f64) -> Result<f64, ApiError> {
    v.is_finite().then_some(v).ok_or_else(|| {
        refuse(
            ErrorCode::InvalidArguments,
            format!("{what} must be finite."),
        )
    })
}

/// A name is 1 to 64 characters and not all digits, so it is never taken for
/// an entity id.
pub(super) fn name_ok(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 64 || name.bytes().all(|b| b.is_ascii_digit()) {
        return Err(refuse(
            ErrorCode::InvalidArguments,
            "A name must be 1 to 64 characters and not all digits.",
        ));
    }
    Ok(())
}
