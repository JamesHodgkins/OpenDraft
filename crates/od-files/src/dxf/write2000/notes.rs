//! What the R2000 writer tells the user it did.

use od_core::{Document, Entity};

/// Why the drawing is written as R2000, and what it could not keep.
pub(super) fn notes(document: &Document) -> Vec<String> {
    let has = |spline: bool| {
        document.objects().iter().any(|o| match o.entity {
            Entity::Spline(_) => spline,
            Entity::Ellipse(_) => !spline,
            _ => false,
        })
    };
    let hatches = document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, Entity::Hatch(_)));
    let texts = document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, Entity::MText(_)));
    let why = match (document.layouts().is_empty(), has(false), has(true)) {
        (true, false, false) if !document.blocks().is_empty() && !texts && !hatches => {
            "Written as DXF 2000 so that blocks are kept (DXF R12 has no BLOCK_RECORD table)."
        }
        (true, false, false) if texts && !hatches => {
            "Written as DXF 2000 so that multiline text is kept (DXF R12 has none)."
        }
        (true, false, false) if hatches => {
            "Written as DXF 2000 so that hatches are kept (DXF R12 has none)."
        }
        (false, ..) => "Written as DXF 2000 so that layouts and viewports are kept.",
        (true, true, true) => {
            "Written as DXF 2000 so that ellipses and splines are kept (DXF R12 has neither)."
        }
        (true, false, true) => "Written as DXF 2000 so that splines are kept (DXF R12 has none).",
        _ => "Written as DXF 2000 so that ellipses are kept (DXF R12 has none).",
    };
    let mut notes = vec![
        why.to_owned(),
        "Rectangles were written as closed polylines.".to_owned(),
    ];
    if texts {
        notes.push(
            "Text styles that use the bundled font were written as Arial; columns were not written."
                .to_owned(),
        );
    }
    if !document.blocks().is_empty() {
        notes.push(
            "Blocks were written as BLOCK records with INSERT, ATTDEF, ATTRIB and SEQEND; they have not been reopened in AutoCAD, and text that is mirrored by a block is written readable."
                .to_owned(),
        );
    }
    if hatches {
        notes.push(
            "Hatches were written without the objects they follow (not associative).".to_owned(),
        );
    }
    notes
}
