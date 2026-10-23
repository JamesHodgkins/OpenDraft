//! Keeping associative hatches and dimensions in step with the objects they
//! follow.
//!
//! After the edits of a transaction (or a single action) are applied, the
//! hatches that follow an object that changed or went, or that were edited
//! themselves, are looked at (`od_core::hatch::follow`). What has to change
//! becomes one more edit in the same undo step, so undo restores the hatch
//! before the object and redo changes the object before the hatch.

use od_core::dimension::follow_associations;
use od_core::hatch::{follow, Follow};
use od_core::{Document, Entity, EntityId};

use crate::Changes;

/// The replacement entities the associative hatches need after `changes`.
pub(crate) fn updates(doc: &Document, changes: &Changes) -> Vec<(EntityId, Entity)> {
    let touched = |id: &EntityId| changes.changed.contains(id) || changes.removed.contains(id);
    let edited = |id: &EntityId| changes.changed.contains(id) || changes.added.contains(id);
    let lookup = |id: EntityId| doc.get(id).cloned();
    let mut out = Vec::new();
    for object in doc.objects() {
        if let Entity::Dimension(d) = &object.entity {
            if d.assoc.is_empty() {
                continue;
            }
            let dimension_edited = edited(&object.id);
            if !dimension_edited && !d.assoc.iter().any(|a| touched(&a.target)) {
                continue;
            }
            if let Some(next) =
                follow_associations(d, &lookup, dimension_edited, &|id| touched(&id))
            {
                out.push((object.id, Entity::Dimension(Box::new(next))));
            }
            continue;
        }
        let Entity::Hatch(h) = &object.entity else {
            continue;
        };
        if !h.associative {
            continue;
        }
        let hatch_edited = edited(&object.id);
        if !hatch_edited && !h.source_ids().any(|id| touched(&id)) {
            continue;
        }
        match follow(h, &lookup, hatch_edited) {
            Follow::Same => {}
            Follow::Replaced(new) | Follow::Detached(new) => {
                out.push((object.id, Entity::Hatch(Box::new(new))));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use od_core::hatch::{
        loops_from_entities, BoundarySource, HatchEntity, HatchFill, HatchPattern,
    };
    use od_core::{Entity, EntityId, Point};

    use crate::AddEntity;
    use crate::{ModifyEntities, RemoveEntities, Session};

    fn square(x: f64) -> Entity {
        Entity::Rectangle {
            min: Point::new(x, 0.0),
            max: Point::new(x + 10.0, 10.0),
        }
    }

    /// A session with a square and an associative hatch of it.
    fn session() -> (Session, EntityId, EntityId) {
        let mut s = Session::default();
        s.perform(AddEntity::new(square(0.0)));
        let boundary = s.document.objects()[0].id;
        let (mut loops, _) = loops_from_entities(&[square(0.0)], 1e-6).unwrap();
        loops[0].sources = vec![BoundarySource::Entity(boundary)];
        let pattern = HatchPattern::user_defined(2.0, 0.0, false, Point::default()).unwrap();
        let mut h = HatchEntity::new(loops, HatchFill::Pattern(pattern));
        h.associative = true;
        s.perform(AddEntity::new(Entity::Hatch(Box::new(h))));
        let hatch = s.document.objects()[1].id;
        (s, boundary, hatch)
    }

    fn the_hatch(s: &Session, id: EntityId) -> HatchEntity {
        match s.document.get(id) {
            Some(Entity::Hatch(h)) => (**h).clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_hatch_follows_its_boundary_in_the_same_undo_step() {
        let (mut s, boundary, hatch) = session();
        let before = the_hatch(&s, hatch);
        let changes = s.perform(ModifyEntities::new("Move", vec![(boundary, square(50.0))]));
        assert!(changes.changed.contains(&hatch) || changes.changed.contains(&boundary));
        let h = the_hatch(&s, hatch);
        assert!(h.associative);
        assert_eq!(h.bounds().unwrap().min, Point::new(50.0, 0.0));
        assert_eq!(h.loops[0].sources, before.loops[0].sources);
        // One undo puts both back.
        let step = s.undo().unwrap();
        assert_eq!(step.name, "Move");
        assert_eq!(the_hatch(&s, hatch), before);
        assert_eq!(s.document.get(boundary), Some(&square(0.0)));
        s.redo();
        assert_eq!(
            the_hatch(&s, hatch).bounds().unwrap().min,
            Point::new(50.0, 0.0)
        );
    }

    #[test]
    fn it_follows_inside_a_transaction_too() {
        let (mut s, boundary, hatch) = session();
        s.transaction("Move", |tx| {
            tx.modify(boundary, |_| square(7.0));
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(
            the_hatch(&s, hatch).bounds().unwrap().min,
            Point::new(7.0, 0.0)
        );
        assert!(s.undo().is_some());
        assert!(s.undo().is_some() && s.undo().is_some() && s.undo().is_none());
    }

    #[test]
    fn a_hatch_moved_alone_lets_go_but_one_moved_with_its_boundary_does_not() {
        let (mut s, boundary, hatch) = session();
        let moved = Entity::Hatch(Box::new(the_hatch(&s, hatch))).translated(30.0, 0.0);
        s.perform(ModifyEntities::new("Move", vec![(hatch, moved)]));
        let h = the_hatch(&s, hatch);
        assert!(!h.associative && h.loops[0].sources.is_empty());
        assert_eq!(
            h.bounds().unwrap().min,
            Point::new(30.0, 0.0),
            "where it was put"
        );
        // Both moved together stay associated.
        let (mut s, boundary2, hatch2) = session();
        let _ = boundary;
        let moved = Entity::Hatch(Box::new(the_hatch(&s, hatch2))).translated(30.0, 0.0);
        s.transaction("Move", |tx| {
            tx.modify(boundary2, |e| e.translated(30.0, 0.0));
            tx.modify(hatch2, |_| moved.clone());
            Ok::<_, ()>(())
        })
        .unwrap();
        let h = the_hatch(&s, hatch2);
        assert!(h.associative, "boundary and hatch moved as one");
        assert_eq!(h.bounds().unwrap().min, Point::new(30.0, 0.0));
    }

    #[test]
    fn erasing_or_opening_the_boundary_detaches_and_keeps_the_shape() {
        let (mut s, boundary, hatch) = session();
        s.perform(RemoveEntities::new(vec![boundary]));
        let h = the_hatch(&s, hatch);
        assert!(!h.associative);
        assert_eq!(h.bounds().unwrap().max, Point::new(10.0, 10.0));
        s.undo();
        assert!(the_hatch(&s, hatch).associative, "undo restores both");
        // A boundary that no longer closes.
        let open = Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
        };
        s.perform(ModifyEntities::new("Break", vec![(boundary, open)]));
        let h = the_hatch(&s, hatch);
        assert!(!h.associative);
        assert_eq!(h.bounds().unwrap().max, Point::new(10.0, 10.0));
    }

    #[test]
    fn a_property_edit_of_the_hatch_keeps_it_associative() {
        let (mut s, _, hatch) = session();
        let mut h = the_hatch(&s, hatch);
        h.style = od_core::hatch::HatchStyle::Outer;
        s.perform(ModifyEntities::new(
            "Style",
            vec![(hatch, Entity::Hatch(Box::new(h)))],
        ));
        assert!(the_hatch(&s, hatch).associative);
    }

    fn line(end: f64) -> Entity {
        Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(end, 0.0),
        }
    }

    /// A line and an aligned dimension tied to its ends.
    fn with_dimension() -> (Session, EntityId, EntityId) {
        let mut s = Session::default();
        s.perform(AddEntity::new(line(10.0)));
        let target = s.document.objects()[0].id;
        let d = od_core::Dimension::aligned(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 5.0),
            &od_core::DimStyle::default(),
        )
        .associated(&s.document);
        assert_eq!(d.assoc.len(), 2);
        s.perform(AddEntity::new(Entity::Dimension(Box::new(d))));
        let dimension = s.document.objects()[1].id;
        (s, target, dimension)
    }

    fn measured(s: &Session, id: EntityId) -> Option<f64> {
        match s.document.get(id) {
            Some(Entity::Dimension(d)) => d.measurement(),
            _ => None,
        }
    }

    #[test]
    fn a_dimension_follows_its_object_in_the_same_undo_step() {
        let (mut s, target, dimension) = with_dimension();
        s.perform(ModifyEntities::new("Stretch", vec![(target, line(25.0))]));
        assert_eq!(measured(&s, dimension), Some(25.0));
        let step = s.undo().unwrap();
        assert_eq!(step.name, "Stretch");
        assert_eq!(measured(&s, dimension), Some(10.0));
        s.redo();
        assert_eq!(measured(&s, dimension), Some(25.0));
    }

    #[test]
    fn it_follows_inside_a_transaction_and_lets_go_when_erased_or_moved_alone() {
        let (mut s, target, dimension) = with_dimension();
        s.transaction("Stretch", |tx| {
            tx.modify(target, |_| line(12.0));
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(measured(&s, dimension), Some(12.0));
        // Moved on its own it lets go.
        let moved = match s.document.get(dimension) {
            Some(e) => e.translated(0.0, 9.0),
            None => unreachable!(),
        };
        s.perform(ModifyEntities::new("Move", vec![(dimension, moved)]));
        s.perform(ModifyEntities::new("Stretch", vec![(target, line(30.0))]));
        assert_eq!(measured(&s, dimension), Some(12.0), "no longer follows");
        // Erasing the object keeps the dimension.
        let (mut s, target, dimension) = with_dimension();
        s.perform(RemoveEntities::new(vec![target]));
        assert_eq!(measured(&s, dimension), Some(10.0));
        s.undo();
        s.perform(ModifyEntities::new("Stretch", vec![(target, line(20.0))]));
        assert_eq!(measured(&s, dimension), Some(20.0), "undo restored the tie");
    }
}
