//! Attributes from the write DTOs: layer, colour, linetype, lineweight and
//! linetype scale, checked against what the drawing holds.

use od_core::{Attr, Attributes, Color, Document, LayerId, Lineweight};

use crate::convert::{bad, positive};
use crate::entity::{ColorDto, LinetypeDto, LineweightDto};
use crate::write::AttributesInput;
use crate::{ApiError, ErrorCode};

/// `base` with the fields set in `input` replaced, after checking that what
/// they name exists.
pub fn attributes_from(
    doc: &Document,
    mut base: Attributes,
    input: &AttributesInput,
) -> Result<Attributes, ApiError> {
    if let Some(id) = input.layer_id {
        let layer = LayerId(id.0);
        doc.layer(layer).ok_or_else(|| {
            ApiError::new(ErrorCode::InvalidReference, "No such layer.").detail("layer_id", id)
        })?;
        base.layer = layer;
    }
    match &input.color {
        None => {}
        Some(ColorDto::ByLayer) => base.color = Attr::ByLayer,
        Some(ColorDto::Foreground) => base.color = Attr::Set(Color::Foreground),
        Some(ColorDto::Rgb { r, g, b }) => base.color = Attr::Set(Color::Rgb(*r, *g, *b)),
    }
    match &input.linetype {
        None => {}
        Some(LinetypeDto::ByLayer) => base.linetype = Attr::ByLayer,
        Some(LinetypeDto::Named { name }) => {
            doc.linetypes().named(name).ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidReference, "No such linetype.")
                    .detail("linetype", name)
            })?;
            base.linetype = Attr::Set(name.clone());
        }
    }
    match &input.lineweight {
        None => {}
        Some(LineweightDto::ByLayer) => base.lineweight = Attr::ByLayer,
        Some(LineweightDto::Set { hundredths_mm }) => {
            let weight = Lineweight(*hundredths_mm);
            if !Lineweight::STANDARD.contains(&weight) {
                return Err(bad(format!(
                    "{hundredths_mm} is not a standard lineweight (hundredths of a mm)"
                )));
            }
            base.lineweight = Attr::Set(weight);
        }
    }
    if let Some(scale) = input.linetype_scale {
        base.linetype_scale = positive("the linetype scale", scale)?;
    }
    Ok(base)
}
