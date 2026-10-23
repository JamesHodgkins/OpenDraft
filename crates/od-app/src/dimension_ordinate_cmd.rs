//! DIMORDINATE: the X or Y coordinate of a feature, on a leader.

use std::sync::Arc;

use od_commands::{Cancelled, Editor, PointOptions};
use od_core::dimension::suggested_x_datum;
use od_core::Dimension;

use crate::dimension_cmd::{place, style, Make};

pub fn dim_ordinate(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let feature = ed.get_point(PointOptions::new("Specify feature location"))?;
    let make: Make = Arc::new(move |at, tweaks| {
        let x_datum = tweaks
            .datum
            .unwrap_or_else(|| suggested_x_datum(feature, at));
        let mut dimension = Dimension::ordinate(feature, at, x_datum, &style);
        dimension.text = tweaks.text.clone();
        dimension.measurement().map(|_| dimension)
    });
    place(
        ed,
        "Specify leader endpoint",
        &[("Xdatum", "X"), ("Ydatum", "Y"), ("Text", "T")],
        feature,
        make,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::{Entity, Point};

    fn dimension(state: &AppState) -> Dimension {
        state
            .session()
            .document()
            .objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::Dimension(d) => Some((**d).clone()),
                _ => None,
            })
            .unwrap()
    }

    fn run(inputs: &[&str]) -> AppState {
        let mut state = AppState::new();
        state.start_command("DIMORDINATE", dim_ordinate);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        state
    }

    #[test]
    fn a_leader_up_measures_x_and_one_across_measures_y() {
        let up = run(&["12,3", "12,20"]);
        assert_eq!(dimension(&up).label(), "12.00");
        let across = run(&["12,3", "30,3"]);
        assert_eq!(dimension(&across).label(), "3.00");
    }

    #[test]
    fn the_datum_options_override_the_direction() {
        let forced = run(&["12,3", "y", "12,20"]);
        assert_eq!(dimension(&forced).measurement(), Some(3.0));
        let forced = run(&["12,3", "x", "30,3"]);
        assert_eq!(dimension(&forced).measurement(), Some(12.0));
        assert_eq!(dimension(&forced).points()[0], Point::new(12.0, 3.0));
    }
}
