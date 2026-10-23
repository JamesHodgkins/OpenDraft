//! DIMEDIT and DIMTEDIT: change the text and extension lines of dimensions
//! that exist. What each change does is a pure method on
//! `od_core::Dimension`; the commands ask and record.

use od_commands::{Answer, Cancelled, Editor, Options, PointOptions};
use od_core::{DimText, Dimension, Entity, EntityId};

use crate::text::plural;

type Change = Box<dyn Fn(&Dimension) -> Option<Dimension> + Send>;

/// The dimensions among `ids`; says so when there are none.
fn dimensions_in(ed: &Editor, ids: Vec<EntityId>) -> Result<Vec<EntityId>, Cancelled> {
    let found = ed.read(move |doc| {
        ids.into_iter()
            .filter(|id| matches!(doc.get(*id), Some(Entity::Dimension(_))))
            .collect::<Vec<_>>()
    })?;
    if found.is_empty() {
        ed.say("No dimensions selected.");
        return Err(Cancelled);
    }
    Ok(found)
}

/// Apply `change` to each dimension as one undo step, saying how many it
/// could not change.
fn apply(
    ed: &Editor,
    name: &'static str,
    ids: Vec<EntityId>,
    change: Change,
) -> Result<(), Cancelled> {
    let refused = ed.transaction(name, move |tx| {
        let mut refused = 0usize;
        for id in ids {
            let next = match tx.document().get(id) {
                Some(Entity::Dimension(d)) => change(d),
                _ => None,
            };
            match next {
                Some(d) => tx.modify(id, |_| Entity::Dimension(Box::new(d.clone()))),
                None => refused += 1,
            }
        }
        Ok(refused)
    })?;
    if refused > 0 {
        ed.say(format!(
            "{} could not be changed that way.",
            plural(refused, "dimension")
        ));
    }
    Ok(())
}

fn home(d: &Dimension) -> Option<Dimension> {
    let mut next = d.clone();
    next.reset_text();
    Some(next)
}

/// DIMEDIT: Home (put the text back where the style has it), New (type
/// other text), Rotate (turn the text) or Oblique (slant the extension
/// lines).
pub fn dim_edit(ed: &mut Editor) -> Result<(), Cancelled> {
    let kind = ed.get_keyword(
        Options::new("Enter type of dimension editing")
            .keyword("Home", "H")
            .keyword("New", "N")
            .keyword("Rotate", "R")
            .keyword("Oblique", "O")
            .default("Home".to_owned()),
    )?;
    let change: Change = match kind.as_str() {
        "New" => {
            let prompt = "Enter new dimension text (<> is the measurement)";
            let text = match ed.ask_string(Options::<String>::new(prompt))? {
                Answer::Value(typed) => DimText::from_entry(&typed),
                _ => return Err(Cancelled),
            };
            Box::new(move |d| {
                let mut next = d.clone();
                next.text = text.clone();
                Some(next)
            })
        }
        "Rotate" => {
            let angle = ed.get_angle("Specify angle for dimension text")?;
            Box::new(move |d| Some(d.with_text_angle(Some(angle))))
        }
        "Oblique" => {
            let prompt = "Enter obliquing angle (press ENTER for none)";
            let angle = match ed.ask_angle(Options::<f64>::new(prompt).allow_empty())? {
                Answer::Value(a) => Some(a),
                _ => None,
            };
            Box::new(move |d| d.with_oblique(angle))
        }
        _ => Box::new(home),
    };
    let ids = ed.pickset_or_select("Select objects")?;
    let ids = dimensions_in(ed, ids)?;
    apply(ed, "Edit dimension", ids, change)
}

