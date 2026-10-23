//! EXPLODE: break rectangles and polylines into lines and arcs, and block
//! references into the objects of their block. The geometry is
//! `Entity::explode` and `Insert::exploded` in `od_core`.

use od_commands::{Cancelled, Editor};
use od_core::{Attributes, Entity, EntityId, Object};

use crate::text::plural;

/// EXPLODE: select the objects (or use those already selected). Each
/// rectangle or polyline is replaced by its lines and arcs, which keep its
/// layer and attributes. A block reference is replaced by its block's
/// objects, one level down, each with its own layer and attributes as in
/// AutoCAD. Other objects are left alone. One undo step.
pub fn explode_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects to explode")?;
    let found = ed.read(move |doc| {
        chosen
            .iter()
            .filter_map(|id| doc.object(*id).map(|o| (*id, parts_of(o))))
            .collect::<Vec<_>>()
    })?;
    let mut gone = Vec::new();
    let mut parts: Vec<(Entity, Attributes)> = Vec::new();
    for (id, pieces) in found {
        let Some(pieces) = pieces else { continue };
        gone.push(id);
        parts.extend(pieces);
    }
    if gone.is_empty() {
        ed.say(
            "Nothing to explode: lines, circles, arcs, ellipses and splines are already simple.",
        );
        return Ok(());
    }
    let (from, to) = (gone.len(), parts.len());
    let made = ed.transaction("Explode", move |tx| {
        let before = tx.document().objects().len();
        tx.add_with(parts);
        let made: Vec<EntityId> = tx.document().objects()[before..]
            .iter()
            .map(|o| o.id)
            .collect();
        tx.remove(gone);
        Ok(made)
    })?;
    // As in AutoCAD, the pieces end up selected.
    ed.select(made);
    ed.say(format!(
        "Exploded {} into {}.",
        plural(from, "object"),
        plural(to, "object")
    ));
    Ok(())
}

/// What an object explodes into, with the attributes each part keeps, or
/// `None` when it is already simple.
fn parts_of(object: &Object) -> Option<Vec<(Entity, Attributes)>> {
    if let Entity::Insert(insert) = &object.entity {
        let parts = insert.exploded().into_iter();
        return Some(parts.map(|p| (p.entity, p.attrs)).collect());
    }
    let pieces = object.entity.explode()?;
    Some(
        pieces
            .into_iter()
            .map(|e| (e, object.attrs.clone()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::Point;

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn exploding(items: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in items {
            state.add_entity(e.clone());
        }
        state.select_all();
        state.start_command("Explode", explode_objects);
        state.settle_command();
        state
    }

    #[test]
    fn a_rectangle_becomes_four_lines_in_one_undo_step() {
        let circle = Entity::Circle {
            center: Point::new(9.0, 9.0),
            radius: 1.0,
        };
        let mut state = exploding(&[
            Entity::Rectangle {
                min: Point::new(0.0, 0.0),
                max: Point::new(2.0, 1.0),
            },
            circle.clone(),
        ]);
        let now = entities(&state);
        assert_eq!(now.len(), 5);
        assert!(now.contains(&circle));
        assert_eq!(
            now.iter()
                .filter(|e| matches!(e, Entity::Line { .. }))
                .count(),
            4
        );
        let selected = state.selection().len();
        assert_eq!(selected, 5, "the pieces and the circle stay selected");
        state.undo();
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn simple_objects_are_left_alone() {
        let state = exploding(&[Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        }]);
        assert_eq!(entities(&state).len(), 1);
    }
}
