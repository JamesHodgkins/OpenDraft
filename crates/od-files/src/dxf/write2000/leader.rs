//! LEADER entities, as AutoCAD writes them (`private_material/LEADERS1.dxf`): a
//! straight path with its vertices, and a hard reference (340) to the MTEXT
//! it annotates, which is written as an entity of its own.

use od_core::leader::Leader;
use od_core::{Document, Entity};

use super::{hex, Ids};
use crate::dxf::write::{finite, Out};
use crate::FileError;

/// The handles of the texts leaders annotate, made before any entity is
/// written so a leader can name a text that comes after it.
pub(super) fn annotation_ids(document: &Document) -> Vec<od_core::EntityId> {
    let mut found: Vec<od_core::EntityId> = document
        .objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Leader(l) => l.annotation,
            _ => None,
        })
        .filter(|id| matches!(document.get(*id), Some(Entity::MText(_))))
        .collect();
    found.sort_by_key(|id| id.0);
    found.dedup();
    found
}

/// What follows the entity header: the whole of the leader's own data.
pub(super) fn entity(
    out: &mut Out,
    ids: &Ids,
    document: &Document,
    l: &Leader,
) -> Result<(), FileError> {
    let annotation = l
        .annotation
        .filter(|id| matches!(document.get(*id), Some(Entity::MText(_))))
        .and_then(|id| ids.annotations.get(&id).copied());
    out.pair(100, "AcDbLeader");
    // The named dimension style is not written: only Standard is in the
    // DIMSTYLE table of a file we write.
    out.pair(3, "Standard");
    out.pair(71, i32::from(l.arrow));
    out.pair(72, 0);
    out.pair(73, if annotation.is_some() { 0 } else { 3 });
    if l.hookline {
        out.pair(74, i32::from(l.hook_same_direction));
        out.pair(75, 1);
    }
    out.pair(40, finite(l.annotation_size.0)?);
    out.pair(41, finite(l.annotation_size.1)?);
    out.pair(76, l.vertices.len());
    for p in &l.vertices {
        out.point3(10, finite(p.x)?, finite(p.y)?, 0.0);
    }
    if let Some(handle) = annotation {
        out.pair(340, hex(handle));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use od_core::{MText, Point, TextStyleId};

    use super::*;
    use crate::dxf::read::read as read_dxf;
    use crate::dxf::write2000::write;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn drawing() -> Document {
        let mut doc = Document::default();
        // The leader comes before its text, as AutoCAD's do.
        let first_id = od_core::EntityId(doc.next_ids().0);
        let mut with_text = Leader::new(vec![p(0.0, 0.0), p(5.0, 6.0), p(7.0, 6.0)], 0.18);
        with_text.hookline = true;
        with_text.annotation = Some(od_core::EntityId(first_id.0 + 1));
        with_text.annotation_size = (0.5, 1.5);
        doc.add(Entity::Leader(Box::new(with_text)));
        doc.add(Entity::MText(Box::new(MText::new(
            p(7.1, 6.0),
            0.18,
            "Note",
            TextStyleId::STANDARD,
        ))));
        let mut bare = Leader::new(vec![p(1.0, 1.0), p(9.0, 4.0)], 0.18);
        bare.arrow = false;
        doc.add(Entity::Leader(Box::new(bare)));
        doc
    }

    fn leaders(doc: &Document) -> Vec<Leader> {
        doc.objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Leader(l) => Some((**l).clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn leaders_and_the_text_they_name_read_back() {
        let doc = drawing();
        let back = read_dxf(&write(&doc).unwrap().bytes).unwrap();
        let (a, b) = (leaders(&doc), leaders(&back.document));
        assert_eq!(a.len(), b.len());
        for (was, now) in a.iter().zip(&b) {
            assert_eq!(was.vertices, now.vertices);
            assert_eq!((was.arrow, was.hookline), (now.arrow, now.hookline));
            assert_eq!(was.annotation_size, now.annotation_size);
        }
        let text = back.document.leader_annotation(
            back.document
                .objects()
                .iter()
                .find(|o| matches!(o.entity, Entity::Leader(_)))
                .unwrap()
                .id,
        );
        assert_eq!(text.unwrap().contents, "Note");
        assert!(b[1].annotation.is_none());
        assert!(back.document.validate().is_ok());
    }
}
