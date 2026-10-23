//! DIMJOGGED: a radius dimension whose line heads for a centre shown
//! elsewhere and jogs on the way.

use od_commands::{Cancelled, Editor, PointOptions};
use od_core::{Dimension, Entity, Point};

use crate::dimension_cmd::{pick, style};
use crate::draw::add;

fn midway(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

pub fn dim_jogged(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let (center, radius, near) = loop {
        let Some((picked, entity)) = pick(ed, "Select arc or circle")? else {
            return Err(Cancelled);
        };
        match entity {
            Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => {
                break (center, radius.abs(), picked.at);
            }
            _ => ed.say("Select an arc or a circle."),
        }
    };
    let shown = ed.get_point(PointOptions::new("Specify center location override"))?;
    // The point of the circle the dimension touches: where the user picked.
    let (dx, dy) = (near.x - center.x, near.y - center.y);
    let length = dx.hypot(dy);
    if length < 1e-9 || radius < 1e-9 {
        ed.say("Those points make no dimension.");
        return Ok(());
    }
    let chord = Point::new(
        center.x + dx / length * radius,
        center.y + dy / length * radius,
    );
    let text_style = style.clone();
    let text_at = ed.get_point(
        PointOptions::new("Specify dimension line location")
            .base(shown)
            .ghost(move |at| {
                let mut d =
                    Dimension::jogged(center, chord, shown, midway(chord, shown), &text_style);
                d.text_pos = Some(at);
                vec![Entity::Dimension(Box::new(d))]
            }),
    )?;
    let ghost_style = style.clone();
    let jog = ed.get_point(PointOptions::new("Specify jog location").base(chord).ghost(
        move |at| {
            let mut d = Dimension::jogged(center, chord, shown, at, &ghost_style);
            d.text_pos = Some(text_at);
            vec![Entity::Dimension(Box::new(d))]
        },
    ))?;
    let mut dimension = Dimension::jogged(center, chord, shown, jog, &style);
    dimension.text_pos = Some(text_at);
    if dimension.picture().is_empty() {
        ed.say("Those points make no dimension.");
        return Ok(());
    }
    add(ed, Entity::Dimension(Box::new(dimension)), "Dimension")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn a_circle_gets_a_jogged_radius_in_one_step() {
        let mut state = AppState::new();
        state.add_entity(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 50.0,
        });
        state.start_command("DIMJOGGED", dim_jogged);
        state.settle_command();
        let id = state.session().document().objects()[0].id;
        state.answer_object(Point::new(50.0, 0.0), Some(id), false);
        state.settle_command();
        for input in ["30,4", "20,10", "40,2"] {
            state.cmd_submit(input);
            state.settle_command();
        }
        let dims: Vec<_> = state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d) => Some((**d).clone()),
                _ => None,
            })
            .collect();
        assert_eq!(dims.len(), 1);
        assert_eq!(dims[0].label(), "R50.00");
        assert_eq!(dims[0].points()[2], Point::new(30.0, 4.0));
        state.undo();
        assert_eq!(state.session().document().objects().len(), 1);
    }
}
