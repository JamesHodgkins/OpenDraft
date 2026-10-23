//! DIMCENTER: the centre mark or centrelines (by the sign of `DIMCEN`) of an
//! arc or circle, as ordinary lines.

use od_commands::{Cancelled, Editor};
use od_core::dimension::centre_segments;
use od_core::Entity;

use crate::dimension_cmd::{pick, style};

pub fn dim_centre(ed: &mut Editor) -> Result<(), Cancelled> {
    let size = style(ed)?.centre_mark * style(ed)?.scale;
    loop {
        let Some((_, entity)) = pick(ed, "Select arc or circle")? else {
            return Err(Cancelled);
        };
        let (Entity::Circle { center, radius } | Entity::Arc { center, radius, .. }) = entity
        else {
            ed.say("Select an arc or a circle.");
            continue;
        };
        let lines = centre_segments(center, radius, size);
        if lines.is_empty() {
            ed.say("The centre mark size (DIMCEN) is zero.");
            return Ok(());
        }
        return ed.transaction("Center mark", move |tx| {
            for (start, end) in lines {
                tx.add(Entity::Line { start, end });
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::Point;

    fn run(circle: Entity, dimcen: f64) -> AppState {
        let mut state = AppState::new();
        state.add_entity(circle);
        let mut style = state.dim_style();
        style.centre_mark = dimcen;
        state.set_dim_style(style);
        state.start_command("DIMCENTER", dim_centre);
        state.settle_command();
        let id = state.session().document().objects()[0].id;
        state.answer_object(Point::new(5.0, 0.0), Some(id), false);
        state.settle_command();
        state
    }

    fn circle() -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 5.0,
        }
    }

    #[test]
    fn a_circle_gets_a_cross_in_one_undo_step() {
        let mut state = run(circle(), 2.5);
        assert_eq!(state.session().document().objects().len(), 3);
        state.undo();
        assert_eq!(state.session().document().objects().len(), 1);
    }

    #[test]
    fn a_negative_size_makes_centrelines_that_reach_past_the_circle() {
        let state = run(circle(), -2.0);
        let longest = state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Line { start, end } => Some((end.x - start.x).abs()),
                _ => None,
            })
            .fold(0.0, f64::max);
        assert_eq!(longest, 14.0);
    }

    #[test]
    fn a_zero_size_adds_nothing() {
        let state = run(circle(), 0.0);
        assert_eq!(state.session().document().objects().len(), 1);
    }
}
