//! Read-side wire forms of the two text entities.

use od_core::{MText, SingleLineText};

use crate::entity::{attachment_name, bounded, num, GeometryDto};

pub(crate) fn mtext(text: &MText) -> GeometryDto {
    let (contents, contents_truncated) = bounded(&text.contents);
    GeometryDto::Mtext {
        insertion: text.insertion.into(),
        direction: text.direction.into(),
        nominal_height: num(text.nominal_height),
        reference_width: num(text.reference_width),
        attachment: attachment_name(text.attachment).to_owned(),
        style_id: text.style.0.to_string(),
        contents,
        contents_truncated,
    }
}

pub(crate) fn single_line(text: &SingleLineText) -> GeometryDto {
    let (contents, contents_truncated) = bounded(&text.contents);
    GeometryDto::Text {
        position: text.placement.position.into(),
        alignment_mode: text
            .alignment
            .map(|a| format!("{:?}", a.mode).to_ascii_lowercase()),
        alignment_end: text.alignment.map(|a| a.second.into()),
        height: num(text.placement.height),
        rotation: num(text.placement.rotation),
        horizontal: format!("{:?}", text.placement.horizontal).to_ascii_lowercase(),
        vertical: format!("{:?}", text.placement.vertical).to_ascii_lowercase(),
        style_id: text.placement.style.0.to_string(),
        contents,
        contents_truncated,
    }
}
