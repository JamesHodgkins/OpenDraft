//! `view.render`: an SVG of model space (or a layout's sheet) through the
//! plotting pipeline, with the specification's size limits.

use std::collections::HashSet;

use od_api::entity::BoundsDto;
use od_api::limits::{
    RENDER_ENTITIES_MAX, RENDER_RESULT_BYTES, RENDER_SIDE_MAX, VECTOR_VERTICES_MAX,
};
use od_api::ops::SpaceFilter;
use od_api::view::{RenderArgs, RenderResult, FIDELITY, WIDTH_DEFAULT, WIDTH_MIN};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::plot::{plot_layout, PlotError, PlotSheet};
use od_core::plot_model::plot_region;
use od_core::{Bounds, EntityId, LayoutId, Point};

use super::{read::parse, resolve};
use crate::state::AppState;

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::InvalidArguments, message)
}

fn too_big(message: String, limit: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ResultTooLarge, message).detail("limit", limit)
}

fn plot_error(error: PlotError) -> ApiError {
    match error {
        PlotError::NoLayout => invalid("There is nothing to draw in that window."),
        other => ApiError::new(ErrorCode::ResultTooLarge, other.to_string()),
    }
}

fn vertices(sheet: &PlotSheet) -> usize {
    sheet.paths.iter().map(|p| p.points.len()).sum::<usize>()
        + sheet
            .fills
            .iter()
            .flat_map(|f| &f.contours)
            .map(Vec::len)
            .sum::<usize>()
}

/// Grow a window by 2% each way so strokes at its edge are not cut. A window
/// with no width or no height (one straight line) gets room to be seen.
fn framed(b: Bounds) -> Bounds {
    let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
    let longest = w.max(h);
    let pad = longest * 0.02;
    let room = if longest == 0.0 { 1.0 } else { longest / 2.0 };
    let (extra_x, extra_y) = (
        if w == 0.0 { room } else { 0.0 },
        if h == 0.0 { room } else { 0.0 },
    );
    Bounds {
        min: Point::new(b.min.x - pad - extra_x, b.min.y - pad - extra_y),
        max: Point::new(b.max.x + pad + extra_x, b.max.y + pad + extra_y),
    }
}

fn picked(document: &od_core::Document, ids: &[WireId]) -> Result<HashSet<EntityId>, ApiError> {
    if ids.len() > RENDER_ENTITIES_MAX {
        return Err(too_big(
            format!("At most {RENDER_ENTITIES_MAX} entities can be drawn."),
            RENDER_ENTITIES_MAX,
        ));
    }
    let mut set = HashSet::new();
    for id in ids {
        let object = document.object(EntityId(id.0)).ok_or_else(|| {
            ApiError::new(ErrorCode::EntityNotFound, "No such entity.").detail("entity_id", id)
        })?;
        if object.layout.is_some() {
            return Err(invalid(
                "That entity is on a layout sheet; render the layout instead.",
            ));
        }
        set.insert(object.id);
    }
    Ok(set)
}

fn model_window(
    document: &od_core::Document,
    args: &RenderArgs,
    only: &Option<HashSet<EntityId>>,
) -> Result<Bounds, ApiError> {
    match (&args.bounds, only) {
        (Some(b), _) => {
            let all = [b.min_x, b.min_y, b.max_x, b.max_y];
            if all.iter().any(|v| !v.is_finite()) || b.min_x >= b.max_x || b.min_y > b.max_y {
                return Err(invalid(
                    "bounds must be finite, with min_x below max_x and min_y not above max_y.",
                ));
            }
            Ok(Bounds::from_points(
                Point::new(b.min_x, b.min_y),
                Point::new(b.max_x, b.max_y),
            ))
        }
        (None, Some(set)) => set
            .iter()
            .filter_map(|id| document.object(*id))
            .map(|o| o.bounds())
            .filter(Bounds::is_finite)
            .reduce(Bounds::union)
            .map(framed)
            .ok_or_else(|| invalid("Give at least one entity.")),
        (None, None) => document
            .model_bounds()
            .map(framed)
            .ok_or_else(|| invalid("Model space is empty: there is nothing to draw.")),
    }
}

pub fn render(
    state: &AppState,
    instance: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: RenderArgs = parse(arguments)?;
    let doc = resolve(state, instance, &args.document_id)?;
    let revision = doc.session.revision();
    if let Some(expected) = args.expected_revision.filter(|e| e.0 != revision) {
        return Err(ApiError::stale_revision(expected.0, revision));
    }
    let width = args.width.unwrap_or(WIDTH_DEFAULT);
    if !(width.is_finite() && (WIDTH_MIN..=f64::from(RENDER_SIDE_MAX)).contains(&width)) {
        return Err(invalid(format!(
            "width must be between {WIDTH_MIN} and {RENDER_SIDE_MAX}."
        )));
    }
    let document = doc.session.document();
    let (sheet, region) = match &args.space {
        Some(SpaceFilter::Paper { layout_id }) => {
            if args.entity_ids.is_some() || args.bounds.is_some() {
                return Err(invalid(
                    "A layout sheet is drawn whole: leave out entity_ids and bounds.",
                ));
            }
            let id = LayoutId(layout_id.0);
            document.layout(id).ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidReference, "No such layout.")
                    .detail("layout_id", layout_id)
            })?;
            (plot_layout(document, id).map_err(plot_error)?, None)
        }
        None | Some(SpaceFilter::Model) => {
            let only = args
                .entity_ids
                .as_deref()
                .map(|ids| picked(document, ids))
                .transpose()?;
            let region = model_window(document, &args, &only)?;
            if only.is_none() {
                let inside = document.objects_intersecting(None, region).len();
                if inside > RENDER_ENTITIES_MAX {
                    return Err(too_big(
                        format!(
                            "{inside} entities are in that window; at most {RENDER_ENTITIES_MAX} can be drawn. Use a smaller window."
                        ),
                        RENDER_ENTITIES_MAX,
                    ));
                }
            }
            let sheet = plot_region(document, region, width, only.as_ref()).map_err(plot_error)?;
            (sheet, Some(region))
        }
    };
    let count = vertices(&sheet);
    if count > VECTOR_VERTICES_MAX {
        return Err(too_big(
            format!(
                "The picture would need {count} points; at most {VECTOR_VERTICES_MAX}. Use a smaller window."
            ),
            VECTOR_VERTICES_MAX,
        ));
    }
    let svg = od_files::svg(&sheet);
    if svg.len() > RENDER_RESULT_BYTES {
        return Err(too_big(
            format!(
                "The picture is {} bytes; at most {RENDER_RESULT_BYTES}.",
                svg.len()
            ),
            RENDER_RESULT_BYTES,
        ));
    }
    let result = RenderResult {
        revision: WireId(revision),
        svg,
        width: sheet.width,
        height: sheet.height,
        region: region.map(BoundsDto::from),
        vertices: count,
        fidelity: FIDELITY.to_owned(),
        notes: sheet.notes,
    };
    serde_json::to_value(&result)
        .map_err(|_| ApiError::new(ErrorCode::InternalError, "The result could not be encoded."))
}
