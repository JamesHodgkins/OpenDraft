//! DXF writing for blocks. The writers have no BLOCKS section yet, so a
//! block reference goes out as the objects it draws, each on its inherited
//! layer. It looks the same but is no longer one block.

use std::borrow::Cow;

use od_core::{Document, Entity};

/// The drawing with each block reference replaced by its pieces, and how
/// many were replaced. A drawing with none is borrowed, not copied.
pub(super) fn exploded(document: &Document) -> (Cow<'_, Document>, usize) {
    if !document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, Entity::Insert(_)))
    {
        return (Cow::Borrowed(document), 0);
    }
    let mut flat = document.clone();
    let replaced = flat.replace_each(|object| match &object.entity {
        Entity::Insert(insert) => Some(
            insert
                .picture_with(&object.attrs)
                .into_iter()
                .map(|piece| (piece.entity, piece.attrs))
                .collect(),
        ),
        _ => None,
    });
    (Cow::Owned(flat), replaced)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use od_core::{Attributes, BlockDef, BlockEntity, BlockId, Insert, LayerId, Point};

    use super::*;

    #[test]
    fn a_block_reference_becomes_its_pieces_in_place() {
        let mut doc = Document::default();
        let def = Arc::new(BlockDef {
            id: BlockId(1),
            name: "Tick".into(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: Entity::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(1.0, 0.0),
                },
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
        });
        doc.push_block(def.clone());
        doc.add(Entity::Insert(Box::new(Insert::new(
            def,
            Point::new(5.0, 5.0),
        ))));
        let (flat, count) = exploded(&doc);
        assert_eq!(count, 1);
        assert_eq!(
            flat.objects()[0].entity,
            Entity::Line {
                start: Point::new(5.0, 5.0),
                end: Point::new(6.0, 5.0)
            }
        );
    }

    /// Exploding many references is one pass over the drawing. Done with a
    /// removal and an insertion for each, 8,000 took nearly three minutes in a
    /// release build, so ten seconds, even unoptimised, is loose and catches it.
    #[test]
    fn exploding_a_great_many_references_scales() {
        let mut doc = Document::default();
        let def = Arc::new(BlockDef {
            id: BlockId(1),
            name: "Tick".into(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: Entity::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(1.0, 0.0),
                },
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
        });
        doc.push_block(def.clone());
        for i in 0..20_000 {
            doc.add(Entity::Insert(Box::new(Insert::new(
                def.clone(),
                Point::new(f64::from(i), 0.0),
            ))));
        }
        let started = std::time::Instant::now();
        let (flat, count) = exploded(&doc);
        assert_eq!((count, flat.len()), (20_000, 20_000));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "took {:?}",
            started.elapsed()
        );
    }
}
