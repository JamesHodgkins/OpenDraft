//! DXF writing for dimensions. A real DIMENSION needs a block holding its
//! picture and a dimension style table, which these writers do not have yet,
//! so a dimension goes out as the lines, arcs and arrowheads (and text
//! strokes) it is drawn with. It looks the same and no longer measures.

use std::borrow::Cow;

use od_core::{Document, Entity};

/// The drawing with each dimension replaced by its picture, and how many
/// were replaced. A drawing with none is borrowed, not copied.
pub(super) fn exploded(document: &Document) -> (Cow<'_, Document>, usize) {
    if !document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, Entity::Dimension(_)))
    {
        return (Cow::Borrowed(document), 0);
    }
    let mut flat = document.clone();
    let replaced = flat.replace_each(|object| match &object.entity {
        Entity::Dimension(_) => Some(
            object
                .dimension_picture()
                .unwrap_or_default()
                .iter()
                .map(|piece| (piece.clone(), object.attrs.clone()))
                .collect(),
        ),
        _ => None,
    });
    (Cow::Owned(flat), replaced)
}

#[cfg(test)]
mod tests {
    use od_core::{DimStyle, Dimension, Point};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_drawing_without_dimensions_is_not_copied() {
        let doc = Document::from_entities([Entity::Line {
            start: p(0.0, 0.0),
            end: p(1.0, 0.0),
        }]);
        let (flat, count) = exploded(&doc);
        assert_eq!(count, 0);
        assert!(matches!(flat, Cow::Borrowed(_)));
    }

    #[test]
    fn a_dimension_becomes_its_picture_in_place() {
        let dim = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &DimStyle::default());
        let pieces = dim.picture().len();
        let mut doc = Document::default();
        doc.add(Entity::Line {
            start: p(0.0, 0.0),
            end: p(1.0, 0.0),
        });
        doc.add(Entity::Dimension(Box::new(dim)));
        doc.add(Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        });
        let (flat, count) = exploded(&doc);
        assert_eq!(count, 1);
        assert_eq!(flat.len(), 2 + pieces);
        assert!(matches!(flat.objects()[0].entity, Entity::Line { .. }));
        assert!(matches!(
            flat.objects()[flat.len() - 1].entity,
            Entity::Circle { .. }
        ));
        assert!(!flat
            .objects()
            .iter()
            .any(|o| matches!(o.entity, Entity::Dimension(_))));
    }
}
