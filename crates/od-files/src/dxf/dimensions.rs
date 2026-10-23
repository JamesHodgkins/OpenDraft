//! Which dimensions the DXF writer writes as DIMENSION entities (see
//! `write2000/dimension.rs`) and which it writes as the lines, arcs and
//! text they are drawn with: an arc length dimension, a jogged radius, and
//! a dimension with a jog line have no DIMENSION to map onto yet.

use std::borrow::Cow;

use od_core::{DimKind, Dimension, Document, Entity};

/// Whether `d` is written as a DIMENSION.
pub(super) fn is_real(d: &Dimension) -> bool {
    d.jog_line.is_none()
        && match &d.kind {
            DimKind::ArcLength { .. } => false,
            DimKind::Radial { jog, .. } => jog.is_none(),
            _ => true,
        }
}

/// The drawing with each dimension that is not [`is_real`] replaced by its
/// picture, and how many were replaced. A drawing with none is borrowed,
/// not copied.
pub(super) fn exploded(document: &Document) -> (Cow<'_, Document>, usize) {
    let unsupported = |entity: &Entity| matches!(entity, Entity::Dimension(d) if !is_real(d));
    if !document.objects().iter().any(|o| unsupported(&o.entity)) {
        return (Cow::Borrowed(document), 0);
    }
    let mut flat = document.clone();
    let replaced = flat.replace_each(|object| {
        unsupported(&object.entity).then(|| {
            object
                .dimension_picture()
                .unwrap_or_default()
                .iter()
                .map(|piece| (piece.clone(), object.attrs.clone()))
                .collect()
        })
    });
    (Cow::Owned(flat), replaced)
}

#[cfg(test)]
mod tests {
    use od_core::{DimStyle, Point};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn aligned() -> Dimension {
        Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &DimStyle::default())
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
    fn real_dimensions_stay_and_the_others_become_their_picture_in_place() {
        let arc = Dimension::arc_length(
            p(0.0, 0.0),
            p(5.0, 0.0),
            p(0.0, 5.0),
            p(6.0, 6.0),
            &DimStyle::default(),
        );
        let pieces = arc.picture().len();
        let mut doc = Document::default();
        doc.add(Entity::Line {
            start: p(0.0, 0.0),
            end: p(1.0, 0.0),
        });
        doc.add(Entity::Dimension(Box::new(aligned())));
        doc.add(Entity::Dimension(Box::new(arc)));
        doc.add(Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        });
        let (flat, count) = exploded(&doc);
        assert_eq!(count, 1);
        assert_eq!(flat.len(), 3 + pieces);
        assert!(matches!(flat.objects()[1].entity, Entity::Dimension(_)));
        assert!(matches!(
            flat.objects()[flat.len() - 1].entity,
            Entity::Circle { .. }
        ));
    }

    #[test]
    fn a_jog_line_or_a_jogged_radius_is_not_real() {
        assert!(is_real(&aligned()));
        let jog = aligned().with_jog_line(Some(p(1.0, 5.0))).unwrap();
        assert!(!is_real(&jog));
        let jogged = Dimension::jogged(
            p(0.0, 0.0),
            p(5.0, 0.0),
            p(3.0, 3.0),
            p(4.0, 1.0),
            &DimStyle::default(),
        );
        assert!(!is_real(&jogged));
    }
}
