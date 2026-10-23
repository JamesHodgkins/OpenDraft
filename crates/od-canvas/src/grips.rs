//! Finding the grip under a point, among the selected objects.

use std::collections::BTreeSet;

use od_core::{Document, EntityId, GripKind, Point};

/// A grip of a selected object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GripHit {
    pub id: EntityId,
    /// The grip's index in `Entity::grips`.
    pub index: usize,
}

/// Grips are shown for this many selected objects at most, so selecting a
/// whole big drawing does not bury it (and the frame) in handles.
pub const GRIP_OBJECT_LIMIT: usize = 100;

/// The selected objects that show grips.
pub fn gripped<'a>(
    document: &'a Document,
    selection: &'a BTreeSet<EntityId>,
) -> impl Iterator<Item = &'a od_core::Object> {
    selection
        .iter()
        .filter_map(|id| document.object(*id))
        .filter(|o| document.is_selectable(o))
        .take(GRIP_OBJECT_LIMIT)
}

/// The grip nearest `at` within `tolerance` drawing units. Where grips
/// coincide (two objects sharing an end) the later object's wins.
pub fn grip_at(
    document: &Document,
    selection: &BTreeSet<EntityId>,
    at: Point,
    tolerance: f64,
) -> Option<GripHit> {
    let mut best: Option<(f64, GripHit)> = None;
    for object in gripped(document, selection) {
        for (index, grip) in object.entity.grips().iter().enumerate() {
            let d = (grip.at.x - at.x).hypot(grip.at.y - at.y);
            if d <= tolerance && best.is_none_or(|(b, _)| d <= b) {
                best = Some((
                    d,
                    GripHit {
                        id: object.id,
                        index,
                    },
                ));
            }
        }
    }
    best.map(|(_, hit)| hit)
}

/// Two stretch grips this close are the same point.
const SHARED: f64 = 1e-9;

/// `hot` plus the stretch grips of the other selected objects that sit on the
/// same points, so a corner two lines share moves as one.
pub fn with_shared(
    document: &Document,
    selection: &BTreeSet<EntityId>,
    hot: Vec<GripHit>,
) -> Vec<GripHit> {
    let stretch_at = |hit: &GripHit| {
        let grip = document
            .object(hit.id)?
            .entity
            .grips()
            .get(hit.index)
            .copied()?;
        (grip.kind == GripKind::Stretch).then_some(grip.at)
    };
    let mut all = hot.clone();
    for at in hot.iter().filter_map(stretch_at) {
        for object in gripped(document, selection) {
            for (index, grip) in object.entity.grips().iter().enumerate() {
                let hit = GripHit {
                    id: object.id,
                    index,
                };
                let near = (grip.at.x - at.x).hypot(grip.at.y - at.y) <= SHARED;
                if near && grip.kind == GripKind::Stretch && !all.contains(&hit) {
                    all.push(hit);
                }
            }
        }
    }
    all
}

/// What pressing a grip does when some grips are already picked ("warm").
///
/// Shift toggles the grip in or out of the warm set and starts nothing.
/// Without Shift, pressing a warm grip makes every warm grip hot, the pressed
/// one first (it is the base point); pressing any other grip makes that one
/// alone hot and forgets the rest. Returns the hot grips, if any.
pub fn press_grip(warm: &mut Vec<GripHit>, pick: GripHit, shift: bool) -> Option<Vec<GripHit>> {
    if shift {
        match warm.iter().position(|w| *w == pick) {
            Some(at) => {
                warm.remove(at);
            }
            None => warm.push(pick),
        }
        return None;
    }
    let hot = if warm.contains(&pick) {
        let mut hot = vec![pick];
        hot.extend(warm.iter().copied().filter(|w| *w != pick));
        hot
    } else {
        vec![pick]
    };
    warm.clear();
    Some(hot)
}

/// Forget warm grips whose object is no longer selected.
pub fn keep_selected(warm: &mut Vec<GripHit>, selection: &BTreeSet<EntityId>) {
    warm.retain(|w| selection.contains(&w.id));
}

