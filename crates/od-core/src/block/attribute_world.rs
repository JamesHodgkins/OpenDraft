//! An insert's attribute text in the drawing's coordinates and back, for the
//! file formats (DXF writes `ATTRIB` where it is drawn, not in the block's
//! coordinates). It maps text exactly as [`Insert::picture`] draws it: text
//! keeps its shape under any insert transform, placed by the transform,
//! turned with the insert and scaled by its size.

use crate::block::affine::Similarity;
use crate::block::attribute::AttributeText;
use crate::block::Insert;
use crate::{arc, Point};

impl Insert {
    /// How text is carried from the block to the drawing: exactly as
    /// `picture` does it.
    fn text_frame(&self) -> Similarity {
        let map = self.transform();
        map.similarity().unwrap_or_else(|| Similarity {
            rotation: map.yx.atan2(map.xx),
            scale: map.mean_scale(),
            mirrored: false,
            tx: map.tx,
            ty: map.ty,
        })
    }

    /// `text`, placed in the block's coordinates, as it is in the drawing.
    pub fn attribute_text_in_drawing(&self, text: &AttributeText) -> AttributeText {
        let frame = self.text_frame();
        AttributeText {
            position: frame.apply(text.position),
            height: text.height * frame.scale,
            rotation: arc::normalize(text.rotation + frame.rotation),
            ..text.clone()
        }
    }

    /// `text`, placed in the drawing, in the block's coordinates. `None` when
    /// the insert's transform cannot be undone.
    pub fn attribute_text_in_block(&self, text: &AttributeText) -> Option<AttributeText> {
        let frame = self.text_frame();
        if !(frame.scale.is_finite() && frame.scale.abs() > 1e-12) {
            return None;
        }
        let (dx, dy) = (text.position.x - frame.tx, text.position.y - frame.ty);
        let (sin, cos) = (-frame.rotation).sin_cos();
        let (x, y) = (
            (dx * cos - dy * sin) / frame.scale,
            (dx * sin + dy * cos) / frame.scale,
        );
        let position = Point::new(x, if frame.mirrored { -y } else { y });
        let local = AttributeText {
            position,
            height: text.height / frame.scale,
            rotation: arc::normalize(text.rotation - frame.rotation),
            ..text.clone()
        };
        (position.x.is_finite() && position.y.is_finite() && local.height.is_finite())
            .then_some(local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::tests::square_block;
    use crate::TextStyleId;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_two_directions_undo_each_other_for_any_insert() {
        let text = AttributeText {
            rotation: 0.3,
            ..AttributeText::new(Point::new(10.5, 12.0), 2.0, TextStyleId::STANDARD)
        };
        for (sx, sy, rot) in [
            (1.0, 1.0, 0.0),
            (2.0, 2.0, 0.7),
            (-1.5, 1.5, 2.0),
            (3.0, 1.0, 1.0),
        ] {
            let mut insert = Insert::new(square_block(), Point::new(50.0, -20.0));
            (insert.scale_x, insert.scale_y, insert.rotation) = (sx, sy, rot);
            let world = insert.attribute_text_in_drawing(&text);
            let back = insert.attribute_text_in_block(&world).unwrap();
            assert!(near(back.position.x, text.position.x), "{sx} {sy}");
            assert!(near(back.position.y, text.position.y), "{sx} {sy}");
            assert!(near(back.height, text.height) && near(back.rotation, text.rotation));
        }
    }

    #[test]
    fn it_agrees_with_what_the_insert_draws() {
        let mut insert = Insert::new(square_block(), Point::new(5.0, 5.0));
        insert.scale_x = 2.0;
        insert.scale_y = 2.0;
        insert.rotation = 0.5;
        let text = AttributeText::new(Point::new(10.0, 11.0), 1.0, TextStyleId::STANDARD);
        let world = insert.attribute_text_in_drawing(&text);
        let drawn = crate::block::picture::mapped(
            &crate::Entity::MText(Box::new(text.projection("x"))),
            &insert.transform(),
        );
        let crate::Entity::MText(m) = &drawn[0] else {
            panic!("text")
        };
        assert!(near(m.insertion.x, world.position.x) && near(m.insertion.y, world.position.y));
        assert!(near(m.nominal_height, world.height));
    }

    #[test]
    fn a_singular_transform_has_no_way_back() {
        let mut insert = Insert::new(square_block(), Point::default());
        insert.scale_x = 1e-30;
        insert.scale_y = 1e-30;
        let text = AttributeText::new(Point::new(1.0, 1.0), 1.0, TextStyleId::STANDARD);
        assert!(insert.attribute_text_in_block(&text).is_none());
    }
}
