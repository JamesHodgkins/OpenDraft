//! The steps of `entities.construct` that make a new shape from numbers: a
//! polygon, a line, and an arc that leaves another shape without a corner.

use od_api::construct_ops::{ArcEnd, PolygonFit};
use od_api::convert::attributes_from;
use od_api::write::AttributesInput;
use od_api::{ApiError, ErrorCode, WireId};
use od_commands::Transaction;
use od_core::{polygon, Attributes, Entity, Point};

use super::super::construct_support::{number, refuse};
use super::Run;

/// `attrs` with the layer named, if one is.
fn on_layer(
    tx: &Transaction,
    attrs: Attributes,
    layer: &Option<WireId>,
) -> Result<Attributes, ApiError> {
    match layer {
        Some(id) => {
            let input = AttributesInput {
                layer_id: Some(*id),
                ..Default::default()
            };
            attributes_from(tx.document(), attrs, &input)
        }
        None => Ok(attrs),
    }
}

impl Run {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn polygon(
        &mut self,
        tx: &mut Transaction,
        client_ref: &str,
        center: Point,
        sides: u32,
        radius: f64,
        fit: PolygonFit,
        rotation: f64,
        layer: &Option<WireId>,
    ) -> Result<(), ApiError> {
        let fit = match fit {
            PolygonFit::Inscribed => polygon::Fit::Inscribed,
            PolygonFit::Circumscribed => polygon::Fit::Circumscribed,
        };
        let points =
            polygon::about_radius(center, radius, sides as usize, fit).ok_or_else(|| {
                refuse(
                    ErrorCode::InvalidArguments,
                    format!(
                        "A polygon has from {} to {} sides and a radius above 0.",
                        polygon::MIN_SIDES,
                        polygon::MAX_SIDES
                    ),
                )
            })?;
        let points = points
            .into_iter()
            .map(|p| p.rotated(center, rotation))
            .collect();
        let attrs = on_layer(tx, tx.document().new_object_attributes(), layer)?;
        let entity = Entity::Polyline {
            points,
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        self.add(tx, client_ref, entity, attrs, None)
    }

    pub(super) fn line(
        &mut self,
        tx: &mut Transaction,
        client_ref: &str,
        start: Point,
        end: Point,
        layer: &Option<WireId>,
    ) -> Result<(), ApiError> {
        if (start.x - end.x).hypot(start.y - end.y) < 1e-9 {
            return Err(refuse(
                ErrorCode::InvalidGeometry,
                "A line needs two different points.",
            ));
        }
        let attrs = on_layer(tx, tx.document().new_object_attributes(), layer)?;
        self.add(tx, client_ref, Entity::Line { start, end }, attrs, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn tangent_arc(
        &mut self,
        tx: &mut Transaction,
        target: &str,
        at: ArcEnd,
        radius: f64,
        sweep: f64,
        client_ref: &str,
        layer: &Option<WireId>,
    ) -> Result<(), ApiError> {
        let source = self.sources(tx, &[target.to_owned()])?.remove(0);
        let made = source
            .entity
            .tangent_arc(
                at == ArcEnd::End,
                number("radius", radius)?,
                number("sweep", sweep)?,
            )
            .ok_or_else(|| {
                refuse(
                    ErrorCode::InvalidGeometry,
                    "A tangent arc leaves a line or an arc: the radius must be above 0 and the \
                     sweep not 0 and at most a full turn (6.283 radians).",
                )
            })?;
        let attrs = on_layer(tx, source.attrs, layer)?;
        self.add(tx, client_ref, made, attrs, source.layout)
    }

    /// Offset's copy, on `layer` when one is named (else the source's).
    pub(super) fn on_layer(
        &self,
        tx: &Transaction,
        attrs: Attributes,
        layer: &Option<WireId>,
    ) -> Result<Attributes, ApiError> {
        on_layer(tx, attrs, layer)
    }
}