#[cfg(test)]
mod tests {
    use od_core::Entity;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn drawing() -> (Document, EntityId, EntityId) {
        let mut doc = Document::default();
        let a = doc.add(Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 0.0),
        });
        let b = doc.add(Entity::Line {
            start: p(10.0, 0.0),
            end: p(10.0, 10.0),
        });
        (doc, a, b)
    }

    #[test]
    fn only_selected_objects_offer_grips() {
        let (doc, a, _) = drawing();
        let none = BTreeSet::new();
        assert_eq!(grip_at(&doc, &none, p(0.0, 0.0), 1.0), None);
        let selected: BTreeSet<_> = [a].into();
        assert_eq!(
            grip_at(&doc, &selected, p(0.2, 0.1), 1.0),
            Some(GripHit { id: a, index: 0 })
        );
        assert_eq!(
            grip_at(&doc, &selected, p(5.0, 0.3), 1.0),
            Some(GripHit { id: a, index: 2 }),
            "the middle"
        );
        assert_eq!(grip_at(&doc, &selected, p(5.0, 3.0), 1.0), None, "too far");
    }

    #[test]
    fn the_nearest_grip_wins_and_a_shared_end_goes_to_the_later_object() {
        let (doc, a, b) = drawing();
        let both: BTreeSet<_> = [a, b].into();
        // Nearer the first line's end than the second's other grips.
        assert_eq!(
            grip_at(&doc, &both, p(9.0, 0.0), 2.0),
            Some(GripHit { id: b, index: 0 }),
            "(10, 0) is both objects' end; the later one wins"
        );
        assert_eq!(
            grip_at(&doc, &both, p(10.0, 9.5), 1.0),
            Some(GripHit { id: b, index: 1 })
        );
    }

    #[test]
    fn a_shared_corner_brings_both_lines_ends_along() {
        let (doc, a, b) = drawing();
        let both: BTreeSet<_> = [a, b].into();
        let pressed = GripHit { id: b, index: 0 };
        assert_eq!(
            with_shared(&doc, &both, vec![pressed]),
            vec![pressed, GripHit { id: a, index: 1 }]
        );
        let only_b: BTreeSet<_> = [b].into();
        assert_eq!(with_shared(&doc, &only_b, vec![pressed]), vec![pressed]);
        let far = GripHit { id: b, index: 1 };
        assert_eq!(with_shared(&doc, &both, vec![far]), vec![far]);
    }

    #[test]
    fn shift_picks_grips_and_a_plain_press_on_one_makes_them_all_hot() {
        let hit = |n: u64, index| GripHit {
            id: EntityId(n),
            index,
        };
        let mut warm = Vec::new();
        assert_eq!(press_grip(&mut warm, hit(1, 0), true), None);
        assert_eq!(press_grip(&mut warm, hit(2, 1), true), None);
        assert_eq!(warm.len(), 2);
        // Shift again takes one back out.
        assert_eq!(press_grip(&mut warm, hit(1, 0), true), None);
        assert_eq!(warm, vec![hit(2, 1)]);
        assert_eq!(press_grip(&mut warm, hit(3, 2), true), None);
        // A plain press on a warm grip: all warm grips, the pressed one first.
        let hot = press_grip(&mut warm, hit(3, 2), false).unwrap();
        assert_eq!(hot, vec![hit(3, 2), hit(2, 1)]);
        assert!(warm.is_empty(), "they are hot now");
    }

    #[test]
    fn a_plain_press_on_a_cold_grip_makes_only_that_one_hot() {
        let hit = |n: u64, index| GripHit {
            id: EntityId(n),
            index,
        };
        let mut warm = vec![hit(1, 0), hit(2, 1)];
        assert_eq!(
            press_grip(&mut warm, hit(9, 0), false),
            Some(vec![hit(9, 0)])
        );
        assert!(warm.is_empty(), "the others are forgotten");
        let mut none = Vec::new();
        assert_eq!(
            press_grip(&mut none, hit(4, 3), false),
            Some(vec![hit(4, 3)])
        );
    }

    #[test]
    fn warm_grips_of_deselected_objects_are_dropped() {
        let hit = |n: u64| GripHit {
            id: EntityId(n),
            index: 0,
        };
        let mut warm = vec![hit(1), hit(2)];
        keep_selected(&mut warm, &[EntityId(2)].into());
        assert_eq!(warm, vec![hit(2)]);
    }

    #[test]
    fn a_locked_layer_shows_no_grips() {
        let mut doc = Document::default();
        let layer = doc.allocate_layer_id();
        doc.push_layer(od_core::Layer {
            locked: true,
            ..od_core::Layer::new(layer, "Locked")
        });
        let id = doc.add_with(
            Entity::Line {
                start: p(0.0, 0.0),
                end: p(10.0, 0.0),
            },
            od_core::Attributes::on_layer(layer),
        );
        let selected: BTreeSet<_> = [id].into();
        assert_eq!(grip_at(&doc, &selected, p(0.0, 0.0), 1.0), None);
    }
}
