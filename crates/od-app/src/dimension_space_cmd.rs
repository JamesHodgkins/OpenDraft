//! DIMSPACE and DIMJOGLINE: the spacing of parallel dimension lines, and a
//! jog line in a dimension line. Both are pure methods on `Dimension` in
//! `od_core::dimension`; these ask.

use od_commands::{Answer, Cancelled, Editor, Options, PointOptions};
use od_core::dimension::spaced_offsets;
use od_core::{Dimension, Entity, EntityId};

use crate::dimension_cmd::pick;
use crate::dimension_edit_cmd::{apply, dimensions_in, Change};

/// DIMSPACE: a base dimension, the dimensions to space, then the spacing
/// (Auto is twice the text height; 0 lines them up with the base).
pub fn dim_space(ed: &mut Editor) -> Result<(), Cancelled> {
    let Some((base_picked, Entity::Dimension(base))) = pick(ed, "Select base dimension")? else {
        ed.say("Select a linear dimension.");
        return Err(Cancelled);
    };
    let base_id = base_picked.id;
    let chosen = ed.get_selection("Select dimensions to space")?;
    let chosen: Vec<EntityId> = dimensions_in(ed, chosen)?
        .into_iter()
        .filter(|id| *id != base_id)
        .collect();
    let auto = 2.0 * base.style.text_height * base.style.scale;
    let options = Options::<f64>::new("Enter value or [Auto]")
        .keyword("Auto", "A")
        .allow_empty();
    let spacing = match ed.ask_distance(options)? {
        Answer::Value(v) if v >= 0.0 => v,
        Answer::Value(_) => {
            ed.say("Requires a value of zero or more.");
            return Err(Cancelled);
        }
        _ => auto,
    };
    let base = *base;
    let offsets = {
        let (base, chosen) = (base.clone(), chosen.clone());
        ed.read(move |doc| {
            chosen
                .iter()
                .map(|id| match doc.get(*id) {
                    Some(Entity::Dimension(d)) => d.line_offset_from(&base),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })?
    };
    // Only dimensions that run the way the base does can be spaced.
    let (ids, now): (Vec<EntityId>, Vec<f64>) = chosen
        .iter()
        .zip(&offsets)
        .filter_map(|(id, off)| off.map(|o| (*id, o)))
        .unzip();
    if ids.len() < chosen.len() {
        ed.say("Dimensions that do not run parallel to the base were left alone.");
    }
    let targets = spaced_offsets(&now, spacing);
    for (id, target) in ids.into_iter().zip(targets) {
        let base = base.clone();
        let change: Change = Box::new(move |d: &Dimension| d.with_line_offset(&base, target));
        apply(ed, "Dimension space", vec![id], change)?;
    }
    Ok(())
}

/// DIMJOGLINE: pick a linear dimension and where the jog goes (Enter for
/// the default place), or Remove it.
pub fn dim_jog_line(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select dimension to add jog or [Remove]")?;
    let ids = dimensions_in(ed, ids)?;
    let options = PointOptions::new("Specify jog location (or press ENTER)").keyword("Remove", "R");
    let change: Change = match ed.ask_point(options)? {
        Answer::Value(at) => Box::new(move |d: &Dimension| d.with_jog_line(Some(at))),
        Answer::Keyword(_) => Box::new(|d: &Dimension| d.with_jog_line(None)),
        Answer::Enter => Box::new(|d: &Dimension| d.with_jog_line(d.default_jog_point())),
    };
    apply(ed, "Dimension jog line", ids, change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::{DimStyle, Point};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn with_three() -> AppState {
        let mut state = AppState::new();
        for y in [5.0, 11.0, 30.0] {
            let d = Dimension::linear(
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(5.0, y),
                0.0,
                &DimStyle::default(),
            );
            state.add_entity(Entity::Dimension(Box::new(d)));
        }
        state
    }

    fn line_heights(state: &AppState) -> Vec<f64> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d) => Some(d.points()[2].y),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn dimspace_sets_even_gaps_from_the_base() {
        let mut state = with_three();
        state.start_command("DIMSPACE", dim_space);
        state.settle_command();
        let first = state.session().document().objects()[0].id;
        state.answer_object(p(5.0, 5.0), Some(first), false);
        state.settle_command();
        state.select_all();
        state.cmd_submit("");
        state.settle_command();
        state.cmd_submit("4");
        state.settle_command();
        assert_eq!(line_heights(&state), [5.0, 9.0, 13.0]);
    }

    #[test]
    fn jog_line_goes_in_and_out() {
        let mut state = with_three();
        state.select_all();
        state.start_command("DIMJOGLINE", dim_jog_line);
        state.settle_command();
        state.cmd_submit("");
        state.settle_command();
        let jogged = |state: &AppState| {
            state
                .session()
                .document()
                .objects()
                .iter()
                .filter(|o| matches!(&o.entity, Entity::Dimension(d) if d.jog_line.is_some()))
                .count()
        };
        assert_eq!(jogged(&state), 3);
        state.select_all();
        state.start_command("DIMJOGLINE", dim_jog_line);
        state.settle_command();
        state.cmd_submit("r");
        state.settle_command();
        assert_eq!(jogged(&state), 0);
    }
}
