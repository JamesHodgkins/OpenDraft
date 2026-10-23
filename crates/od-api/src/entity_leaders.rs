//! The read-only summaries of leaders and multileaders.

use od_core::{Leader, MLeader};

use crate::entity::{bounded, GeometryDto};

pub(crate) fn leader(l: &Leader) -> GeometryDto {
    GeometryDto::Leader {
        vertices: l.vertices.iter().map(|p| (*p).into()).collect(),
        arrow: l.arrow,
        hookline: l.hookline,
        annotation_id: l.annotation.map(|id| id.0.to_string()),
    }
}

pub(crate) fn multileader(m: &MLeader) -> GeometryDto {
    let (text, text_truncated) = match m.text() {
        Some(t) => {
            let (text, cut) = bounded(&t.text.contents);
            (Some(text), cut)
        }
        None => (None, false),
    };
    GeometryDto::Multileader {
        style: bounded(&m.style_name).0,
        lines: m
            .lines
            .iter()
            .map(|l| l.iter().map(|p| (*p).into()).collect())
            .collect(),
        landing_direction: m.landing_dir.into(),
        text_insertion: m.text().map(|t| t.text.insertion.into()),
        text,
        text_truncated,
    }
}