/// DIMTEDIT: pick a dimension, then a new place for its text, or Left,
/// Right, Center (along the dimension line), Home or Angle.
pub fn dim_text_edit(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = loop {
        let ids = ed.pickset_or_select("Select dimension")?;
        if let Ok(found) = dimensions_in(ed, ids) {
            break found;
        }
    };
    let ids = vec![ids[0]];
    let options = PointOptions::new("Specify new location for dimension text")
        .keyword("Left", "L")
        .keyword("Right", "R")
        .keyword("Center", "C")
        .keyword("Home", "H")
        .keyword("Angle", "A");
    let change: Change = match ed.ask_point(options)? {
        Answer::Value(at) => Box::new(move |d| {
            let mut next = d.clone();
            next.text_pos = Some(at);
            Some(next)
        }),
        Answer::Keyword(k) => match k.as_str() {
            "Left" => Box::new(|d| d.with_text_justified(1)),
            "Right" => Box::new(|d| d.with_text_justified(2)),
            "Center" => Box::new(|d| d.with_text_justified(0)),
            "Angle" => {
                let angle = ed.get_angle("Specify angle for dimension text")?;
                Box::new(move |d| Some(d.with_text_angle(Some(angle))))
            }
            _ => Box::new(home),
        },
        Answer::Enter => return Err(Cancelled),
    };
    apply(ed, "Edit dimension text", ids, change)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::state::AppState;
    use od_core::{DimStyle, Point};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn started(run: fn(&mut Editor) -> Result<(), Cancelled>, name: &'static str) -> AppState {
        let mut state = AppState::new();
        let d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &DimStyle::default());
        state.add_entity(Entity::Dimension(Box::new(d)));
        state.select_all();
        state.start_command(name, run);
        state.settle_command();
        state
    }

    fn rerun(state: &mut AppState, run: fn(&mut Editor) -> Result<(), Cancelled>) {
        state.select_all();
        state.start_command("Test", run);
        state.settle_command();
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn the_dimension(state: &AppState) -> Dimension {
        match &state.session().document().objects()[0].entity {
            Entity::Dimension(d) => (**d).clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn new_replaces_the_text_and_undoes_in_one_step() {
        let mut state = started(dim_edit, "DIMEDIT");
        submit(&mut state, "n");
        submit(&mut state, "<> TYP");
        assert_eq!(the_dimension(&state).label(), "10.00 TYP");
        state.undo();
        assert_eq!(the_dimension(&state).label(), "10.00");
    }

    #[test]
    fn rotate_turns_the_text_and_home_puts_it_back() {
        let mut state = started(dim_edit, "DIMEDIT");
        submit(&mut state, "r");
        submit(&mut state, "90");
        let d = the_dimension(&state);
        assert!((d.text_angle.unwrap() - PI / 2.0).abs() < 1e-9);
        rerun(&mut state, dim_edit);
        submit(&mut state, "");
        assert_eq!(the_dimension(&state).text_angle, None);
    }

    #[test]
    fn oblique_slants_and_enter_squares_the_extension_lines() {
        let oblique = |d: &Dimension| match d.kind {
            od_core::DimKind::Linear { oblique, .. } => oblique,
            _ => None,
        };
        let mut state = started(dim_edit, "DIMEDIT");
        submit(&mut state, "o");
        submit(&mut state, "30");
        assert!((oblique(&the_dimension(&state)).unwrap() - 30.0_f64.to_radians()).abs() < 1e-9);
        rerun(&mut state, dim_edit);
        submit(&mut state, "o");
        submit(&mut state, "");
        assert_eq!(oblique(&the_dimension(&state)), None);
    }

    #[test]
    fn dimtedit_justifies_places_and_resets_the_text() {
        let mut state = started(dim_text_edit, "DIMTEDIT");
        submit(&mut state, "l");
        assert_eq!(the_dimension(&state).style.text_just, 1);
        rerun(&mut state, dim_text_edit);
        submit(&mut state, "4,9");
        assert_eq!(the_dimension(&state).text_pos, Some(p(4.0, 9.0)));
        rerun(&mut state, dim_text_edit);
        submit(&mut state, "h");
        assert_eq!(the_dimension(&state).text_pos, None);
    }
}
