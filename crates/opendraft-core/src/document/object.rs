//! An object as the drawing stores it, with the box around it remembered.

use std::sync::{Arc, OnceLock};

use super::EntityId;
use crate::layout::LayoutId;
use crate::{Attributes, Bounds, Entity};

/// An entity as stored in a drawing: its geometry, identity and attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub id: EntityId,
    pub entity: Entity,
    pub attrs: Attributes,
    /// The layout whose paper space holds it, or `None` for model space.
    pub layout: Option<LayoutId>,
    /// The box around the entity, worked out the first time it is asked for.
    bounds: BoundsCache,
    /// What a dimension draws, worked out the first time it is asked for.
    picture: PictureCache,
}

impl Object {
    pub fn new(id: EntityId, entity: Entity, attrs: Attributes, layout: Option<LayoutId>) -> Self {
        Self {
            id,
            entity,
            attrs,
            layout,
            bounds: BoundsCache::default(),
            picture: PictureCache::default(),
        }
    }

    /// The box around the entity: [`Entity::bounds`], worked out once. For a
    /// dimension or a block reference that means building everything it draws,
    /// which costs far more than a test against the box, and culling, picking
    /// and snapping ask every object on every frame or mouse move. Nothing
    /// outside the document can change the entity of a stored object, and
    /// [`Document::replace`](super::Document::replace), the one way that does,
    /// forgets the box (`forget_bounds`).
    pub fn bounds(&self) -> Bounds {
        *self.bounds.0.get_or_init(|| match &self.entity {
            Entity::Dimension(d) => d.bounds_of(&self.dimension_picture().unwrap_or_default()),
            Entity::MLeader(m) => m.bounds_of(&self.dimension_picture().unwrap_or_default()),
            Entity::Leader(l) => l.bounds_of(&self.dimension_picture().unwrap_or_default()),
            other => other.bounds(),
        })
    }

    /// For a dimension, the lines, arrows and text strokes that draw it
    /// ([`Dimension::picture`](crate::Dimension::picture)), built once and
    /// shared: building takes about 3 us and painting, picking, snapping and
    /// selecting all ask. `None` for any other kind of object. Like the box,
    /// it depends on the entity alone and is forgotten when the entity is
    /// replaced.
    pub fn dimension_picture(&self) -> Option<Arc<[Entity]>> {
        let build = || -> Arc<[Entity]> {
            match &self.entity {
                Entity::Dimension(d) => d.picture().into(),
                Entity::MLeader(m) => m.picture().into(),
                Entity::Leader(l) => l.picture().into(),
                _ => Arc::from([]),
            }
        };
        matches!(
            self.entity,
            Entity::Dimension(_) | Entity::MLeader(_) | Entity::Leader(_)
        )
        .then(|| self.picture.0.get_or_init(build).clone())
    }

    /// Forget what is remembered of the entity: it has changed.
    pub(super) fn forget_bounds(&mut self) {
        self.bounds = BoundsCache::default();
        self.picture = PictureCache::default();
    }
}

/// What an object remembers of its box. It says nothing about the object, so
/// two objects that differ only in whether the box has been worked out yet are
/// the same object.
#[derive(Clone, Debug, Default)]
struct BoundsCache(OnceLock<Bounds>);

impl PartialEq for BoundsCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// What an object remembers of what it draws. Like [`BoundsCache`], it says
/// nothing about the object.
#[derive(Clone, Debug, Default)]
struct PictureCache(OnceLock<Arc<[Entity]>>);

impl PartialEq for PictureCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{DimStyle, Dimension, Document, LayerId, Point};

    use super::*;

    fn dimension(len: f64) -> Entity {
        Entity::Dimension(Box::new(Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(len, 0.0),
            Point::new(0.0, 5.0),
            &DimStyle::default(),
        )))
    }

    fn object(entity: Entity) -> Object {
        Object::new(
            EntityId(1),
            entity,
            Attributes::on_layer(LayerId::DEFAULT),
            None,
        )
    }

    #[test]
    fn the_remembered_box_is_the_entitys_box() {
        let entity = dimension(40.0);
        let o = object(entity.clone());
        assert_eq!(o.bounds(), entity.bounds());
        assert_eq!(o.bounds(), entity.bounds(), "asked twice");
    }

    #[test]
    fn two_objects_that_differ_only_in_having_asked_are_equal() {
        let asked = object(dimension(40.0));
        let fresh = asked.clone();
        asked.bounds();
        assert_eq!(asked, fresh);
        // The box asked for is kept by a copy, and is still right.
        assert_eq!(asked.clone().bounds(), asked.bounds());
    }

    #[test]
    fn replacing_the_entity_forgets_the_box() {
        let mut doc = Document::default();
        let id = doc.add(dimension(40.0));
        let before = doc.object(id).unwrap().bounds();
        doc.replace(id, dimension(400.0));
        let after = doc.object(id).unwrap().bounds();
        assert!(after.max.x > before.max.x + 300.0, "{before:?} {after:?}");
        assert_eq!(after, doc.get(id).unwrap().bounds());
        // The drawing's own extents follow too.
        assert_eq!(doc.bounds(), Some(after));
    }

    #[test]
    fn a_dimension_builds_its_picture_once_and_the_box_comes_from_it() {
        let entity = dimension(40.0);
        let o = object(entity.clone());
        let before = crate::dimension::builds_on_this_thread();
        let first = o.dimension_picture().unwrap();
        let bounds = o.bounds();
        let second = o.dimension_picture().unwrap();
        assert_eq!(crate::dimension::builds_on_this_thread(), before + 1);
        assert!(Arc::ptr_eq(&first, &second));
        let Entity::Dimension(d) = &entity else {
            panic!("a dimension")
        };
        assert_eq!(first.to_vec(), d.picture());
        assert_eq!(bounds, entity.bounds());
        // A copy shares the picture instead of building it again.
        let built = crate::dimension::builds_on_this_thread();
        let copy = o.clone();
        assert!(Arc::ptr_eq(&first, &copy.dimension_picture().unwrap()));
        assert_eq!(crate::dimension::builds_on_this_thread(), built);
        assert_eq!(
            object(Entity::Circle {
                center: Point::new(0.0, 0.0),
                radius: 1.0
            })
            .dimension_picture(),
            None
        );
    }

    #[test]
    fn a_dimension_that_draws_nothing_keeps_its_old_box() {
        let mut d = Dimension::aligned(
            Point::new(3.0, 4.0),
            Point::new(3.0, 4.0),
            Point::new(3.0, 4.0),
            &DimStyle::default(),
        );
        d.style.text_height = 0.0;
        let entity = Entity::Dimension(Box::new(d));
        let o = object(entity.clone());
        let fresh = entity.bounds();
        o.dimension_picture();
        assert_eq!(o.bounds(), fresh);
    }

    #[test]
    fn replacing_the_entity_forgets_the_picture_and_equal_objects_stay_equal() {
        let mut doc = Document::default();
        let id = doc.add(dimension(40.0));
        let before = doc.object(id).unwrap().dimension_picture().unwrap();
        let untouched = doc.clone();
        doc.replace(id, dimension(400.0));
        let after = doc.object(id).unwrap().dimension_picture().unwrap();
        assert_ne!(before.to_vec(), after.to_vec());
        let Entity::Dimension(d) = doc.get(id).unwrap() else {
            panic!("a dimension")
        };
        assert_eq!(after.to_vec(), d.picture());
        // Whether a picture has been asked for is not part of the drawing.
        assert_eq!(untouched.clone(), untouched);
        let fresh = Document::from_entities([dimension(40.0)]);
        assert_eq!(fresh, untouched);
    }
}
